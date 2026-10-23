use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use gloo_timers::callback::{Interval, Timeout};
use serde_json::{Map, Value};
use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::css::ColorScheme;
use pwt::prelude::*;
use pwt::state::{PersistentState, Selection, Store};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader, DataTableKeyboardEvent, DataTableMouseEvent,
    DataTableRowRenderCallback,
};
use pwt::widget::form::{Combobox, DateField, Field, Form, FormContext, InputType, PlainDate};
use pwt::widget::menu::{Menu, MenuButton, MenuItem};
use pwt::widget::{Button, Column, Container, Dialog, Mask, SegmentedButton, Toolbar, Tooltip};

use pwt_macros::builder;

use proxmox_access_log::{AccessLogEntry, QueryResult};

use crate::utils::{date_time_to_epoch, render_epoch};
use crate::{
    LoadableComponent, LoadableComponentContext, LoadableComponentMaster,
    LoadableComponentScopeExt, LoadableComponentState,
};

const FILTER_UPDATE_BUFFER_MS: u32 = 300;
const BATCH_LIMIT: u64 = 500;
const LOAD_BUFFER_ROWS: usize = 20;
/// How often live mode checks for new requests.
const LIVE_INTERVAL_MS: u32 = 3000;
/// Live mode starts over after adding that many requests, so an open view does not grow forever.
const LIVE_LIMIT: usize = 5000;
/// The filters that take a text, named like their form field and API parameter.
const TEXT_FILTERS: [&str; 5] = ["search", "auth-id", "client", "method", "path"];

/// Shows the API access log of a node, newest requests first. Live mode follows the log, a
/// timespan shows the requests logged in it, and scrolling down loads older requests.
#[derive(PartialEq, Properties)]
#[builder]
pub struct AccessLog {
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub nodename: Option<AttrValue>,

    /// The base url, default is `/nodes/<nodename>/access-log`.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub base_url: Option<AttrValue>,
}

impl Default for AccessLog {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessLog {
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    fn url(&self) -> String {
        match &self.base_url {
            Some(url) => url.to_string(),
            None => format!(
                "/nodes/{}/access-log",
                self.nodename.as_deref().unwrap_or("localhost")
            ),
        }
    }
}

/// An entry with a key, as requests can be logged several times with the same content.
#[derive(Clone, PartialEq)]
pub struct Row {
    key: usize,
    entry: AccessLogEntry,
}

/// The identity of an ordinary page request, including the filter snapshot it continues.
pub struct PageRequest {
    generation: usize,
    filter: Value,
    cursor: Option<String>,
}

pub enum Msg {
    ToggleFilter,
    UpdateFilter,
    /// Starts over with the newest page, applying the timespan.
    Refresh,
    LoadOlder,
    PageLoaded {
        request: PageRequest,
        result: Result<QueryResult, String>,
    },
    ChangeMode(bool),
    SinceDate(Option<PlainDate>),
    SinceTime(String),
    UntilDate(Option<PlainDate>),
    UntilTime(String),
    /// Live mode checks for new requests.
    Poll,
    /// The requests logged after the head cursor of the rows of this generation.
    LoadedNewer {
        generation: usize,
        result: QueryResult,
    },
    NewerFailed {
        generation: usize,
        busy: bool,
    },
    /// Whether the table got scrolled to its top.
    Scrolled(bool),
    SelectionChange,
    /// Sets the filter field of this name to a value of the selected request.
    FilterBy(&'static str, String),
}

pub struct ProxmoxAccessLog {
    state: LoadableComponentState<ViewState>,
    selection: Selection,
    store: Store<Row>,
    next_key: usize,
    show_filter: PersistentState<bool>,
    filter_form_context: FormContext,
    row_render_callback: DataTableRowRenderCallback<Row>,
    columns: Rc<Vec<DataTableHeader<Row>>>,
    // the filter to load the first page with, and the one of the rows shown, which older pages
    // continue with also while the filter changes
    filter: Value,
    shown_filter: Value,
    load_timeout: Option<Timeout>,
    load_error: Option<String>,
    load_older: bool,
    next_cursor: Option<String>,
    head_cursor: Option<String>,
    // counts the refreshes and first pages loaded, to drop new requests of earlier rows
    generation: usize,
    newer_running: bool,
    live_rows: usize,
    older_pending: bool,
    restart_pending: bool,
    _visibility_listener: gloo_events::EventListener,
    live: bool,
    _live_interval: Interval,
    at_top: bool,
    view_ref: NodeRef,
    since: PlainDate,
    since_time: String,
    since_label_id: AttrValue,
    until: PlainDate,
    until_time: String,
    until_label_id: AttrValue,
    searched_back_to: Option<i64>,
    gap_files: Vec<String>,
    invalid_lines: u64,
}

pwt::impl_deref_mut_property!(ProxmoxAccessLog, state, LoadableComponentState<ViewState>);

#[derive(PartialEq)]
pub enum ViewState {
    Request(AccessLogEntry),
}

impl ProxmoxAccessLog {
    fn is_current_request(&self, request: &PageRequest) -> bool {
        request.generation == self.generation
            && match &request.cursor {
                Some(_) => {
                    request.cursor == self.next_cursor && request.filter == self.shown_filter
                }
                None => request.filter == self.filter,
            }
    }

