use std::rc::Rc;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoOptionalRenderFn, RenderFn};
use pwt::state::{Loader, LoaderState, SharedStateObserver, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::{ActionIcon, Button, Container, Panel, Toolbar, Tooltip};

use crate::utils::{format_duration_human, format_upid, render_epoch_short};
use pbs_api_types::TaskListItem;

use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct RunningTasks {
    pub loader: Loader<Vec<TaskListItem>>,

    #[builder_cb(IntoEventCallback, into_event_callback, (String, Option<i64>))]
    #[prop_or_default]
    pub on_show_task: Option<Callback<(String, Option<i64>)>>,

    #[builder_cb(IntoOptionalRenderFn, into_optional_render_fn, TaskListItem)]
    #[prop_or_default]
    /// Render function for the [`TaskListItem`]
    pub render: Option<RenderFn<TaskListItem>>,

    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    #[builder]
    #[prop_or_default]
    pub as_dropdown: bool,

    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    /// Custom Buttons instead of the default 'Show all' one.
    pub buttons: Option<Vec<Button>>,
}

impl RunningTasks {
    pub fn new(loader: Loader<Vec<TaskListItem>>) -> Self {
        yew::props!(Self { loader })
    }
}

pub enum Msg {
    DataChange,
}

#[doc(hidden)]
pub struct ProxmoxRunningTasks {
    store: Store<TaskListItem>,
    _listener: SharedStateObserver<LoaderState<Vec<TaskListItem>>>,
}

impl ProxmoxRunningTasks {
    fn update_data(&mut self, ctx: &Context<Self>) {
        let props = ctx.props();
        if let Some(Ok(data)) = &props.loader.read().data {
            let mut list = data.to_vec();
            list.sort_by(|a, b| a.starttime.cmp(&b.starttime));
            list.truncate(10);

            let now = proxmox_time::epoch_i64();
            for task in &mut list {
                if task.endtime.is_none() {
                    task.endtime = Some(now);
                }
            }
            self.store.set_data(list);
        }
    }

    fn running_tasks_columns(&self, ctx: &Context<Self>) -> Rc<Vec<DataTableHeader<TaskListItem>>> {
        let props = ctx.props();

        Rc::new(vec![
            DataTableColumn::new(tr!("Task"))
                .flex(1)
                .render({
                    let render = props.render.clone();
                    move |item: &TaskListItem| {
                        if let Some(render) = &render {
                            render.apply(item)
                        } else {
                            html! {format_upid(&item.upid)}
                        }
                    }
                })
                .into(),
            DataTableColumn::new(tr!("Start Time"))
                .width("130px")
                .render(|item: &TaskListItem| render_epoch_short(item.starttime).into())
                .into(),
            DataTableColumn::new(tr!("Duration"))
                .render(|item: &TaskListItem| {
                    let duration = match item.endtime {
                        Some(endtime) => endtime - item.starttime,
                        None => return html! {"-"},
                    };
                    html! {format_duration_human(duration as f64)}
                })
                .into(),
            DataTableColumn::new(tr!("Action"))
                .width("40px")
                .render({
                    let on_show_task = props.on_show_task.clone();
                    move |item: &TaskListItem| {
                        let upid = item.upid.clone();
                        let endtime = item.endtime;
                        let on_show_task = on_show_task.clone();
                        let icon = ActionIcon::new("fa fa-chevron-right").on_activate(move |_| {
                            if let Some(on_show_task) = &on_show_task {
                                on_show_task.emit((upid.clone(), endtime));
                            }
                        });
                        Tooltip::new(icon).tip(tr!("Open Task")).into()
                    }
                })
                .into(),
        ])
    }
}

impl Component for ProxmoxRunningTasks {
    type Message = Msg;
    type Properties = RunningTasks;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        let store = Store::with_extract_key(|item: &TaskListItem| Key::from(item.upid.clone()));

        let _listener = props
            .loader
            .add_listener(ctx.link().callback(|_| Msg::DataChange));

        let mut me = Self { store, _listener };

        me.update_data(ctx);
        me
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::DataChange => {
                self.update_data(ctx);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let content = props.loader.render(|_data| -> Html {
            if self.store.data_len() == 0 {
                Container::new()
                    .padding(2)
                    .with_child(tr!("No running tasks"))
                    .into()
            } else {
                let columns = self.running_tasks_columns(ctx);
                DataTable::new(columns, self.store.clone())
                    .show_header(false)
                    .into()
            }
        });

        let toolbar = props.as_dropdown.then(|| {
            if let Some(buttons) = props.buttons.clone() {
                let mut tb = Toolbar::new().with_flex_spacer();
                for button in buttons {
                    tb.add_child(button);
                }
                tb
            } else {
                Toolbar::new().with_flex_spacer().with_child({
                    let on_close = props.on_close.clone();
                    Button::new(tr!("Show All Tasks"))
                        .class("pwt-scheme-primary")
                        .onclick(move |_| {
                            crate::utils::set_location_href("#/administration/tasks");
                            if let Some(on_close) = &on_close {
                                on_close.emit(());
                            }
                        })
                })
            }
        });

        Panel::new()
            .min_width(600)
            .class("pwt-flex-fit")
            .border(true)
            .title(tr!("Running Tasks"))
            .with_child(content)
            .with_optional_child(toolbar)
            .into()
    }
}

impl From<RunningTasks> for VNode {
    fn from(val: RunningTasks) -> Self {
        let comp = VComp::new::<ProxmoxRunningTasks>(Rc::new(val), None);
        VNode::from(comp)
    }
}
