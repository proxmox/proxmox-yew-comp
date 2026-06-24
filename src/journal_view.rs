use std::rc::Rc;

use anyhow::{bail, Error};
use gloo_timers::callback::Timeout;
use serde::Deserialize;
use serde_json::json;
use yew::html::IntoEventCallback;
use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::dom::IntoHtmlElement;
use pwt::props::{AsClassesMut, AsCssStylesMut, CssStyles};
use pwt::widget::{Column, Container, VisibilityContext};
use pwt::{prelude::*, AsyncPool};
use pwt_macros::builder;

const ENTRIES_LOAD_NUM: usize = 500;
const LOAD_ZONE: i32 = 50;

#[builder]
#[derive(Properties, PartialEq, Clone)]
pub struct JournalView {
    #[prop_or_default]
    pub key: Option<Key>,

    /// The URL to query
    pub url: AttrValue,

    /// The classes on the element
    #[prop_or_default]
    pub class: Classes,

    /// The style on the element
    #[prop_or_default]
    pub style: CssStyles,

    /// Request the structured `-J` reader output instead of the plain `-j` lines.
    ///
    /// When enabled the response carries per-entry records (timestamp, identifier, priority, ...)
    /// which are rendered with priority coloring and reboot separators. The backend must support
    /// the `structured` parameter; leave this off for endpoints that only emit plain strings.
    #[prop_or_default]
    #[builder]
    pub structured: bool,

    /// Callback when the loading state changes.
    /// The values determine if it's currently loading and if it's in "tail view" mode
    #[prop_or_default]
    #[builder_cb(IntoEventCallback, into_event_callback, (bool, bool))]
    pub on_loading_change: Option<Callback<(bool, bool)>>,
}

impl AsClassesMut for JournalView {
    fn as_classes_mut(&mut self) -> &mut yew::Classes {
        &mut self.class
    }
}

impl AsCssStylesMut for JournalView {
    fn as_css_styles_mut(&mut self) -> &mut CssStyles {
        &mut self.style
    }
}

impl CssMarginBuilder for JournalView {}
impl CssPaddingBuilder for JournalView {}
impl WidgetStyleBuilder for JournalView {}

impl JournalView {
    pub fn new(url: impl Into<AttrValue>) -> Self {
        yew::props!(Self { url: url.into() })
    }

    pwt::impl_yew_std_props_builder!();
    pwt::impl_class_prop_builder!();
}

/// A single record from the structured (`-J`) reader output.
///
/// Control records carry a `ty` discriminator; log lines do not, so `untagged` falls through to
/// [`LineRecord`] for them.
#[derive(Clone, PartialEq, Deserialize)]
#[serde(untagged)]
enum JournalRecord {
    Control(ControlRecord),
    Line(LineRecord),
}

#[derive(Clone, PartialEq, Deserialize)]
#[serde(tag = "ty", rename_all = "lowercase")]
enum ControlRecord {
    Cursor { c: String },
    Reboot { t: u64 },
    Host { h: String },
    Identifiers { ids: Vec<String> },
    Units { names: Vec<String> },
}

#[derive(Clone, PartialEq, Deserialize)]
struct LineRecord {
    /// realtime microseconds since the epoch
    t: u64,
    /// syslog identifier
    id: String,
    #[serde(default)]
    pid: Option<u64>,
    msg: String,
    /// syslog priority, 0 (emerg) .. 7 (debug)
    p: u8,
}

/// Buffered journal content, in the shape that matches the active reader mode.
///
/// Both variants share the same paging model: the cursors live in
/// [`ProxmoxJournalView::cursors`] and the entries here are only the renderable body, with the
/// leading and trailing cursor elements already stripped in [`Msg::PageLoad`] handling.
enum Content {
    Legacy(Vec<String>),
    Structured(Vec<JournalRecord>),
}

/// A loaded page of journal data plus the cursors that bound it.
pub struct Page {
    start_cursor: String,
    end_cursor: String,
    content: Content,
}

pub enum Msg {
    PageLoad(Page, Position),
    /// A poll that returned no usable entries (only cursors, or nothing at all).
    EmptyLoad,
    Scrolled(i32, i32, i32),
    VisibilityChanged(VisibilityContext),
    Error(Error),
}