    fn columns() -> Rc<Vec<DataTableHeader<Row>>> {
        Rc::new(vec![
            DataTableColumn::new(tr!("Time"))
                .width("160px")
                .render(|row: &Row| render_epoch(row.entry.time).into())
                .into(),
            DataTableColumn::new(tr!("User"))
                .width("minmax(150px, 0.4fr)")
                .render(|row: &Row| html! { row.entry.auth_id.clone().unwrap_or_default() })
                .into(),
            DataTableColumn::new(tr!("Client"))
                .width("minmax(130px, 0.3fr)")
                .render(|row: &Row| {
                    html! { row.entry.client.map(|ip| ip.to_string()).unwrap_or_default() }
                })
                .into(),
            DataTableColumn::new(tr!("Method"))
                .width("80px")
                .render(|row: &Row| html! { row.entry.method.clone().unwrap_or_default() })
                .into(),
            DataTableColumn::new(tr!("Path"))
                .flex(1)
                .render(|row: &Row| html! { row.entry.uri.clone().unwrap_or_default() })
                .into(),
            DataTableColumn::new(tr!("Status"))
                .width("90px")
                .render(|row: &Row| match row.entry.status {
                    Some(status) => {
                        let class = match status {
                            400..=499 => {
                                classes!(ColorScheme::WarningContainer, "pwt-default-colors")
                            }
                            500..=599 => {
                                classes!(ColorScheme::ErrorContainer, "pwt-default-colors")
                            }
                            _ => Classes::new(),
                        };
                        html! {<span {class}>{status}</span>}
                    }
                    None => html! { tr!("Unknown") },
                })
                .into(),
            DataTableColumn::new(tr!("Size"))
                .width("90px")
                .justify("end")
                .hidden(true)
                .render(|row: &Row| match row.entry.size {
                    Some(size) => {
                        html! { proxmox_human_byte::HumanByte::from(size).to_string() }
                    }
                    None => html! {},
                })
                .into(),
            DataTableColumn::new(tr!("User Agent"))
                .width("minmax(150px, 0.4fr)")
                .hidden(true)
                .render(|row: &Row| html! { row.entry.user_agent.clone().unwrap_or_default() })
                .into(),
        ])
    }

    /// The API parameters of the filter form and the timespan, without the limit.
    fn filter_params(&self) -> Value {
        let form = self.filter_form_context.read();
        let mut params = Map::new();

        for name in TEXT_FILTERS {
            let text = form.get_field_text(name);
            let value = text.trim();
            if !value.is_empty() {
                params.insert(name.into(), value.into());
            }
        }
        if !self.live {
            if let Some(since) = date_time_to_epoch(&self.since, &self.since_time) {
                params.insert("since".into(), since.into());
            }
            // the fields have minutes, and the selected one counts until its end
            if let Some(until) = date_time_to_epoch(&self.until, &self.until_time) {
                params.insert("until".into(), (until + 59).into());
            }
        }
        let (min, max) = match form.get_field_text("status").as_str() {
            "success" => (None, Some(399)),
            "error" => (Some(400), None),
            "client-error" => (Some(400), Some(499)),
            "server-error" => (Some(500), None),
            _ => (None, None),
        };
        if let Some(min) = min {
            params.insert("min-status".into(), min.into());
        }
        if let Some(max) = max {
            params.insert("max-status".into(), max.into());
        }

        Value::Object(params)
    }

