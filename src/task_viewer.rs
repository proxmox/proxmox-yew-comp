use std::rc::Rc;

use pwt_macros::builder;
use serde_json::Value;

use gloo_timers::callback::Timeout;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::Loader;
use pwt::widget::{Button, Column, Dialog, TabBarItem, TabPanel, Toolbar};

use crate::percent_encoding::percent_encode_component;
use crate::{KVGrid, KVGridRow, LogView};
use crate::utils::render_epoch;

#[builder]
#[derive(Properties, PartialEq, Clone)]
pub struct TaskViewer {
    #[prop_or_default]
    node_ref: NodeRef,
    pub key: Option<Key>,

    pub task_id: String,
    pub endtime: Option<f64>,

    /// Close/Abort callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    pub on_close: Option<Callback<()>>,

    #[prop_or("/nodes/localhost/tasks".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,
}

impl TaskViewer {
    pub fn new(task_id: impl Into<String>) -> Self {
        yew::props!(Self {
            task_id: task_id.into(),
        })
    }

    pub fn endtime(mut self, endtime: f64) -> Self {
        self.endtime = Some(endtime);
        self
    }

    /// Builder style method to set the yew `node_ref`
    pub fn node_ref(mut self, node_ref: ::yew::html::NodeRef) -> Self {
        self.node_ref = node_ref;
        self
    }

    /// Builder style method to set the yew `key` property
    pub fn key(mut self, key: impl IntoOptionalKey) -> Self {
        self.key = key.into_optional_key();
        self
    }
}

pub enum Msg {
    DataChange,
    Reload,
    StopTask,
}

pub struct PwtTaskViewer {
    loader: Loader<Value>,
    reload_timeout: Option<Timeout>,
    active: bool,
    endtime: Option<f64>,
}

impl Component for PwtTaskViewer {
    type Message = Msg;
    type Properties = TaskViewer;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let url = format!(
            "{}/{}/status",
            props.base_url,
            percent_encode_component(&props.task_id),
        );
        let endtime = props.endtime;

        let loader = Loader::new(ctx.link().callback(|_| Msg::DataChange)).loader(move || {
            let url = url.clone();
            async move {
                let mut data: Value = crate::http_get(&url, None).await?;
                if let Some(endtime) = endtime {
                    data["endtime"] = endtime.into();
                }
                Ok(data)
            }
        });

