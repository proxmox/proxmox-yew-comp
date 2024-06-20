use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;

use pwt::widget::form::{Field, Form, FormContext, InputType};

use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::state::{PersistentState, Selection, Store};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader, DataTableRowRenderCallback,
};
use pwt::widget::{Button, Column, Toolbar};

use crate::utils::{format_upid, render_epoch_short};

use crate::common_api_types::TaskListItem;

use pwt_macros::builder;

use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster, TaskViewer};

use super::{TaskStatusSelector, TaskTypeSelector};

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

    /// The base url, default is '/nodes/<nodename>/tasks'
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub base_url: Option<AttrValue>,
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
    UpdateFilter,
}
pub struct ProxmoxTasks {
    selection: Selection,
    store: Store<TaskListItem>,
    show_filter: PersistentState<bool>,
    filter_form_context: FormContext,
    row_render_callback: DataTableRowRenderCallback<TaskListItem>,
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

        let row_render_callback = DataTableRowRenderCallback::new(|args: &mut _| {
            let record: &TaskListItem = args.record();
            if let Some(status) = &record.status {
                if status != "OK" {
                    if status.starts_with("WARNINGS:") {
                        args.add_class("pwt-color-warning");
                    } else {
                        args.add_class("pwt-color-error");
                    }
                }
            }
        });

        Self {
            selection,
            store,
            show_filter: PersistentState::new("ProxmoxTasksShowFilter"),
            filter_form_context,
            row_render_callback,
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
        let filter = form_context.get_submit_data();
        Box::pin(async move {
            let data = crate::http_get(&path, Some(filter)).await?;
            store.write().set_data(data);
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
                // fixme: delay load
                let form_context = self.filter_form_context.read();
                if !form_context.is_valid() {
                    return false;
                }
                ctx.link().send_reload();
                true
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
                Button::new(tr!("View")).disabled(disabled).onclick(
                    ctx.link()
                        .change_view_callback(|_| Some(ViewDialog::TaskViewer)),
                ),
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
                Button::refresh(loading).onclick(move |_| link.send_reload())
            });

        let filter_classes = classes!(
            "pwt-overflow-auto",
            "pwt-border-bottom",
            "pwt-p-4",
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
            .attribute("style", "grid-template-columns: minmax(100px,auto) auto minmax(100px,auto) auto minmax(100px,auto) auto 1fr;")
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
        let columns = COLUMNS.with(Rc::clone);
        let link = ctx.link();

        DataTable::new(columns, self.store.clone())
            .class("pwt-flex-fit")
            .selection(self.selection.clone())
            .on_row_dblclick(move |_: &mut _| {
                link.change_view(Some(ViewDialog::TaskViewer));
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

        let selected_key = match self.selection.selected_key() {
            Some(key) => key, // upid
            None => return None,
        };
        let selected_item = match self.store.read().lookup_record(&selected_key) {
            Some(item) => item.clone(),
            None => return None,
        };

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
}

thread_local! {
    static COLUMNS: Rc<Vec<DataTableHeader<TaskListItem>>> = Rc::new(vec![
        DataTableColumn::new(tr!("Start Time"))
            .width("130px")
            .render(|item: &TaskListItem| {
                render_epoch_short(item.starttime).into()
            })
            .into(),
        DataTableColumn::new(tr!("End Time"))
            .width("130px")
            .render(|item: &TaskListItem| {
                match item.endtime {
                    Some(endtime) => render_epoch_short(endtime).into(),
                    None => html!{},
            }})
            .into(),
        DataTableColumn::new(tr!("User name"))
            .width("150px")
            .render(|item: &TaskListItem| {
                html!{&item.user}
            })
            .into(),
        DataTableColumn::new(tr!("Description"))
            .flex(1)
            .render(|item: &TaskListItem| {
                html!{format_upid(&item.upid)}
            })
            .into(),
        DataTableColumn::new(tr!("Status"))
            .width("200px")
            .render(|item: &TaskListItem| {
                let text = item.status.as_deref().unwrap_or("");
                html!{text}
            })
            .into(),
        ]);
}

impl Into<VNode> for Tasks {
    fn into(self) -> VNode {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxTasks>>(Rc::new(self), None);
        VNode::from(comp)
    }
}