    fn rows(&mut self, entries: Vec<AccessLogEntry>) -> Vec<Row> {
        entries
            .into_iter()
            .map(|entry| {
                self.next_key += 1;
                Row {
                    key: self.next_key,
                    entry,
                }
            })
            .collect()
    }

    fn add_problems(&mut self, result: &QueryResult) {
        self.invalid_lines += result.invalid_lines;
        for gap in &result.gaps {
            if !self.gap_files.contains(&gap.file) {
                self.gap_files.push(gap.file.clone());
            }
        }
    }

    /// Reading problems, for the tooltip of a warning icon.
    fn problems(&self) -> Option<String> {
        let mut texts = Vec::new();
        if !self.gap_files.is_empty() {
            texts.push(tr!("Could not read all of {0}.", self.gap_files.join(", ")));
        }
        if self.invalid_lines > 0 {
            texts.push(tr!(
                "Skipped one invalid line." | "Skipped {n} invalid lines." % self.invalid_lines
            ));
        }
        (!texts.is_empty()).then(|| texts.join(" "))
    }

    /// Whether the view is shown, also in a browser tab that is not hidden.
    fn visible(&self) -> bool {
        !gloo_utils::document().hidden()
            && self
                .view_ref
                .cast::<web_sys::HtmlElement>()
                .is_some_and(|el| el.offset_parent().is_some())
    }

    /// Live mode starts over with the newest page, but not while the view is hidden, which then
    /// waits for the next poll.
    fn restart(&mut self, ctx: &LoadableComponentContext<Self>) {
        match self.visible() {
            true => ctx.link().send_message(Msg::Refresh),
            false => self.restart_pending = true,
        }
    }

    /// A menu to narrow the shown requests to those sharing a value with the selected one.
    fn filter_by_menu(&self, ctx: &LoadableComponentContext<Self>) -> MenuButton {
        let row = self.store.selected_record(&self.selection);
        let mut menu = Menu::new();
        if let Some(Row { entry, .. }) = &row {
            // without the query, which the path filter does not look at
            let path = entry
                .uri
                .as_deref()
                .map(|uri| uri.split('?').next().unwrap_or(uri).to_string());
            for (name, value) in [
                ("client", entry.client.map(|ip| ip.to_string())),
                ("auth-id", entry.auth_id.clone()),
                ("path", path),
            ] {
                let Some(value) = value else { continue };
                let shown = match value.chars().count() > 60 {
                    true => value.chars().take(57).collect::<String>() + "...",
                    false => value.clone(),
                };
                let text = match name {
                    "client" => tr!("Client '{0}'", shown),
                    "auth-id" => tr!("User '{0}'", shown),
                    _ => tr!("Path '{0}'", shown),
                };
                menu.add_item(
                    MenuItem::new(text).icon_class("fa fa-filter").on_select(
                        ctx.link()
                            .callback(move |_| Msg::FilterBy(name, value.clone())),
                    ),
                );
            }
        }
        MenuButton::new(tr!("Filter by"))
            .show_arrow(true)
            .disabled(row.is_none())
            .menu(menu)
    }

