use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use anyhow::Error;

use pwt::props::{
    AsClassesMut, AsCssStylesMut, ContainerBuilder, CssMarginBuilder, CssPaddingBuilder, CssStyles,
    WidgetBuilder, WidgetStyleBuilder,
};
use pwt::AsyncPool;
use serde::Deserialize;
use serde_json::json;

use gloo_timers::callback::{Interval, Timeout};

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::dom::DomSizeObserver;
use pwt::widget::Container;

use pwt_macros::builder;

// Note: virtual scrolling fails when log is large:
// See: https://bugs.chromium.org/p/chromium/issues/detail?id=932109
// See: https://bugzilla.mozilla.org/show_bug.cgi?id=1527883
// https://github.com/WICG/display-locking/issues/49
//
// Firefox shows wrong scrollbar, because it usese real client height
// instead of height property.

// possible solution: https://github.com/bvaughn/react-virtualized/issues/396

const MAX_PHYSICAL: f64 = 17_000_000.0;
const DEFAULT_LINE_HEIGHT: u64 = 18;

#[derive(Deserialize)]
struct LogEntry {
    #[allow(dead_code)]
    n: u64,
    t: String,
}

pub struct LogPage {
    page: u64,
    lines: Vec<LogEntry>,
    total: u64,
}

const PAGE_HEIGHT: u64 = 500;
const PAGE_LOAD_DELAY: u32 = 20; // Load delay in milliseconds

fn epoch_to_syslog_api(epoch: i64) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((epoch as f64) * 1000.0);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds(),
    )
}

async fn load_log_page(props: &LogView, page: u64) -> Result<LogPage, Error> {
    let mut param = json!({
        "start": page * PAGE_HEIGHT,
        "limit": PAGE_HEIGHT,
    });

    if let Some(service) = props.service.as_deref() {
        param["service"] = service.into();
    }

    if let Some(since) = props.since {
        param["since"] = epoch_to_syslog_api(since).into();
    }

    if let Some(until) = props.until {
        param["until"] = epoch_to_syslog_api(until).into();
    }

    let url = props.url.as_str();
    let resp = crate::http_get_full::<Vec<LogEntry>>(url, Some(param)).await?;

    let data_len = resp.data.len() as u64;
    let total = resp
        .attribs
        .get("total")
        .and_then(|v| v.as_u64())
        .unwrap_or(data_len);

    Ok(LogPage {
        page,
        lines: resp.data,
        total,
    })
}

#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct LogView {
    #[prop_or_default]
    node_ref: NodeRef,
    #[prop_or_default]
    pub key: Option<Key>,
    pub url: AttrValue,

    /// Determines if the log should auto refresh on the tail end of the log,
    /// useful for running task logs, etc.
    #[prop_or_default]
    #[builder]
    pub active: bool,

    #[prop_or_default]
    pub class: Classes,

    #[prop_or_default]
    pub style: CssStyles,

    /// View logs for the specified service,
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub service: Option<AttrValue>,

    /// Since when (unix epoch)
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub since: Option<i64>,

    /// Until when (unix epoch)
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub until: Option<i64>,

    /// Callback when the number of pending pages changes and if it's in "tail view" mode
    #[builder_cb(IntoEventCallback, into_event_callback, (usize, bool))]
    #[prop_or_default]
    pub on_pending_change: Option<Callback<(usize, bool)>>,
}

impl AsClassesMut for LogView {
    fn as_classes_mut(&mut self) -> &mut yew::Classes {
        &mut self.class
    }
}

impl AsCssStylesMut for LogView {
    fn as_css_styles_mut(&mut self) -> &mut CssStyles {
        &mut self.style
    }
}

impl CssMarginBuilder for LogView {}
impl CssPaddingBuilder for LogView {}
impl WidgetStyleBuilder for LogView {}

impl LogView {
    pub fn new(url: impl Into<AttrValue>) -> Self {
        yew::props!(Self { url: url.into() })
    }

