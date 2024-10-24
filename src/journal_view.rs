use std::rc::Rc;

use anyhow::{format_err, Error};
use gloo_timers::callback::Timeout;
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
}

pub enum Msg {
    PageLoad(Vec<String>, Position),
    Scrolled(i32, i32, i32),
    VisibilityChanged(VisibilityContext),
    Error(Error),
}

enum JournalRequest {
    Initial(usize),
    Bottom(String),
    Top(usize, String),
}

#[derive(PartialEq, Clone, Copy)]
pub enum Position {
    Initial,
    Bottom,
    Top,
    Middle,
}

pub struct ProxmoxJournalView {
    cursors: Option<(String, String)>,
    lines: Vec<String>,
    log_ref: NodeRef,
    timeout: Option<Timeout>,
    position: Position,
    last_error: Option<Error>,
    old_scroll_height: i32,
    visibility: VisibilityContext,
    _visibility_context_observer: Option<ContextHandle<VisibilityContext>>,
    async_pool: AsyncPool,
}

async fn load_content(
    url: AttrValue,
    request: JournalRequest,
) -> Result<(Vec<String>, Position), Error> {
    let (param, response_type) = match request {
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

    let resp = crate::http_get_full::<Vec<String>>(url.to_string(), Some(param)).await?;

    Ok((resp.data, response_type))
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

                let msg = match load_content(props.url, request).await {
                    Ok((res, response_type)) => Msg::PageLoad(res, response_type),
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

        let mut this = Self {
            cursors: None,
            lines: Vec::new(),
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
            Msg::PageLoad(mut lines, response_type) => {
                self.timeout.take();
                if let Some(callback) = ctx.props().on_loading_change.clone() {
                    callback.emit((false, self.position == Position::Bottom));
                }
                if lines.len() < 2 {
                    ctx.link()
                        .send_message(Msg::Error(format_err!("invalid response: {:?}", lines)));
                    return false;
                }

                let (old_start, old_end) = if let Some((start, end)) = self.cursors.take() {
                    (Some(start), Some(end))
                } else {
                    (None, None)
                };
                let start_cursor = lines.remove(0);
                let end_cursor = lines.pop().unwrap();

                match response_type {
                    Position::Initial => {
                        self.cursors = Some((start_cursor, end_cursor));
                        self.lines = lines;
                    }
                    Position::Bottom => {
                        self.cursors = Some((old_start.unwrap_or(start_cursor), end_cursor));
                        if !lines.is_empty() {
                            self.lines.append(&mut lines);
                        }
                    }
                    Position::Top => {
                        self.cursors = Some((start_cursor, old_end.unwrap_or(end_cursor)));
                        if !lines.is_empty() {
                            lines.append(&mut self.lines);
                            self.lines = lines;
                        }
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
            .node_ref(self.log_ref.clone())
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

        for line in self.lines.iter() {
            log.add_child(format!("{line}\n"));
        }

        let error = self
            .last_error
            .as_ref()
            .map(|err| pwt::widget::error_message(&err.to_string()).border_top(true));

        Column::new()
            .class("pwt-flex-fit")
            .class(props.class.clone())
            .styles(props.style.clone())
            .with_child(log)
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

impl From<JournalView> for VNode {
    fn from(val: JournalView) -> Self {
        let key = val.key.clone();
        let comp = VComp::new::<ProxmoxJournalView>(Rc::new(val), key);
        VNode::from(comp)
    }
}
