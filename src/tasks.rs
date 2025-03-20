use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;

use pwt::css::JustifyContent;
use pwt::widget::form::{Field, Form, FormContext, InputType};

use gloo_timers::callback::Timeout;
use html::IntoEventCallback;
use serde_json::Map;
use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::state::{PersistentState, Selection, Store};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader, DataTableRowRenderCallback,
};
use pwt::widget::{Button, Column, Fa, Row, Toolbar};

use crate::utils::{format_upid, render_epoch_short};

use crate::common_api_types::TaskListItem;

use pwt_macros::builder;

use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster, TaskViewer};

use super::{TaskStatusSelector, TaskTypeSelector};

const FILTER_UPDATE_BUFFER_MS: u32 = 150;
const BATCH_LIMIT: u64 = 500;
const LOAD_BUFFER_ROWS: usize = 20;

#[derive(PartialEq, Properties)]
#[builder]
pub struct Tasks {
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub nodename: Option<AttrValue>,

    /// Additional Input label/widget displayed on the filter panel.
    ///
    /// The widget need to read/write data from/to the provided form context.
    #[prop_or_default]
    pub extra_filter: Option<(AttrValue, Html)>,

    /// The base url, default is `/nodes/<nodename>/tasks`.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub base_url: Option<AttrValue>,

    #[builder_cb(IntoEventCallback, into_event_callback, (String, Option<i64>))]
    #[prop_or_default]
    /// Called when the task is opened
    pub on_show_task: Option<Callback<(String, Option<i64>)>>,

    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    /// An optional column configuration that overwrites the default one.
    pub columns: Option<Rc<Vec<DataTableHeader<TaskListItem>>>>,
}

impl Default for Tasks {
    fn default() -> Self {
        Self::new()
    }
}

impl Tasks {
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    /// Builder style method to set the extra filter input (label + widget)
    pub fn extra_filter(mut self, label: impl Into<AttrValue>, input: impl Into<Html>) -> Self {
        self.extra_filter = Some((label.into(), input.into()));
        self
    }