    fn timespan_fields(&self, ctx: &LoadableComponentContext<Self>) -> Vec<Html> {
        let label = |id: &AttrValue, text: String| {
            Container::from_tag("label")
                .id(id.clone())
                .padding_start(2)
                .class("pwt-align-self-center")
                .with_child(text)
                .into()
        };
        let date = |id: &AttrValue, value: &PlainDate, cb: Callback<Option<PlainDate>>| {
            Container::new()
                .style("width", "150px")
                .with_child(
                    DateField::new()
                        .label_id(id.clone())
                        .required(true)
                        .on_change(cb)
                        .value(value.to_string()),
                )
                .into()
        };
        let time = |id: &AttrValue, value: &str, cb: Callback<String>| {
            Container::new()
                .style("width", "100px")
                .with_child(
                    Field::new()
                        .label_id(id.clone())
                        .input_type(InputType::Time)
                        .required(true) // avoid clear button in firefox
                        .class("pwt-input-hide-clear-button")
                        .on_change(cb)
                        .value(value.to_string()),
                )
                .into()
        };
        let link = ctx.link();
        vec![
            label(&self.since_label_id, tr!("Since") + ":"),
            date(
                &self.since_label_id,
                &self.since,
                link.callback(Msg::SinceDate),
            ),
            time(
                &self.since_label_id,
                &self.since_time,
                link.callback(Msg::SinceTime),
            ),
            label(&self.until_label_id, tr!("Until") + ":"),
            date(
                &self.until_label_id,
                &self.until,
                link.callback(Msg::UntilDate),
            ),
            time(
                &self.until_label_id,
                &self.until_time,
                link.callback(Msg::UntilTime),
            ),
            Button::new(tr!("Update"))
                .onclick(link.callback(|_| Msg::Refresh))
                .into(),
        ]
    }
}

impl LoadableComponent for ProxmoxAccessLog {
    type Properties = AccessLog;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let store = Store::with_extract_key(|row: &Row| Key::from(row.key));
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::SelectionChange));

        let filter_form_context =
            FormContext::new().on_change(ctx.link().callback(|_| Msg::UpdateFilter));

        let row_render_callback = DataTableRowRenderCallback::new({
            let store = store.clone();
            let link = ctx.link().clone();
            move |args: &mut _| {
                if args.row_index() > store.data_len().saturating_sub(LOAD_BUFFER_ROWS) {
                    link.send_message(Msg::LoadOlder);
                }
            }
        });

        let live_interval = {
            let link = ctx.link().clone();
            Interval::new(LIVE_INTERVAL_MS, move || link.send_message(Msg::Poll))
        };
        // catch up without waiting for the polling interval
        let visibility_listener = {
            let link = ctx.link().clone();
            gloo_events::EventListener::new(
                &gloo_utils::document(),
                "visibilitychange",
                move |_| link.send_message(Msg::Poll),
            )
        };

        Self {
            state: LoadableComponentState::new(),
            selection,
            store,
            next_key: 0,
            show_filter: PersistentState::new("ProxmoxAccessLogShowFilter"),
            filter_form_context,
            row_render_callback,
            columns: Self::columns(),
            filter: Value::Object(Map::new()),
            shown_filter: Value::Object(Map::new()),
            load_timeout: None,
            load_error: None,
            load_older: false,
            next_cursor: None,
            head_cursor: None,
            generation: 0,
            newer_running: false,
            live_rows: 0,
            older_pending: false,
            restart_pending: false,
            _visibility_listener: visibility_listener,
            live: true,
            _live_interval: live_interval,
            at_top: true,
            view_ref: NodeRef::default(),
            since: PlainDate::today(),
            since_time: "00:00".to_string(),
            since_label_id: AttrValue::from(pwt::widget::get_unique_element_id()),
            until: PlainDate::today(),
            until_time: "23:59".to_string(),
            until_label_id: AttrValue::from(pwt::widget::get_unique_element_id()),
            searched_back_to: None,
            gap_files: Vec::new(),
            invalid_lines: 0,
        }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let url = ctx.props().url();
        let (filter, cursor) = match self.load_older {
            true => (self.shown_filter.clone(), self.next_cursor.clone()),
            false => (self.filter.clone(), None),
        };

        let mut params = filter.clone();
        params["limit"] = BATCH_LIMIT.into();
        if let Some(cursor) = &cursor {
            params["cursor"] = cursor.as_str().into();
        }