    /// Builder style method to add a html class
    pub fn class(mut self, class: impl Into<Classes>) -> Self {
        self.add_class(class);
        self
    }

    /// Method to add a html class
    pub fn add_class(&mut self, class: impl Into<Classes>) {
        self.class.push(class);
    }
}

pub enum Msg {
    ScrollTo(i32, i32, bool),
    ViewportResize(f64, f64),
    PageLoad(LogPage),
    TailView,
    Reload,
}

pub struct PwtLogView {
    pages: [Option<LogPage>; 4],
    pending_pages: HashMap<u64, Timeout>,
    required_pages: HashSet<u64>,
    total: Option<u64>,
    viewport_ref: NodeRef,
    page_ref: NodeRef,
    viewport_lines: u64,
    scroll_top: i32,

    // Note: We just do stupid scaleing top avoid browser scrolling bugs.
    // This is probably good enouth until scale gets larger than line_height...
    scale: f64,

    size_observer: Option<DomSizeObserver>,

    tailview_trigger: Option<Interval>,

    enable_tail_view: bool,

    line_height: Option<u64>,

    async_pool: AsyncPool,
}

impl PwtLogView {
    fn line_height(&self) -> u64 {
        self.line_height.unwrap_or(DEFAULT_LINE_HEIGHT)
    }

    fn physical_to_logical(&self, physical: i32) -> u64 {
        (physical as f64 * self.scale) as u64
    }

    fn logical_to_physical(&self, logical: u64) -> i32 {
        (logical as f64 / self.scale) as i32
    }

    fn page_index(&self, page: u64) -> Option<usize> {
        self.pages.iter().position(|item| match item {
            Some(item) => item.page == page,
            None => false,
        })
    }

    fn emit_pending_change(&self, ctx: &Context<Self>) {
        let props = ctx.props();
        if let Some(handler) = props.on_pending_change.clone() {
            handler.emit((self.pending_pages.keys().count(), self.enable_tail_view));
        }
    }

    fn request_page(&mut self, ctx: &Context<Self>, page_num: u64, delay: u32) {
        if let Entry::Vacant(e) = self.pending_pages.entry(page_num) {
            let props = ctx.props().clone();
            let link = ctx.link().clone();
            let async_pool = self.async_pool.clone();
            //log::info!("REQUEST {}", page_num);
            let timeout = Timeout::new(delay, move || {
                async_pool.spawn(async move {
                    match load_log_page(&props, page_num).await {
                        Ok(page) => link.send_message(Msg::PageLoad(page)),
                        Err(err) => {
                            log::error!("Page load failed: {}", err);
                        }
                    }
                });
            });
            e.insert(timeout);
            self.emit_pending_change(ctx);
        }
    }

    fn request_pages(&mut self, ctx: &Context<Self>) {
        let last_page = match self.total {
            Some(total) => total / PAGE_HEIGHT,
            None => 0,
        };

        if self.enable_tail_view {
            self.pending_pages.retain(|page, _| *page == last_page);
            self.emit_pending_change(ctx);
            self.required_pages.clear();
            self.required_pages.insert(last_page);
            // in case we're just on a page boundary, we need the previous page too
            if last_page > 0 {
                self.required_pages.insert(last_page - 1);
                if self.page_index(last_page - 1).is_none() {
                    self.request_page(ctx, last_page - 1, PAGE_LOAD_DELAY);
                }
            }
            let delay = if self.page_index(last_page).is_some() {
                1000
            } else {
                PAGE_LOAD_DELAY
            };

            self.request_page(ctx, last_page, delay);
            return;
        }

        let line = self.physical_to_logical(self.scroll_top) / self.line_height();

        let prev = if line > 100 {
            (line - 100) / PAGE_HEIGHT
        } else {
            0
        };
        let start = line / PAGE_HEIGHT;
        let end = (line + self.viewport_lines) / PAGE_HEIGHT;
        let next = if line + 100 < self.total.unwrap_or(0) {
            (line + self.viewport_lines + 100) / PAGE_HEIGHT
        } else {
            end
        };
        //log::info!("REQUEST PAGES {} {} {} {} {}", line, prev, start, end, next);

        let mut required_pages: HashSet<u64> = HashSet::new();

        for page_num in [start, end, prev, next] {
            required_pages.insert(page_num);
            if self.page_index(page_num).is_none() {
                self.request_page(ctx, page_num, PAGE_LOAD_DELAY);
            }
        }

        self.pending_pages.retain(|p, _| required_pages.contains(p));
        self.required_pages = required_pages;
    }