    fn get_nodename(&self) -> String {
        self.nodename.as_deref().unwrap_or("localhost").to_string()
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum ViewDialog {
    TaskViewer,
}

pub enum Msg {
    Redraw,
    ToggleFilter,
    LoadBatch(u64), // start
    UpdateFilter,
    ShowTask,
}
pub struct ProxmoxTasks {
    selection: Selection,
    store: Store<TaskListItem>,
    show_filter: PersistentState<bool>,
    filter_form_context: FormContext,
    row_render_callback: DataTableRowRenderCallback<TaskListItem>,
    start: u64,
    last_filter: serde_json::Value,
    load_timeout: Option<Timeout>,
    columns: Rc<Vec<DataTableHeader<TaskListItem>>>,
}

impl ProxmoxTasks {
    fn columns(ctx: &LoadableComponentContext<Self>) -> Rc<Vec<DataTableHeader<TaskListItem>>> {
        if let Some(columns) = ctx.props().columns.clone() {
            columns
        } else {
            Rc::new(vec![
                DataTableColumn::new(tr!("Start Time"))
                    .width("130px")
                    .render(|item: &TaskListItem| render_epoch_short(item.starttime).into())
                    .into(),
                DataTableColumn::new(tr!("End Time"))
                    .width("130px")
                    .render(|item: &TaskListItem| match item.endtime {
                        Some(endtime) => render_epoch_short(endtime).into(),
                        None => Row::new()
                            .class(JustifyContent::Center)
                            .with_child(Fa::new("").class("pwt-loading-icon"))
                            .into(),
                    })
                    .into(),
                DataTableColumn::new(tr!("User name"))
                    .width("150px")
                    .render(|item: &TaskListItem| {
                        html! {&item.user}
                    })
                    .into(),
                DataTableColumn::new(tr!("Description"))
                    .flex(1)
                    .render(move |item: &TaskListItem| html! {format_upid(&item.upid)})
                    .into(),
                DataTableColumn::new(tr!("Status"))
                    .width("200px")
                    .render(|item: &TaskListItem| match item.status.as_deref() {
                        Some("RUNNING") | None => Row::new()
                            .class(JustifyContent::Center)
                            .with_child(Fa::new("").class("pwt-loading-icon"))
                            .into(),
                        Some(text) => html! {text},
                    })
                    .into(),
            ])
        }
    }
}

impl LoadableComponent for ProxmoxTasks {
    type Properties = Tasks;
    type Message = Msg;
    type ViewState = ViewDialog;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let link = ctx.link();
        let selection = Selection::new().on_select(link.callback(|_| Msg::Redraw));
        let store = Store::new();

        let filter_form_context =
            FormContext::new().on_change(ctx.link().callback(|_| Msg::UpdateFilter));

        let row_render_callback = DataTableRowRenderCallback::new({
            let store = store.clone();
            let link = link.clone();
            move |args: &mut _| {
                if args.row_index() > store.data_len().saturating_sub(LOAD_BUFFER_ROWS) {
                    link.send_message(Msg::LoadBatch(store.data_len() as u64));
                }
                let record: &TaskListItem = args.record();
                match record.status.as_deref() {
                    Some("RUNNING" | "OK") | None => {}
                    Some(status) if status.starts_with("WARNINGS:") => {
                        args.add_class("pwt-color-warning")
                    }
                    _ => args.add_class("pwt-color-error"),
                }
            }
        });

        Self {
            selection,
            store,
            show_filter: PersistentState::new("ProxmoxTasksShowFilter"),
            filter_form_context,
            row_render_callback,
            last_filter: serde_json::Value::Object(Map::new()),
            start: 0,
            load_timeout: None,
            columns: Self::columns(ctx),
        }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let props = ctx.props();
        let nodename = props.get_nodename();
        let path = match &props.base_url {
            Some(url) => url.to_string(),
            None => format!("/nodes/{nodename}/tasks"),
        };

        let store = self.store.clone();

        let form_context = self.filter_form_context.read();
        let mut filter = form_context.get_submit_data();

        // Transform Date values
        if let Some(since) = filter.get("since").and_then(|v| v.as_str()) {
            let since = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(since));
            since.set_hours(0);
            since.set_minutes(0);
            since.set_seconds(0);
            let since = (since.get_time() / 1000.0) as u64;
            filter["since"] = since.into();
        }

        if let Some(until) = filter.get("until").and_then(|v| v.as_str()) {
            let until = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(until));
            until.set_hours(23);
            until.set_minutes(59);
            until.set_seconds(59);
            let until = (until.get_time() / 1000.0) as u64;
            filter["until"] = until.into();
        }

        let start = self.start;
        filter["start"] = start.into();
        filter["limit"] = BATCH_LIMIT.into();

