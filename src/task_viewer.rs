use std::rc::Rc;

use serde_json::Value;

use gloo_timers::callback::Timeout;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::state::Loader;
use pwt::widget::{Button, Column, Dialog, TabBarItem, TabPanel, Toolbar};
use pwt::{prelude::*, AsyncPool};

use crate::percent_encoding::percent_encode_component;
use crate::utils::{format_duration_human, format_upid, render_epoch};
use crate::{KVGrid, KVGridRow, LogView};

use pwt_macros::builder;

#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct TaskViewer {
    #[prop_or_default]
    pub key: Option<Key>,

    pub task_id: String,

    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub endtime: Option<i64>,

    /// Close/Abort callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
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

    pwt::impl_yew_std_props_builder!();
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
    endtime: Option<i64>,
    async_pool: AsyncPool,
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

        let loader = Loader::new()
            .loader(move || {
                let url = url.clone();
                async move {
                    let mut data: Value = crate::http_get(&url, None).await?;
                    if let Some(endtime) = endtime {
                        data["endtime"] = endtime.into();
                    }
                    Ok(data)
                }
            })
            .on_change(ctx.link().callback(|_| Msg::DataChange));

        loader.load();
        Self {
            loader,
            reload_timeout: None,
            active: props.endtime.is_none(),
            endtime: props.endtime,
            async_pool: AsyncPool::new(),
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
                } else if self.endtime.is_none() {
                    self.endtime = Some(proxmox_time::epoch_i64());
                }
                true
            }
            Msg::StopTask => {
                let url = format!(
                    "{}/{}",
                    props.base_url,
                    percent_encode_component(&props.task_id),
                );
                let link = ctx.link().clone();
                self.async_pool.spawn(async move {
                    let _ = crate::http_delete(&url, None).await; // ignore errors?
                    link.send_message(Msg::Reload);
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
                .with_item(
                    TabBarItem::new().label(tr!("Output")),
                    self.view_output(ctx),
                )
                .with_item(
                    TabBarItem::new().label(tr!("Status")),
                    self.view_status(ctx, data.clone()),
                )
        });

        let title = format_upid(&props.task_id);

        Dialog::new(tr!("Task Viewer") + ": " + &title)
            .resizable(true)
            .width(840)
            .height(600)
            .on_close(props.on_close.clone())
            .with_child(panel)
            .into()
    }
}

impl From<TaskViewer> for VNode {
    fn from(val: TaskViewer) -> Self {
        let key = val.key.clone();
        let comp = VComp::new::<PwtTaskViewer>(Rc::new(val), key);
        VNode::from(comp)
    }
}

impl PwtTaskViewer {
    fn task_is_active(&self) -> bool {
        if let Some(Ok(data)) = self.loader.read().data.as_ref() {
            if let Some("stopped") = data["status"].as_str() {
                return false;
            }
        }
        true
    }

    fn view_status(&self, ctx: &Context<Self>, data: Rc<Value>) -> Html {
        let active = self.active;
        let endtime = self.endtime;
        let link = ctx.link();

        let toolbar = Toolbar::new().with_child(
            Button::new(tr!("Stop"))
                .disabled(!active)
                .onclick(link.callback(|_| Msg::StopTask)),
        );

        let grid = KVGrid::new()
            .class("pwt-flex-fit")
            .data(data)
            .rows(Rc::new(vec![
                KVGridRow::new("status", tr!("Status"))
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
                KVGridRow::new("type", tr!("Task type")).required(true),
                KVGridRow::new("user", tr!("User name"))
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
                KVGridRow::new("node", tr!("Node")).required(true),
                KVGridRow::new("pid", tr!("Process ID")).required(true),
                KVGridRow::new("task_id", tr!("Task ID")),
                KVGridRow::new("starttime", tr!("Start Time"))
                    .renderer(|_name, value, _record| match value.as_i64() {
                        None => html! {"unknown (wrong format)"},
                        Some(epoch) => html! { {render_epoch(epoch)} },
                    })
                    .required(true),
                KVGridRow::new("endtime", tr!("End Time")).renderer(|_name, value, _record| {
                    match value.as_i64() {
                        None => html! {"unknown (wrong format)"},
                        Some(epoch) => html! { {render_epoch(epoch)} },
                    }
                }),
                KVGridRow::new("duration", tr!("Duration"))
                    .renderer(move |_name, _value, record| {
                        if let Some(starttime) = record["starttime"].as_i64() {
                            let duration = if let Some(endtime) = record["endtime"].as_i64() {
                                endtime - starttime
                            } else {
                                let now = endtime.unwrap_or_else(proxmox_time::epoch_i64);
                                now - starttime
                            };
                            return html! {format_duration_human(duration as f64)};
                        }
                        html! {"-"}
                    })
                    .required(true),
                KVGridRow::new("upid", tr!("Unique task ID")),
            ]));

        Column::new()
            .class("pwt-flex-fit")
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
            Button::new(tr!("Stop"))
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
            .with_child(
                LogView::new(url)
                    .padding(2)
                    .class("pwt-flex-fill")
                    .active(active),
            )
            .into()
    }
}