    fn insert_page(&mut self, info: LogPage) {
        if let Some(index) = self.page_index(info.page) {
            //log::info!("REPLACE PAGE {} at {}", info.page, index);
            self.pages[index] = Some(info);
        } else {
            let mut found = false;
            for i in 0..self.pages.len() {
                if self.pages[i].is_none() {
                    //log::info!("INSERT PAGE {} at {}", info.page, i);
                    self.pages[i] = Some(info);
                    found = true;
                    break;
                }
            }
            if !found {
                log::error!("no empty page slot");
            }
        }

        // sort, so the order of the elements of pages is stable
        self.pages.sort_by(|page_a, page_b| match (page_a, page_b) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(page_a), Some(page_b)) => page_a.page.cmp(&page_b.page),
        });
    }
}

impl Component for PwtLogView {
    type Message = Msg;
    type Properties = LogView;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::TailView);
        let tailview_trigger = Interval::new(1000, {
            let link = ctx.link().clone();
            move || {
                link.send_message(Msg::TailView);
            }
        });

        Self {
            pages: [None, None, None, None],
            pending_pages: HashMap::new(),
            viewport_ref: NodeRef::default(),
            page_ref: NodeRef::default(),
            total: None,
            viewport_lines: 0,
            scroll_top: 0,
            size_observer: None,
            tailview_trigger: Some(tailview_trigger),
            enable_tail_view: ctx.props().active,
            // Note: we use window.get_computed_style() to get the real value in rendered()
            line_height: None,
            scale: 1.0,
            required_pages: HashSet::new(),
            async_pool: AsyncPool::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Reload => {
                self.pages = [None, None, None, None];
                self.pending_pages.clear();
                self.emit_pending_change(ctx);
                self.required_pages.clear();
                self.total = None;
                self.request_pages(ctx);
                false
            }
            Msg::ScrollTo(_x, y, at_end) => {
                self.scroll_top = y;
                self.request_pages(ctx);

                if self.enable_tail_view {
                    if !at_end {
                        self.enable_tail_view = false;
                    }
                } else if at_end && ctx.props().active {
                    self.enable_tail_view = true;
                }

                true
            }
            Msg::ViewportResize(_width, height) => {
                let lines = (height as u64 + self.line_height() - 1) / self.line_height();
                self.viewport_lines = lines;
                self.request_pages(ctx);
                true
            }
            Msg::PageLoad(info) => {
                let total = info.total;
                self.total = Some(total);
                let scale = (total as f64 * self.line_height() as f64) / MAX_PHYSICAL;
                self.scale = scale.max(1.0);
                //log::info!("SCALE1 {}", self.scale);

                self.pending_pages.remove(&info.page);
                self.emit_pending_change(ctx);

                if !self.required_pages.contains(&info.page) {
                    //log::info!("SKIP PageLoad {}", info.page);
                    return false;
                }

                // remove stale pages
                for i in 0..self.pages.len() {
                    if let Some(page) = &self.pages[i] {
                        if !self.required_pages.contains(&page.page) {
                            //log::info!("remove stale page {}", page.page);
                            self.pages[i] = None;
                        }
                    }
                }

                self.insert_page(info);

                true
            }
            Msg::TailView => {
                if !self.enable_tail_view {
                    return false;
                }
                self.request_pages(ctx);
                if !ctx.props().active {
                    //log::info!("STOP TAIL VIEW");
                    self.enable_tail_view = false;
                    if let Some(trigger) = self.tailview_trigger.take() {
                        trigger.cancel();
                    }
                }
                false
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if props.since != old_props.since
            || props.until != old_props.until
            || props.service != old_props.service
        {
            ctx.link().send_message(Msg::Reload);
        }
        true
    }
    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let lines = self.total.unwrap_or(0);

        let mut page_ref = Some(self.page_ref.clone());
        let pages: Html = self
            .pages
            .iter()
            .filter_map(|page| {
                match page {
                    Some(page) => {
                        let offset = page.page * PAGE_HEIGHT * self.line_height();
                        let offset = self.logical_to_physical(offset);
                        //log::info!("render PAGE {} AT OFFSET {}", page.page, offset);

                        let mut tag = Container::new()
                            .key(format!("page{}", page.page))
                            .class("pwt-log-content")
                            .style("position", "absolute")
                            .style("top", format!("{}px", offset));

                        if let Some(line_height) = &self.line_height {
                            tag.set_style("line-height", format!("{line_height}px"));
                        }

                        if let Some(page_ref) = page_ref.take() {
                            tag.set_node_ref(page_ref);
                        }

                        for item in page.lines.iter() {
                            tag.add_child(format!("{}\n", item.t));
                        }

                        let html: Html = tag.into();
                        Some(html)
                    }
                    None => None,
                }
            })
            .collect();

        let viewport_ref = self.viewport_ref.clone();
        let onscroll = ctx.link().batch_callback(move |_: Event| {
            if let Some(el) = viewport_ref.cast::<web_sys::Element>() {
                let top = el.scroll_top();
                let left = el.scroll_left();
                let height = el.scroll_height();
                let client_height = el.client_height();

                let at_end = (height - top - client_height) <= 3;

                Some(Msg::ScrollTo(left, top, at_end))
            } else {
                None
            }
        });

        let class = classes! {
            "pwt-log",
            "pwt-overflow-auto",
            props.class.clone(),
        };

        let style = props.style.compile_style_attribute(None);

        let physical_height = self.logical_to_physical(lines * self.line_height());
        html! {
            // Note: we set class "pwt-log-content" her, so that we can query the font size
            <div ref={self.viewport_ref.clone()} {style} {class} {onscroll}>
               <div style={format!("height:{}px;position:relative;", physical_height)}>
                 {pages}
               </div>
            </div>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            if let Some(el) = self.viewport_ref.cast::<web_sys::Element>() {
                let link = ctx.link().clone();
                let size_observer = DomSizeObserver::new(&el, move |(width, height)| {
                    link.send_message(Msg::ViewportResize(width, height));
                });

                self.size_observer = Some(size_observer);
            }
        }
        if self.line_height.is_none() {
            if let Some(el) = self.page_ref.cast::<web_sys::Element>() {
                // get font size in pixels
                let window = web_sys::window().unwrap();
                if let Ok(Some(style)) = window.get_computed_style(&el) {
                    if let Ok(line_height) = style.get_property_value("line-height") {
                        let line_height = line_height.trim_end_matches("px");
                        if let Ok(line_height) = line_height.parse::<f64>() {
                            self.line_height = Some(line_height as u64);
                        }
                    }
                }
            }
        }
        if self.enable_tail_view {
            let top = match self.total {
                Some(total) => {
                    if total > self.viewport_lines {
                        (total - self.viewport_lines + self.line_height() - 1) * self.line_height()
                    } else {
                        0
                    }
                }
                None => 0,
            };

            if let Some(el) = self.viewport_ref.cast::<web_sys::Element>() {
                //log::info!("SCROLLTO {}", top);
                el.set_scroll_top(top as i32);
            };
        }
    }
}

impl From<LogView> for VNode {
    fn from(val: LogView) -> Self {
        let key = val.key.clone();
        let comp = VComp::new::<PwtLogView>(Rc::new(val), key);
        VNode::from(comp)
    }
}
