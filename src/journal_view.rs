use std::rc::Rc;

use anyhow::Error;
use gloo_timers::callback::Timeout;
use serde::Deserialize;
use serde_json::json;
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::dom::IntoHtmlElement;
use pwt::props::{AsClassesMut, AsCssStylesMut, CssStyles, FieldBuilder, WidgetBuilder};
use pwt::widget::form::{Checkbox, Combobox};
use pwt::widget::{Button, Column, Container, FieldLabel, Row, Toolbar, VisibilityContext};
use pwt::{prelude::*, AsyncPool};
use pwt_macros::builder;

const ENTRIES_LOAD_NUM: usize = 500;
const LOAD_ZONE: i32 = 50;
/// debounce before a freeform filter change reloads, so typing does not query the server on every
/// keystroke (the unit and identifier comboboxes emit on each input)
const FILTER_DEBOUNCE_MS: u32 = 400;

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

    /// Whether the filter row is shown (structured mode only).
    ///
    /// The parent owns this so the toggle button can sit in a shared toolbar with the live/timespan
    /// selection rather than in the journal view's own toolbar.
    #[prop_or_default]
    #[builder]
    pub show_filters: bool,

    /// Seed the "Unit" filter with a fixed systemd unit (structured mode only).
    ///
    /// Mirrors the ExtJS per-service preset: a view scoped to one unit starts with that unit
    /// pre-filled in the filter. The user can still clear or change it.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub unit: Option<AttrValue>,

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
    PageLoad(Page, Position, Completions),
    /// A poll that returned no usable entries (only cursors, or nothing at all). It can still carry
    /// the autocomplete lists, which the backend may emit alongside an otherwise empty page.
    EmptyLoad(Completions),
    Scrolled(i32, i32, i32),
    VisibilityChanged(VisibilityContext),
    Error(Error),
    /// The minimum-priority combobox changed; an empty value (the "All" placeholder) maps to `None`.
    PriorityChanged(Option<String>),
    /// The freeform unit combobox changed; empty maps to `None`.
    UnitChanged(Option<String>),
    /// The freeform identifier combobox changed; empty maps to `None`.
    ServiceChanged(Option<String>),
    /// The "Kernel only" checkbox changed.
    KernelChanged(bool),
    /// The freeform-filter debounce elapsed; apply the pending unit/identifier and reload.
    ApplyFilters,
    /// The "Reset" button cleared every filter back to the unfiltered default.
    ResetFilters,
}

enum JournalRequest {
    Initial(usize),
    Bottom(String),
    Top(usize, String),
}

/// Server-side filter selection shared by every request of a view.
///
/// Mirrors the ExtJS `doLoad` parameter assembly: a minimum priority, a freeform unit and
/// identifier glob, and a kernel-only toggle that is exclusive with unit/identifier.
#[derive(Clone, Default, PartialEq)]
struct Filters {
    /// minimum syslog priority as a "0".."7" string; `None` means no filter ("All")
    priority: Option<String>,
    /// syslog identifier glob (the `service` request parameter)
    service: Option<String>,
    /// systemd unit (matched like `journalctl -u`)
    unit: Option<String>,
    /// restrict to kernel messages; exclusive with `service`/`unit`
    kernel: bool,
}

impl Filters {
    /// Apply the active filters onto a request parameter object.
    ///
    /// `kernel` is exclusive: when set, the unit and identifier globs are omitted entirely, just
    /// like the ExtJS view, which disables those fields while kernel-only is active.
    fn apply(&self, param: &mut serde_json::Value) {
        if let Some(priority) = &self.priority {
            param["priority"] = priority.as_str().into();
        }
        if self.kernel {
            param["kernel"] = 1.into();
        } else {
            if let Some(service) = &self.service {
                param["service"] = service.as_str().into();
            }
            if let Some(unit) = &self.unit {
                param["unit"] = unit.as_str().into();
            }
        }
    }
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