        Box::pin(async move {
            let mut data: Vec<_> = crate::http_get(&path, Some(filter)).await?;
            if start == 0 {
                store.write().set_data(data);
            } else {
                store.write().append(&mut data);
            }
            Ok(())
        })
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Redraw => true,
            Msg::ToggleFilter => {
                self.show_filter.update(!*self.show_filter);
                true
            }
            Msg::UpdateFilter => {
                let form_context = self.filter_form_context.read();
                if !form_context.is_valid() {
                    return false;
                }
                let filter_params = form_context.get_submit_data();
                if ctx.loading() && self.last_filter == filter_params {
                    return false;
                }

                self.last_filter = filter_params;
                self.start = 0;

                let link = ctx.link().clone();
                self.load_timeout = Some(Timeout::new(FILTER_UPDATE_BUFFER_MS, move || {
                    link.send_reload();
                }));
                true
            }
            Msg::LoadBatch(start) => {
                self.start = start;
                let link = ctx.link().clone();
                self.load_timeout = Some(Timeout::new(FILTER_UPDATE_BUFFER_MS, move || {
                    link.send_reload();
                }));
                false
            }
            Msg::ShowTask => {
                if let Some(on_show_task) = &ctx.props().on_show_task {
                    let selected_item = self
                        .selection
                        .selected_key()
                        .and_then(|key| self.store.read().lookup_record(&key).cloned());
                    let selected_item = match selected_item {
                        Some(item) => item,
                        None => return false,
                    };
                    on_show_task.emit((selected_item.upid, selected_item.endtime));
                } else {
                    ctx.link().change_view(Some(ViewDialog::TaskViewer));
                }
                false
            }
        }
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let props = ctx.props();
        //let nodename = ctx.props().get_nodename();
        let selected_service = self.selection.selected_key().map(|k| k.to_string());
        let disabled = selected_service.is_none();

        let filter_icon_class = if *self.show_filter {
            "fa fa-filter pwt-color-primary"
        } else {
            "fa fa-filter"
        };

        let dirty_count = self.filter_form_context.read().dirty_count();

        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                Button::new(tr!("View"))
                    .disabled(disabled)
                    .onclick(ctx.link().callback(|_| Msg::ShowTask)),
            )
            .with_flex_spacer()
            .with_child({
                let form_context = self.filter_form_context.clone();
                Button::new(tr!("Clear Filter ({})", dirty_count))
                    .disabled(dirty_count == 0)
                    .onclick(move |_| form_context.write().reset_form())
            })
            .with_child(
                Button::new("Filter")
                    .icon_class(filter_icon_class)
                    .onclick(ctx.link().callback(|_| Msg::ToggleFilter)),
            )
            .with_child({
                let loading = ctx.loading();
                let link = ctx.link();
                Button::refresh(loading).onclick(move |_| link.send_message(Msg::LoadBatch(0)))
            });

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

        let mut filter = Form::new()
            .form_context(self.filter_form_context.clone())
            .class(filter_classes)
            .padding(4)
            .style("grid-template-columns","minmax(100px,auto) auto minmax(100px,auto) auto minmax(100px,auto) auto 1fr" )
            .with_child(html!{<div>{tr!("Since")}</div>})
            .with_child(
                Field::new()
                    .name("since")
                    .input_type(InputType::Date)
                )
            .with_child(html!{<div class="pwt-text-align-end">{tr!("Task Type")}</div>})
            .with_child(TaskTypeSelector::new().name("typefilter"))
            .with_child(html!{<div class="pwt-text-align-end">{tr!("Status")}</div>})
            .with_child(
                html!{<div style="grid-column-start:6; grid-column-end: -1;">{TaskStatusSelector::new().name("statusfilter")}</div>}
            )

            // second row
            .with_child(html!{<div>{tr!("Until:")}</div>})
            .with_child(
                Field::new()
                    .name("until")
                    .input_type(InputType::Date)
            )
            .with_child(html!{<div class="pwt-text-align-end">{tr!("User name")}</div>})
            .with_child(Field::new().name("userfilter"));

        if let Some((label, input)) = &props.extra_filter {
            filter.add_child(html! {<div class="pwt-text-align-end">{label}</div>});
            filter.add_child(input.clone());
        }

        let column = Column::new().with_child(toolbar).with_child(filter);

        Some(column.into())
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let columns = self.columns.clone();
        let link = ctx.link();

        DataTable::new(columns, self.store.clone())
            .class("pwt-flex-fit")
            .selection(self.selection.clone())
            .on_row_dblclick(move |_: &mut _| {
                link.send_message(Msg::ShowTask);
            })
            .row_render_callback(self.row_render_callback.clone())
            .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        let props = ctx.props();

        let selected_key = self.selection.selected_key()?;
        let selected_item = self.store.read().lookup_record(&selected_key)?.clone();

        match view_state {
            ViewDialog::TaskViewer => {
                let mut dialog = TaskViewer::new(&*selected_key)
                    .endtime(selected_item.endtime)
                    .on_close(ctx.link().change_view_callback(|_| None));
                if let Some(base_url) = &props.base_url {
                    dialog.set_base_url(base_url);
                }
                Some(dialog.into())
            }
        }
    }

    fn changed(
        &mut self,
        ctx: &LoadableComponentContext<Self>,
        old_props: &Self::Properties,
    ) -> bool {
        if old_props.columns != ctx.props().columns {
            self.columns = Self::columns(ctx);
        }
        true
    }
}

impl From<Tasks> for VNode {
    fn from(val: Tasks) -> Self {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxTasks>>(Rc::new(val), None);
        VNode::from(comp)
    }
}