        loader.load();
        Self {
            loader,
            reload_timeout: None,
            active: props.endtime.is_none(),
            endtime: props.endtime,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::DataChange => {
                let link = ctx.link().clone();
                self.active = self.task_is_active();
                if self.active {
                    self.reload_timeout = Some(Timeout::new(1_000, move || {
                        link.send_message(Msg::Reload);
                    }));
                } else {
                    if self.endtime.is_none() {
                        self.endtime = Some(proxmox_time::epoch_f64());
                    }
                }
                true
            }
            Msg::StopTask => {
                let url = format!(
                    "{}/{}",
                    props.base_url,
                    percent_encode_component(&props.task_id),
                );
                ctx.link().send_future(async move {
                    let _ = crate::http_delete(&url, None).await; // ignore errors?
                    Msg::Reload
                });

                true
            }
            Msg::Reload => {
                self.active = self.task_is_active();
                if self.active {
                    self.loader.load();
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let panel = self.loader.render(|data| {
            TabPanel::new()
                .class("pwt-flex-fit")
                .with_item(TabBarItem::new().label("Output"), self.view_output(ctx))
                .with_item(
                    TabBarItem::new().label("Status"),
                    self.view_status(ctx, data.clone()),
                )
        });

        Dialog::new("Task Viewer")
            .resizable(true)
            .style("width: 840px; height:600px;")
            .node_ref(props.node_ref.clone())
            .on_close(props.on_close.clone())
            .with_child(panel)
            .into()
    }
}

impl Into<VNode> for TaskViewer {
    fn into(self) -> VNode {
        let key = self.key.clone();
        let comp = VComp::new::<PwtTaskViewer>(Rc::new(self), key);
        VNode::from(comp)
    }
}

impl PwtTaskViewer {
    fn task_is_active(&self) -> bool {
        self.loader.with_state(|state| {
            if let Some(Ok(data)) = state.data.as_ref() {
                if let Some("stopped") = data["status"].as_str() {
                    return false;
                }
            }
            true
        })
    }

    fn view_status(&self, ctx: &Context<Self>, data: Rc<Value>) -> Html {
        let active = self.active;
        let endtime = self.endtime;
        let link = ctx.link();

        let toolbar = Toolbar::new().with_child(
            Button::new("Stop")
                .disabled(!active)
                .onclick(link.callback(|_| Msg::StopTask)),
        );

        let grid = KVGrid::new().data(data).rows(Rc::new(vec![
            KVGridRow::new("status", "Status")
                .renderer(|_name, value, record| {
                    let value = match value.as_str() {
                        Some(s) => s,
                        None => return html! {"unknown"},
                    };
                    if value != "stopped" {
                        return html! {{value}};
                    }
                    let status = record["exitstatus"].as_str().unwrap_or("unknown");
                    html! {{format!("{}: {}", value, status)}}
                })
                .placeholder("unknown"),
            KVGridRow::new("type", "Task type").required(true),
            KVGridRow::new("user", "User name")
                .renderer(|_name, value, record| {
                    let mut user = match value.as_str() {
                        Some(s) => s.to_owned(),
                        None => return html! {"unknown"},
                    };
                    if let Some(tokenid) = record["tokenid"].as_str() {
                        user.push_str(&format!("!{} (API Token)", tokenid));
                    }
                    html! {{user}}
                })
                .required(true),
            KVGridRow::new("node", "Node").required(true),
            KVGridRow::new("pid", "Process ID").required(true),
            KVGridRow::new("task_id", "Task ID"),
            KVGridRow::new("starttime", "Start Time")
                .renderer(|_name, value, _record| match value.as_i64() {
                    None => html! {"unknown (wrong format)"},
                    Some(epoch) => html! { {render_epoch(epoch)} },
                })
                .required(true),
            KVGridRow::new("endtime", "End Time").renderer(|_name, value, _record| {
                match value.as_i64() {
                    None => html! {"unknown (wrong format)"},
                    Some(epoch) => html! { {render_epoch(epoch)} },
                }
            }),
            KVGridRow::new("duration", "Duration")
                .renderer(move |_name, _value, record| {
                    if let Some(starttime) = record["starttime"].as_f64() {
                        if let Some(endtime) = record["endtime"].as_f64() {
                            if endtime >= starttime {
                                return html! {{format!("{:.0}s", endtime - starttime)}};
                            }
                        } else {
                            let now = endtime.unwrap_or_else(|| proxmox_time::epoch_f64());
                            return html! {{format!("{:.0}s", now - starttime)}};
                        }
                    }
                    html! {"-"}
                })
                .required(true),
            KVGridRow::new("upid", "Unique task ID"),
        ]));

        Column::new()
            .class("pwt-fit")
            .with_child(toolbar)
            .with_child(grid)
            .into()
    }

    fn view_output(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let task_id = props.task_id.clone();
        let active = self.active;
        let link = ctx.link();

        let toolbar = Toolbar::new().class("pwt-border-bottom").with_child(
            Button::new("Stop")
                .disabled(!active)
                .onclick(link.callback(|_| Msg::StopTask)),
        );

        let url = format!(
            "{}/{}/log",
            props.base_url,
            percent_encode_component(&task_id),
        );

        Column::new()
            .class("pwt-flex-fit")
            .with_child(toolbar)
            .with_child(LogView::new(url).class("pwt-p-2 pwt-flex-fill").active(active))
            .into()
    }
}