    /// the active server-side filter selection
    filters: Filters,
    /// whether the backend has already returned the autocomplete lists once
    completions_loaded: bool,
    /// syslog identifier suggestions for the "Identifier" combobox, from the backend
    identifiers: Vec<String>,
    /// systemd unit suggestions for the "Unit" combobox, from the backend
    units: Vec<String>,
    /// debounce timer for freeform-filter changes, so typing does not reload on every keystroke
    filter_timeout: Option<Timeout>,
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

/// Autocomplete lists the backend returns once, when asked, for the filter comboboxes.
#[derive(Default)]
pub struct Completions {
    identifiers: Option<Vec<String>>,
    units: Option<Vec<String>>,
}

async fn load_content(
    url: AttrValue,
    request: JournalRequest,
    structured: bool,
    filters: Filters,
    request_completions: bool,
) -> Result<(Content, Option<(String, String)>, Position, Completions), Error> {
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
        filters.apply(&mut param);
        // ask for the completion lists once so the unit/identifier comboboxes can suggest values
        if request_completions {
            param["identifiers"] = true.into();
            param["units"] = true.into();
        }
        let resp = crate::http_get_full::<Vec<JournalRecord>>(url.to_string(), Some(param)).await?;
        let (content, cursors, completions) = split_structured(resp.data);
        Ok((content, cursors, response_type, completions))
    } else {
        let resp = crate::http_get_full::<Vec<String>>(url.to_string(), Some(param)).await?;
        let (content, cursors) = split_legacy(resp.data);
        Ok((content, cursors, response_type, Completions::default()))
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

/// Split a structured (`-J`) response into the renderable body, the bounding cursors, and any
/// autocomplete lists.
///
/// Records are dispatched by kind, not position: when requested, the identifier and unit lists
/// arrive as a preamble before the start cursor, so the cursors are found by scanning for cursor
/// records rather than by taking the first and last element. Returns `None` cursors for an empty
/// or cursor-only response (fewer than two cursors), matching the legacy path; the completion
/// lists can still ride along on such a response.
fn split_structured(
    records: Vec<JournalRecord>,
) -> (Content, Option<(String, String)>, Completions) {
    let mut completions = Completions::default();
    let mut first_cursor: Option<String> = None;
    let mut last_cursor: Option<String> = None;
    let mut cursor_count = 0;
    let mut body = Vec::new();

    for record in records {
        match record {
            JournalRecord::Control(ControlRecord::Identifiers { ids }) => {
                completions.identifiers = Some(ids);
            }
            JournalRecord::Control(ControlRecord::Units { names }) => {
                completions.units = Some(names);
            }
            JournalRecord::Control(ControlRecord::Cursor { c }) => {
                if first_cursor.is_none() {
                    first_cursor = Some(c.clone());
                }
                last_cursor = Some(c);
                cursor_count += 1;
            }
            other => body.push(other),
        }
    }

    let cursors = (cursor_count >= 2).then(|| (first_cursor.unwrap(), last_cursor.unwrap()));

    (Content::Structured(body), cursors, completions)
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
        let filters = self.filters.clone();
        // request the autocomplete lists only until we have them, and only when the comboboxes
        // exist to consume them (structured mode)
        let request_completions = props.structured && !self.completions_loaded;
        self.timeout = Some(Timeout::new(timeout, move || {
            async_pool.spawn(async move {
                if let Some(callback) = callback {
                    callback.emit((true, tailview));
                }

                let msg = match load_content(
                    props.url,
                    request,
                    props.structured,
                    filters,
                    request_completions,
                )
                .await
                {
                    Ok((content, cursors, response_type, completions)) => match cursors {
                        Some((start_cursor, end_cursor)) => Msg::PageLoad(
                            Page {
                                start_cursor,
                                end_cursor,
                                content,
                            },
                            response_type,
                            completions,
                        ),
                        // an empty or cursor-only response is a normal outcome, e.g. a priority
                        // filter that currently matches nothing
                        None => Msg::EmptyLoad(completions),
                    },
                    Err(err) => Msg::Error(err),
                };
                link.send_message(msg);
            });
        }));
    }

    /// Store any autocomplete lists a response carried, marking them loaded so later requests stop
    /// asking for them. Returns whether the suggestion lists changed (and thus a redraw is due).
    fn store_completions(&mut self, completions: Completions) -> bool {
        let mut changed = false;
        if let Some(mut ids) = completions.identifiers {
            ids.sort();
            ids.dedup();
            if self.identifiers != ids {
                self.identifiers = ids;
                changed = true;
            }
            self.completions_loaded = true;
        }
        if let Some(mut names) = completions.units {
            names.sort();
            names.dedup();
            if self.units != names {
                self.units = names;
                changed = true;
            }
            self.completions_loaded = true;
        }
        changed
    }

    /// Drop all buffered state and reload from the bottom, the way the ExtJS `onFilterChange` does:
    /// server-side filters changed, so the cursors and content no longer correspond to the query.
    fn reset_and_reload(&mut self, ctx: &Context<Self>) {
        self.filter_timeout.take();
        self.timeout.take();
        // abort any request already in flight under the old filter, so its response cannot land in
        // the freshly reset buffer and interleave stale entries
        self.async_pool = AsyncPool::new();
        self.cursors = None;
        self.content = Content::Structured(Vec::new());
        self.position = Position::Bottom;
        self.last_error = None;
        self.load(ctx);
    }

    /// Debounce a freeform-filter change: the unit and identifier comboboxes emit on every
    /// keystroke, so wait for a typing pause before reloading rather than querying per character.
    fn schedule_filter_reload(&mut self, ctx: &Context<Self>) {
        let link = ctx.link().clone();
        self.filter_timeout = Some(Timeout::new(FILTER_DEBOUNCE_MS, move || {
            link.send_message(Msg::ApplyFilters);
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

        // seed the unit filter from the per-service preset prop, mirroring the ExtJS `unit` config
        let filters = Filters {
            unit: ctx
                .props()
                .unit
                .as_ref()
                .map(|unit| unit.to_string())
                .filter(|unit| !unit.is_empty()),
            ..Filters::default()
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
            filters,
            completions_loaded: false,
            identifiers: Vec::new(),
            units: Vec::new(),
            filter_timeout: None,
        };

        this.load(ctx);
        this
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::EmptyLoad(completions) => {
                self.timeout.take();
                if let Some(callback) = ctx.props().on_loading_change.clone() {
                    callback.emit((false, self.position == Position::Bottom));
                }
                let changed = self.store_completions(completions);
                // keep the existing cursors and, in live mode, poll again instead of erroring
                if self.position == Position::Bottom {
                    self.load(ctx);
                }
                changed
            }
            Msg::PageLoad(page, response_type, completions) => {
                self.timeout.take();
                if let Some(callback) = ctx.props().on_loading_change.clone() {
                    callback.emit((false, self.position == Position::Bottom));
                }

                self.store_completions(completions);

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
            Msg::PriorityChanged(priority) => {
                if self.filters.priority == priority {
                    return false;
                }
                self.filters.priority = priority;
                self.reset_and_reload(ctx);
                true
            }
            Msg::UnitChanged(unit) => {
                if self.filters.unit == unit {
                    return false;
                }
                self.filters.unit = unit;
                self.schedule_filter_reload(ctx);
                true
            }
            Msg::ServiceChanged(service) => {
                if self.filters.service == service {
                    return false;
                }
                self.filters.service = service;
                self.schedule_filter_reload(ctx);
                true
            }
            Msg::KernelChanged(kernel) => {
                if self.filters.kernel == kernel {
                    return false;
                }
                self.filters.kernel = kernel;
                self.reset_and_reload(ctx);
                true
            }
            Msg::ApplyFilters => {
                self.reset_and_reload(ctx);
                true
            }
            Msg::ResetFilters => {
                if self.filters == Filters::default() {
                    return false;
                }
                self.filters = Filters::default();
                self.reset_and_reload(ctx);
                true
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
            .with_optional_child(
                (props.structured && props.show_filters).then(|| self.render_filter_row(ctx)),
            )
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

    /// The filter row: minimum priority, unit, identifier, and a kernel-only toggle.
    ///
    /// Mirrors the ExtJS filter row: kernel-only is exclusive, so it disables the unit and
    /// identifier fields; every control reloads from scratch on change.
    fn render_filter_row(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        // the syslog priorities 0..=6, plus an explicit "All" sentinel that maps back to no filter
        let priority = Combobox::from_key_value_pairs([
            ("__all__", tr!("All")),
            ("0", tr!("Emergency")),
            ("1", tr!("Alert")),
            ("2", tr!("Critical")),
            ("3", tr!("Error")),
            ("4", tr!("Warning")),
            ("5", tr!("Notice")),
            ("6", tr!("Informational")),
        ])
        .width(200)
        .show_filter(false)
        // always has a value (the "All" item is "no filter"), so mark it required to drop the
        // redundant clear trigger the selector would otherwise add for a non-empty value
        .required(true)
        .value(
            self.filters
                .priority
                .clone()
                .unwrap_or_else(|| "__all__".to_string()),
        )
        .on_change(link.callback(|value: String| {
            Msg::PriorityChanged((value != "__all__" && !value.is_empty()).then_some(value))
        }));

        let unit =
            Combobox::new()
                .editable(true)
                .width(260)
                .items(Rc::new(
                    self.units.iter().cloned().map(AttrValue::from).collect(),
                ))
                .placeholder(tr!("All"))
                .disabled(self.filters.kernel)
                .value(self.filters.unit.clone().unwrap_or_default())
                .on_change(link.callback(|value: String| {
                    Msg::UnitChanged((!value.is_empty()).then_some(value))
                }));

        let identifier = Combobox::new()
            .editable(true)
            .width(260)
            .items(Rc::new(
                self.identifiers
                    .iter()
                    .cloned()
                    .map(AttrValue::from)
                    .collect(),
            ))
            .placeholder(tr!("e.g. pve* or postfix/*"))
            .disabled(self.filters.kernel)
            .value(self.filters.service.clone().unwrap_or_default())
            .on_change(link.callback(|value: String| {
                Msg::ServiceChanged((!value.is_empty()).then_some(value))
            }));

        let kernel = Checkbox::new()
            .checked(self.filters.kernel)
            .box_label(tr!("Kernel only"))
            .on_change(link.callback(Msg::KernelChanged));

        let reset = Button::new(tr!("Reset"))
            .disabled(self.filters == Filters::default())
            .onclick(link.callback(|_| Msg::ResetFilters));

        let labeled = |label: FieldLabel, field: Html| {
            Row::new()
                .gap(2)
                .class(pwt::css::AlignItems::Center)
                .with_child(label)
                .with_child(field)
        };

        Toolbar::new()
            .class("pwt-border-bottom")
            .class("pwt-gap-4")
            .with_child(labeled(
                FieldLabel::new(tr!("Minimum Priority")),
                priority.into(),
            ))
            .with_child(labeled(FieldLabel::new(tr!("Unit")), unit.into()))
            .with_child(labeled(
                FieldLabel::new(tr!("Identifier")),
                identifier.into(),
            ))
            .with_child(kernel)
            .with_flex_spacer()
            .with_child(reset)
            .into()
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
            // the shared stylesheet colors each priority class; no inline style here
            Some(html! {
                <span class={format!("pwt-journal-prio-{}", line.p)}>
                    {text}
                </span>
            })
        }
        JournalRecord::Control(ControlRecord::Reboot { .. }) => {
            // a full-width hairline separator marks the boot boundary; styled by the stylesheet
            Some(html! {
                <span class="pwt-journal-reboot" />
            })
        }
        JournalRecord::Control(ControlRecord::Host { h }) => {
            *host = Some(h);
            None
        }
        // cursors, identifiers, and units are stripped from the body by `split_structured`; they
        // never reach rendering
        _ => None,
    }
}

impl From<JournalView> for VNode {
    fn from(val: JournalView) -> Self {
        let key = val.key.clone();
        let comp = VComp::new::<ProxmoxJournalView>(Rc::new(val), key);
        VNode::from(comp)
    }
}
