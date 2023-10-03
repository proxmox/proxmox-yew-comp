use std::rc::Rc;

use pwt_macros::builder;
use serde_json::Value;

use gloo_timers::callback::Timeout;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::Loader;
use pwt::widget::{Button, Column, Container, Dialog, Progress, Row};

use crate::percent_encoding::percent_encode_component;
use crate::TaskViewer;

#[builder]
#[derive(Properties, PartialEq, Clone)]
pub struct TaskProgress {
    #[prop_or_default]
    node_ref: NodeRef,
    pub key: Option<Key>,

    pub task_id: String,

    pub on_close: Option<Callback<()>>,

    #[prop_or("/nodes/localhost/tasks".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,
}

impl TaskProgress {
    pub fn new(task_id: impl Into<String>) -> Self {
        yew::props!(Self {
            task_id: task_id.into(),
        })
    }

    /// Builder style method to set the yew `node_ref`
    pub fn node_ref(mut self, node_ref: ::yew::html::NodeRef) -> Self {
        self.node_ref = node_ref;
        self
    }

    /// Builder style method to set the yew `key` property
    pub fn key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn on_close(mut self, cb: impl IntoEventCallback<()>) -> Self {
        self.on_close = cb.into_event_callback();
        self
    }
}

pub enum Msg {
    DataChange,
    Reload,
    ShowDetails,
}

pub struct PwtTaskProgress {
    loader: Loader<Value>,
    reload_timeout: Option<Timeout>,
    active: bool,
    endtime: Option<f64>,
    show_details: bool,
}

impl Component for PwtTaskProgress {
    type Message = Msg;
    type Properties = TaskProgress;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let url = format!(
            "{}/{}/status",
            props.base_url,
            percent_encode_component(&props.task_id),
        );

        let loader = Loader::new()
            .loader(move || {
                let url = url.clone();
                async move { crate::http_get(&url, None).await }
            })
            .on_change(ctx.link().callback(|_| Msg::DataChange))
            ;

        loader.load();
        Self {
            loader,
            reload_timeout: None,
            active: true,
            endtime: None,
            show_details: false,
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
                    if !self.show_details {
                        if let Some(on_close) = &props.on_close {
                            on_close.emit(());
                        }
                    }
                }
                true
            }
            Msg::ShowDetails => {
                self.show_details = true;
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

        if self.show_details {
            return TaskViewer::new(props.task_id.clone())
                .base_url(props.base_url.clone())
                .on_close(props.on_close.clone())
                .into();
        }

        let active = self.active;
        let panel = self.loader.render(|data| {
            Column::new()
                .class("pwt-flex-fill pwt-overflow-auto")
                .with_child({
                    if active {
                        Progress::new().into()
                    } else {
                        Progress::new().value(1.0)
                    }
                })
                .with_child(Container::new().padding(2).with_child(if active {
                    html! {"running"}
                } else {
                    let exit_status = data["exitstatus"].as_str().unwrap_or("unknown");
                    if exit_status == "OK" {
                        html! {"Done! Task finished successfully."}
                    } else {
                        html! {format!("Task failed: {exit_status}")}
                    }
                }))
        });

        Dialog::new("Task Progress")
            .resizable(true)
            .style("min-width: 300px;")
            .node_ref(props.node_ref.clone())
            .on_close(props.on_close.clone())
            .with_child(panel)
            .with_child(Row::new().padding(2).with_flex_spacer().with_child(
                Button::new("Details").onclick(ctx.link().callback(|_| Msg::ShowDetails)),
            ))
            .into()
    }
}

impl Into<VNode> for TaskProgress {
    fn into(self) -> VNode {
        let key = self.key.clone();
        let comp = VComp::new::<PwtTaskProgress>(Rc::new(self), key);
        VNode::from(comp)
    }
}

impl PwtTaskProgress {
    fn task_is_active(&self) -> bool {
        if let Some(Ok(data)) = self.loader.read().data.as_ref() {
            if let Some("stopped") = data["status"].as_str() {
                return false;
            }
        }
        true
    }
}