        let request = PageRequest {
            generation: self.generation,
            filter,
            cursor,
        };
        let link = ctx.link().clone();
        Box::pin(async move {
            let result = crate::http_get(&url, Some(params))
                .await
                .map_err(|err| err.to_string());
            link.send_message(Msg::PageLoaded { request, result });
            // errors get handled along with the request they are of, so stale ones do not show
            Ok(())
        })
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::ToggleFilter => {
                self.show_filter.update(!*self.show_filter);
                true
            }
            Msg::UpdateFilter => {
                if !self.filter_form_context.read().is_valid() {
                    return false;
                }
                let filter = self.filter_params();
                if filter == self.filter {
                    return false;
                }
                self.filter = filter;
                let link = ctx.link().clone();
                self.load_timeout = Some(Timeout::new(FILTER_UPDATE_BUFFER_MS, move || {
                    link.send_message(Msg::Refresh);
                }));
                true
            }
            Msg::Refresh => {
                if self.filter_form_context.read().is_valid() {
                    self.filter = self.filter_params();
                }
                self.load_timeout = None;
                self.load_older = false;
                self.older_pending = false;
                self.generation += 1;
                self.newer_running = false;
                self.restart_pending = false;
                // rows of another filter or timespan would look like a result while loading
                if self.filter != self.shown_filter {
                    self.store.write().clear();
                    self.next_cursor = None;
                    self.head_cursor = None;
                }
                ctx.link().send_reload();
                true
            }
            Msg::ChangeMode(live) => {
                self.live = live;
                ctx.link().send_message(Msg::Refresh);
                true
            }
            Msg::SinceDate(Some(date)) => {
                self.since = date;
                true
            }
            Msg::SinceTime(time) => {
                self.since_time = time;
                true
            }
            Msg::UntilDate(Some(date)) => {
                self.until = date;
                true
            }
            Msg::UntilTime(time) => {
                self.until_time = time;
                true
            }
            Msg::SinceDate(None) | Msg::UntilDate(None) => false,
            Msg::LoadOlder => {
                if self.next_cursor.is_none() {
                    self.older_pending = false;
                    return false;
                }
                if self.load_older {
                    return false;
                }
                // a pending filter change is about to start over; obsolete loads can also block
                // continuation, so keep it pending until the wrapper finishes them
                if self.loading() || self.load_timeout.is_some() || !self.visible() {
                    self.older_pending = true;
                    return false;
                }
                self.older_pending = false;
                self.load_older = true;
                ctx.link().send_reload();
                false
            }
            Msg::PageLoaded { request, result } => {
                if !self.is_current_request(&request) {
                    return false;
                }
                let PageRequest { filter, cursor, .. } = request;
                let mut result = match result {
                    Ok(result) => result,
                    Err(error) => {
                        if self.load_error.is_none() {
                            ctx.link().show_error(tr!("Load failed"), &error, false);
                        }
                        self.load_error = Some(error);
                        if cursor.is_none() {
                            self.store.write().clear();
                            self.shown_filter = filter;
                            self.next_cursor = None;
                            self.head_cursor = None;
                            self.generation += 1;
                            self.searched_back_to = None;
                            self.gap_files.clear();
                            self.invalid_lines = 0;
                        }
                        // let scrolling try a failed older page again
                        self.load_older = false;
                        self.older_pending = false;
                        return true;
                    }
                };
                let older = cursor.is_some();
                self.load_error = None;
                let empty = result.entries.is_empty();
                let rows = self.rows(std::mem::take(&mut result.entries));
                if older {
                    self.store.write().extend(rows);
                    if result.searched_back_to.is_some() {
                        self.searched_back_to = result.searched_back_to;
                    }
                } else {
                    self.shown_filter = filter;
                    self.store.write().set_data(rows);
                    self.generation += 1;
                    self.newer_running = false;
                    self.live_rows = 0;
                    self.head_cursor = result.head_cursor.clone();
                    self.searched_back_to = result.searched_back_to;
                    self.gap_files.clear();
                    self.invalid_lines = 0;
                }
                self.add_problems(&result);
                self.next_cursor = result.next_cursor;
                self.load_older = false;
                // without rows, there is nothing to scroll down to
                if empty || self.store.data_len() < LOAD_BUFFER_ROWS {
                    ctx.link().send_message(Msg::LoadOlder);
                }
                true
            }
            Msg::Poll => {
                if self.older_pending
                    && self.next_cursor.is_some()
                    && !self.loading()
                    && self.visible()
                {
                    // alone, as a refresh sent along would make both reloads load the first page
                    ctx.link().send_message(Msg::LoadOlder);
                    return false;
                }
                // older pages can load meanwhile, at the other end of the rows
                if !self.live
                    || !self.at_top
                    || self.newer_running
                    || (self.loading() && !self.load_older)
                    || self.load_timeout.is_some()
                    || !self.visible()
                {
                    return false;
                }
                if self.restart_pending {
                    ctx.link().send_message(Msg::Refresh);
                    return false;
                }
                let Some(head) = self.head_cursor.clone() else {
                    if !self.loading() && self.load_error.is_none() {
                        // the log has no line yet, or ends in an unfinished overlong one
                        ctx.link().send_message(Msg::Refresh);
                    }
                    return false;
                };
                self.newer_running = true;
                let url = ctx.props().url();
                let mut params = self.shown_filter.clone();
                params["limit"] = BATCH_LIMIT.into();
                params["after"] = head.into();
                let generation = self.generation;
                let link = ctx.link().clone();
                self.spawn(async move {
                    match crate::http_get(&url, Some(params)).await {
                        Ok(result) => link.send_message(Msg::LoadedNewer { generation, result }),
                        Err(err) => link.send_message(Msg::NewerFailed {
                            generation,
                            busy: busy(&err),
                        }),
                    }
                });
                false
            }
            Msg::LoadedNewer {
                generation,
                mut result,
            } => {
                if generation != self.generation {
                    return false;
                }
                self.newer_running = false;
                self.live_rows += result.entries.len();
                if !result.complete || self.live_rows > LIVE_LIMIT {
                    self.restart(ctx);
                    return false;
                }
                let mut rows = self.rows(std::mem::take(&mut result.entries));
                if !rows.is_empty() {
                    let mut store = self.store.write();
                    rows.extend(store.drain(..));
                    store.set_data(rows);
                }
                self.head_cursor = result.head_cursor.clone();
                self.add_problems(&result);
                true
            }
            Msg::NewerFailed { generation, busy } => {
                if generation != self.generation {
                    return false;
                }
                // the server is busy with other requests to the log, the next poll tries again
                if busy {
                    self.newer_running = false;
                    return false;
                }
                // the part of the log after the head can be gone after rotations, so start over,
                // which shows the error if it persists
                self.newer_running = false;
                self.restart(ctx);
                false
            }
            Msg::Scrolled(at_top) => {
                self.at_top = at_top;
                false
            }
            Msg::SelectionChange => true,
            Msg::FilterBy(name, value) => {
                self.filter_form_context
                    .write()
                    .set_field_value(name, value.into());
                self.show_filter.update(true);
                true
            }
        }
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let filter_icon_class = if *self.show_filter {
            "fa fa-filter pwt-color-primary"
        } else {
            "fa fa-filter"
        };
        let dirty_count = self.filter_form_context.read().dirty_count();

        let toolbar = Toolbar::new()
            .key("main-toolbar")
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                SegmentedButton::new()
                    .with_button(
                        Button::new(tr!("Live Mode"))
                            .pressed(self.live)
                            .onclick(ctx.link().callback(|_| Msg::ChangeMode(true))),
                    )
                    .with_button(
                        Button::new(tr!("Select Timespan"))
                            .pressed(!self.live)
                            .onclick(ctx.link().callback(|_| Msg::ChangeMode(false))),
                    ),
            )
            .with_flex_spacer()
            .with_optional_child((self.loading() && self.load_older).then(|| {
                Container::from_tag("i")
                    .class("pwt-loading-icon")
                    .class("pwt-align-self-center")
            }))
            .with_optional_child(self.problems().map(|problems| {
                Tooltip::new(
                    Container::from_tag("i")
                        .class("fa fa-exclamation-triangle pwt-color-warning")
                        .padding(2),
                )
                .tip(problems)
            }))
            .with_child(self.filter_by_menu(ctx))
            .with_child(
                Container::new()
                    .style("flex", "0 1 250px")
                    .style("min-width", "120px")
                    .with_child(
                        Field::new()
                            .name("search")
                            .aria_label(tr!("Search client, user or path"))
                            .placeholder(tr!("Search client, user or path"))
                            .validate(max_length(1024)),
                    ),
            )
            .with_child({
                let form_context = self.filter_form_context.clone();
                Button::new(tr!("Clear Filter ({})", dirty_count))
                    .disabled(dirty_count == 0)
                    .onclick(move |_| form_context.write().reset_form())
            })
            .with_child(
                Button::new(tr!("Filter"))
                    .icon_class(filter_icon_class)
                    .onclick(ctx.link().callback(|_| Msg::ToggleFilter)),
            );

        let filter_classes = classes!(
            "pwt-overflow-auto",
            "pwt-border-bottom",
            "pwt-gap-2",
            "pwt-align-items-baseline",
            if *self.show_filter {
                "pwt-d-grid"
            } else {
                "pwt-d-none"
            },
        );

        let label = |text: String| html! {<div class="pwt-text-align-end">{text}</div>};
        let filter = Container::new()
            .key("filter")
            .class(filter_classes)
            .padding(4)
            .style(
                "grid-template-columns",
                "auto minmax(150px, 300px) auto minmax(150px, 300px) auto minmax(150px, 360px) 1fr",
            )
            .with_child(label(tr!("User")))
            .with_child(
                Field::new()
                    .name("auth-id")
                    .aria_label(tr!("User"))
                    .placeholder(tr!("All"))
                    .validate(max_length(1024)),
            )
            .with_child(label(tr!("Method")))
            .with_child(
                Combobox::new()
                    .name("method")
                    .aria_label(tr!("Method"))
                    .editable(true)
                    .placeholder(tr!("All"))
                    .items(Rc::new(
                        ["GET", "POST", "PUT", "DELETE"]
                            .into_iter()
                            .map(AttrValue::from)
                            .collect(),
                    )),
            )
            .with_child(label(tr!("Path")))
            .with_child(
                Field::new()
                    .name("path")
                    .aria_label(tr!("Path"))
                    .placeholder(tr!("Contains"))
                    .validate(max_length(1024)),
            )
            .with_child(html! {<div/>})
            .with_child(label(tr!("Client")))
            .with_child(
                Field::new()
                    .name("client")
                    .aria_label(tr!("Client"))
                    .placeholder(tr!("All"))
                    .validate(|value: &String| match value.trim() {
                        "" => Ok(()),
                        value => value
                            .parse::<std::net::IpAddr>()
                            .map(drop)
                            .map_err(|_| anyhow::format_err!(tr!("Invalid IP address"))),
                    }),
            )
            .with_child(label(tr!("Status")))
            .with_child(
                Combobox::from_key_value_pairs([
                    ("success", tr!("Successful")),
                    ("error", tr!("Errors")),
                    ("client-error", tr!("Client Errors")),
                    ("server-error", tr!("Server Errors")),
                ])
                .name("status")
                .aria_label(tr!("Status"))
                .attribute(
                    "title",
                    tr!("Status filters include only requests with a known status."),
                )
                .placeholder(tr!("All")),
            )
            .with_child(html! {<div/>});

        // one form, for the search field in the toolbar to count as filter
        Some(
            Form::new()
                .form_context(self.filter_form_context.clone())
                .class("pwt-w-100")
                .with_child(
                    Column::new()
                        .class("pwt-w-100")
                        .with_child(toolbar)
                        .with_optional_child((!self.live).then(|| {
                            let mut toolbar = Toolbar::new()
                                .key("timespan-toolbar")
                                .class("pwt-w-100")
                                .class("pwt-border-bottom");
                            for child in self.timespan_fields(ctx) {
                                toolbar.add_child(child);
                            }
                            toolbar
                        }))
                        .with_child(filter),
                )
                .into(),
        )
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let empty = self.store.data_len() == 0;
        let nothing_found = empty && !self.loading() && self.next_cursor.is_none();
        let mask_text = match (self.load_older, self.searched_back_to) {
            (true, Some(time)) => tr!("Searching back from {0}...", render_epoch(time)),
            _ => tr!("Loading..."),
        };
        let activate = Callback::from({
            let store = self.store.clone();
            let link = ctx.link().clone();
            move |key: Key| {
                if let Some(row) = store.read().lookup_record(&key) {
                    link.change_view(Some(ViewState::Request(row.entry.clone())));
                }
            }
        });
        let table = DataTable::new(self.columns.clone(), self.store.clone())
            .class("pwt-flex-fit")
            .selection(self.selection.clone())
            .on_row_keydown({
                let activate = activate.clone();
                move |event: &mut DataTableKeyboardEvent| {
                    if event.key() == "Enter" {
                        event.prevent_default();
                        activate.emit(event.record_key.clone());
                    }
                }
            })
            .on_row_dblclick(move |event: &mut DataTableMouseEvent| {
                activate.emit(event.record_key.clone());
            })
            .row_render_callback(self.row_render_callback.clone())
            .on_table_scroll(ctx.link().callback(|event: Event| {
                let element: web_sys::Element = event.target_unchecked_into();
                Msg::Scrolled(element.scroll_top() <= 5)
            }));

        Column::new()
            .class("pwt-flex-fit")
            .with_child(
                Mask::new(table)
                    // keeps the text below the header
                    .class((!nothing_found).then_some("pwt-flex-fit"))
                    .visible(empty && self.loading())
                    .text(mask_text),
            )
            .with_optional_child(nothing_found.then(|| {
                Container::new()
                    .padding(4)
                    .class("pwt-text-align-center")
                    .with_child(tr!("No requests found"))
            }))
            .with_optional_child(
                self.load_error
                    .as_ref()
                    .map(|err| pwt::widget::error_message(err).class("pwt-border-top")),
            )
            .into_html_with_ref(self.view_ref.clone())
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        let ViewState::Request(entry) = view_state;
        let rows = [
            (tr!("Time"), Some(render_epoch(entry.time))),
            (tr!("User"), entry.auth_id.clone()),
            (tr!("Client"), entry.client.map(|ip| ip.to_string())),
            (tr!("Method"), entry.method.clone()),
            (tr!("Path"), entry.uri.clone()),
            (tr!("HTTP Version"), entry.http_version.clone()),
            (tr!("Request Line"), entry.request_line.clone()),
            (
                tr!("Status"),
                Some(
                    entry
                        .status
                        .map_or_else(|| tr!("Unknown"), |status| status.to_string()),
                ),
            ),
            (
                tr!("Size"),
                entry
                    .size
                    .map(|size| proxmox_human_byte::HumanByte::from(size).to_string()),
            ),
            (tr!("User Agent"), entry.user_agent.clone()),
        ];
        let mut content = Container::new()
            .class("pwt-d-grid pwt-gap-2 pwt-overflow-auto pwt-user-select-text")
            .padding(4)
            .style("grid-template-columns", "130px minmax(0, 1fr)")
            .style("overflow-wrap", "anywhere");
        for (label, value) in rows {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                content.add_child(html! {<div>{label}</div>});
                content.add_child(html! {<div>{value}</div>});
            }
        }
        Some(
            Dialog::new(tr!("Request"))
                .width(700)
                .style("max-height", "80vh")
                .on_close(ctx.link().change_view_callback(|_| None))
                .with_child(content)
                .into(),
        )
    }
}

impl From<AccessLog> for VNode {
    fn from(val: AccessLog) -> Self {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxAccessLog>>(Rc::new(val), None);
        VNode::from(comp)
    }
}

/// A validator for the maximum length the API accepts, as a longer value would only make the call
/// fail.
fn max_length(max: usize) -> impl Fn(&String) -> Result<(), Error> {
    move |value: &String| match value.trim().len() > max {
        true => Err(anyhow::format_err!(tr!("Maximum length is {0}", max))),
        false => Ok(()),
    }
}

/// Whether the server refused a request because too many requests to the log run already.
fn busy(err: &Error) -> bool {
    matches!(
        err.downcast_ref::<proxmox_client::Error>(),
        Some(proxmox_client::Error::Api(status, _)) if *status == http::StatusCode::SERVICE_UNAVAILABLE
    )
}