enum JournalRequest {
    Initial(usize),
    Bottom(String),
    Top(usize, String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Position {
    Initial,
    Bottom,
    Top,
    Middle,
}

pub struct ProxmoxJournalView {
    cursors: Option<(String, String)>,
    content: Content,
    log_ref: NodeRef,
    timeout: Option<Timeout>,
    position: Position,
    last_error: Option<Error>,
    old_scroll_height: i32,
    visibility: VisibilityContext,
    _visibility_context_observer: Option<ContextHandle<VisibilityContext>>,
    async_pool: AsyncPool,
}

/// Convert a realtime timestamp in microseconds to a short syslog-like local time string,
/// mirroring the ExtJS `'M d H:i:s'` format (for example `Jun 24 14:30:05`).
fn format_timestamp(usec: u64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let date = js_sys::Date::new_0();
    date.set_time(usec as f64 / 1000.0);
    let month = MONTHS
        .get(date.get_month() as usize)
        .copied()
        .unwrap_or("???");
    format!(
        "{} {:02} {:02}:{:02}:{:02}",
        month,
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds(),
    )
}

async fn load_content(
    url: AttrValue,
    request: JournalRequest,
    structured: bool,
) -> Result<(Content, Option<(String, String)>, Position), Error> {
    let (mut param, response_type) = match request {
        JournalRequest::Bottom(end_cursor) => (
            json!({
                "startcursor": end_cursor,
            }),
            Position::Bottom,
        ),
        JournalRequest::Top(num, start_cursor) => (
            json!({
                "endcursor": start_cursor,
                "lastentries": num,
            }),
            Position::Top,
        ),
        JournalRequest::Initial(num) => (
            json!({
                "lastentries": num,
            }),
            Position::Initial,
        ),
    };

    if structured {
        param["structured"] = true.into();
        let resp =
            crate::http_get_full::<Vec<JournalRecord>>(url.to_string(), Some(param)).await?;
        let (content, cursors) = split_structured(resp.data)?;
        Ok((content, cursors, response_type))
    } else {
        let resp = crate::http_get_full::<Vec<String>>(url.to_string(), Some(param)).await?;
        let (content, cursors) = split_legacy(resp.data);
        Ok((content, cursors, response_type))
    }
}

/// Strip the leading/trailing cursor strings from a legacy (`-j`) response.
///
/// Returns the renderable lines and the `(start, end)` cursors, or `None` cursors when the
/// response was empty or cursor-only (a normal outcome, for example a filter that matches nothing).
fn split_legacy(mut lines: Vec<String>) -> (Content, Option<(String, String)>) {
    if lines.len() < 2 {
        return (Content::Legacy(Vec::new()), None);
    }
    let start_cursor = lines.remove(0);
    let end_cursor = lines.pop().unwrap();
    (Content::Legacy(lines), Some((start_cursor, end_cursor)))
}

/// Strip the leading/trailing cursor records from a structured (`-J`) response.
///
/// The first and last data elements are `{"ty":"cursor","c":"..."}` records; everything in between
/// is renderable. Identifier and unit records are kept in the body for now and simply ignored at
/// render time (they will later feed filter autocomplete). Returns `None` cursors for an empty or
/// cursor-only response, matching the legacy path.
fn split_structured(
    mut records: Vec<JournalRecord>,
) -> Result<(Content, Option<(String, String)>), Error> {
    if records.len() < 2 {
        return Ok((Content::Structured(Vec::new()), None));
    }

    let cursor_of = |record: &JournalRecord| match record {
        JournalRecord::Control(ControlRecord::Cursor { c }) => Some(c.clone()),
        _ => None,
    };

    let Some(start_cursor) = cursor_of(&records[0]) else {
        bail!("structured response did not start with a cursor record");
    };
    let Some(end_cursor) = cursor_of(records.last().unwrap()) else {
        bail!("structured response did not end with a cursor record");
    };
    records.pop();
    records.remove(0);

    Ok((
        Content::Structured(records),
        Some((start_cursor, end_cursor)),
    ))
}

impl ProxmoxJournalView {
    fn load(&mut self, ctx: &Context<Self>) {
        if self.timeout.is_some() {
            return;
        }
        let (request, timeout) = match (&self.position, &self.cursors) {
            (_, None) => {
                self.position = Position::Bottom;
                (JournalRequest::Initial(ENTRIES_LOAD_NUM), 0)
            }
            (Position::Bottom, Some((_, end))) => (JournalRequest::Bottom(end.to_string()), 1000),
            (Position::Top, Some((start, _))) => {
                (JournalRequest::Top(ENTRIES_LOAD_NUM, start.to_string()), 0)
            }

            // shouldn't happen
            (Position::Initial, Some(_)) => return,
            (Position::Middle, Some(_)) => return,
        };
        let props = ctx.props().clone();
        let link = ctx.link().clone();
        let callback = ctx.props().on_loading_change.clone();
        let tailview = self.position == Position::Bottom;
        let async_pool = self.async_pool.clone();
        self.timeout = Some(Timeout::new(timeout, move || {
            async_pool.spawn(async move {
                if let Some(callback) = callback {
                    callback.emit((true, tailview));
                }

                let msg = match load_content(props.url, request, props.structured).await {
                    Ok((content, cursors, response_type)) => match cursors {
                        Some((start_cursor, end_cursor)) => Msg::PageLoad(
                            Page {
                                start_cursor,
                                end_cursor,
                                content,
                            },
                            response_type,
                        ),
                        // an empty or cursor-only response is a normal outcome, e.g. a priority
                        // filter that currently matches nothing
                        None => Msg::EmptyLoad,
                    },
                    Err(err) => Msg::Error(err),
                };
                link.send_message(msg);
            });
        }));
    }
}

impl Component for ProxmoxJournalView {
    type Message = Msg;
    type Properties = JournalView;

    fn create(ctx: &Context<Self>) -> Self {
        let (visibility, _visibility_context_observer) = ctx
            .link()
            .context(ctx.link().callback(Msg::VisibilityChanged))
            .unzip();

        let content = if ctx.props().structured {
            Content::Structured(Vec::new())
        } else {
            Content::Legacy(Vec::new())
        };

        let mut this = Self {
            cursors: None,
            content,
            log_ref: Default::default(),
            timeout: None,
            last_error: None,
            position: Position::Initial,
            old_scroll_height: 0,
            visibility: visibility.unwrap_or_default(),
            _visibility_context_observer,
            async_pool: AsyncPool::new(),
        };

        this.load(ctx);
        this
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::EmptyLoad => {
                self.timeout.take();
                if let Some(callback) = ctx.props().on_loading_change.clone() {
                    callback.emit((false, self.position == Position::Bottom));
                }
                // keep the existing cursors and, in live mode, poll again instead of erroring
                if self.position == Position::Bottom {
                    self.load(ctx);
                }
                false
            }
            Msg::PageLoad(page, response_type) => {
                self.timeout.take();
                if let Some(callback) = ctx.props().on_loading_change.clone() {
                    callback.emit((false, self.position == Position::Bottom));
                }

                let (old_start, old_end) = if let Some((start, end)) = self.cursors.take() {
                    (Some(start), Some(end))
                } else {
                    (None, None)
                };
                let Page {
                    start_cursor,
                    end_cursor,
                    content,
                } = page;

                match response_type {
                    Position::Initial => {
                        self.cursors = Some((start_cursor, end_cursor));
                        self.content = content;
                    }
                    Position::Bottom => {
                        self.cursors = Some((old_start.unwrap_or(start_cursor), end_cursor));
                        self.append(content);
                    }
                    Position::Top => {
                        self.cursors = Some((start_cursor, old_end.unwrap_or(end_cursor)));
                        self.prepend(content);
                    }
                    Position::Middle => {}
                }

                if self.position == Position::Bottom {
                    self.load(ctx);
                }
                true
            }
            Msg::Scrolled(scroll_top, height, scroll_height) => {
                let old_position = self.position;
                self.position = if scroll_height - (scroll_top + height) <= LOAD_ZONE {
                    Position::Bottom
                } else if scroll_top < LOAD_ZONE {
                    Position::Top
                } else {
                    Position::Middle
                };
                self.old_scroll_height = scroll_height;
                if self.position != Position::Middle && old_position != Position::Middle {
                    self.load(ctx);
                } else if self.position == Position::Middle && old_position == Position::Bottom {
                    self.timeout.take();
                }

                false
            }
            Msg::Error(err) => {
                self.last_error = Some(err);
                true
            }
            Msg::VisibilityChanged(visibility) => {
                let changed = self.visibility != visibility;
                self.visibility = visibility;
                if changed {
                    if self.visibility.visible {
                        self.load(ctx);
                    } else {
                        self.timeout = None;
                    }
                }
                changed
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let log_ref = self.log_ref.clone();
        let mut log = Container::new()
            .padding(2)
            .onscroll(ctx.link().batch_callback(move |_| {
                if let Some(el) = log_ref.clone().into_html_element() {
                    let scroll_top = el.scroll_top();
                    let scroll_height = el.scroll_height();
                    let height = el.client_height();
                    Some(Msg::Scrolled(scroll_top, height, scroll_height))
                } else {
                    None
                }
            }))
            .class("pwt-flex-fit")
            .class("pwt-log-content");

        match &self.content {
            Content::Legacy(lines) => {
                for line in lines.iter() {
                    log.add_child(format!("{line}\n"));
                }
            }
            Content::Structured(records) => {
                // the wire format factors the hostname into a separate host record; track the
                // latest seen and prefix it onto each line, like the ExtJS view and journalctl
                let mut host: Option<&str> = None;
                for record in records.iter() {
                    if let Some(child) = render_record(record, &mut host) {
                        log.add_child(child);
                    }
                }
            }
        }

        let error = self
            .last_error
            .as_ref()
            .map(|err| pwt::widget::error_message(&err.to_string()).border_top(true));

        Column::new()
            .class("pwt-flex-fit")
            .class(props.class.clone())
            .styles(props.style.clone())
            .with_child(log.into_html_with_ref(self.log_ref.clone()))
            .with_optional_child(error)
            .into()
    }

    fn rendered(&mut self, _ctx: &Context<Self>, _first_render: bool) {
        match self.position {
            Position::Bottom => {
                if let Some(el) = self.log_ref.cast::<web_sys::Element>() {
                    let scroll_height = el.scroll_height();
                    el.set_scroll_top(scroll_height);
                }
            }
            Position::Top => {
                if let Some(el) = self.log_ref.cast::<web_sys::Element>() {
                    let scroll_height = el.scroll_height();
                    el.set_scroll_top(scroll_height - self.old_scroll_height);
                }
            }
            Position::Initial | Position::Middle => {}
        }
    }
}

impl ProxmoxJournalView {
    /// Append a freshly loaded page to the bottom of the buffer; the mode is fixed for a view, so
    /// the variant always matches the existing buffer.
    fn append(&mut self, content: Content) {
        match (&mut self.content, content) {
            (Content::Legacy(existing), Content::Legacy(mut new)) => existing.append(&mut new),
            (Content::Structured(existing), Content::Structured(mut new)) => {
                existing.append(&mut new)
            }
            _ => {}
        }
    }

    /// Prepend a freshly loaded page to the top of the buffer.
    fn prepend(&mut self, content: Content) {
        match (&mut self.content, content) {
            (Content::Legacy(existing), Content::Legacy(mut new)) => {
                new.append(existing);
                *existing = new;
            }
            (Content::Structured(existing), Content::Structured(mut new)) => {
                new.append(existing);
                *existing = new;
            }
            _ => {}
        }
    }
}

/// Render a single structured record, threading the current host across calls.
///
/// Returns `None` for records that carry no visible content (cursors are stripped earlier;
/// host/identifier/unit records only update state or feed future filters).
fn render_record<'a>(record: &'a JournalRecord, host: &mut Option<&'a str>) -> Option<Html> {
    match record {
        JournalRecord::Line(line) => {
            let ts = format_timestamp(line.t);
            let pid = line.pid.map(|pid| format!("[{pid}]")).unwrap_or_default();
            let host_prefix = host.map(|h| format!("{h} ")).unwrap_or_default();
            let prefix = format!("{ts} {host_prefix}{}{pid}: ", line.id);
            // align a multi-line message's continuation lines under where the message starts, like
            // journalctl and the ExtJS view
            let msg = if line.msg.contains('\n') {
                let indent = " ".repeat(prefix.chars().count());
                line.msg.replace('\n', &format!("\n{indent}"))
            } else {
                line.msg.clone()
            };
            let text = format!("{prefix}{msg}\n");
            Some(html! {
                <span class={format!("pwt-journal-prio-{}", line.p)}>
                    {text}
                </span>
            })
        }
        JournalRecord::Control(ControlRecord::Reboot { .. }) => {
            // a full-width hairline separator marks the boot boundary without louder text
            Some(html! {
                <span
                    class="pwt-journal-reboot"
                    style="display: block; margin: 6px 0; border-top: 1px solid currentColor;"
                />
            })
        }
        JournalRecord::Control(ControlRecord::Host { h }) => {
            *host = Some(h);
            None
        }
        // identifiers and units feed filter autocomplete (a follow-up); parse but ignore for now
        JournalRecord::Control(ControlRecord::Identifiers { .. })
        | JournalRecord::Control(ControlRecord::Units { .. }) => None,
        // cursors are stripped before rendering; treat any stray one as nothing to show
        JournalRecord::Control(ControlRecord::Cursor { .. }) => None,
    }
}

impl From<JournalView> for VNode {
    fn from(val: JournalView) -> Self {
        let key = val.key.clone();
        let comp = VComp::new::<ProxmoxJournalView>(Rc::new(val), key);
        VNode::from(comp)
    }
}
