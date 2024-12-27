use std::rc::Rc;

use gloo_timers::callback::Timeout;
use pwt::css::ColorScheme;
use wasm_bindgen::JsCast;
use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::dom::align::{align_to, AlignOptions, GrowDirection, Point};
use pwt::prelude::*;
use pwt::state::{Loader, LoaderState, SharedStateObserver};
use pwt::widget::{Button, Container};

use crate::common_api_types::TaskListItem;
use crate::RunningTasks;

use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct RunningTasksButton {
    running_tasks: Loader<Vec<TaskListItem>>,

    #[builder_cb(IntoEventCallback, into_event_callback, (String, Option<i64>))]
    #[prop_or_default]
    on_show_task: Option<Callback<(String, Option<i64>)>>,
}

impl RunningTasksButton {
    pub fn new(running_tasks: Loader<Vec<TaskListItem>>) -> Self {
        yew::props!(Self { running_tasks })
    }
}

pub enum Msg {
    Redraw,
    ShowMenu,
    CloseMenu,
    FocusChange(bool),
    DelayedFocusChange(bool),
}

pub struct ProxmoxRunningTasksButton {
    node_ref: NodeRef,
    submenu_ref: NodeRef,
    _listener: SharedStateObserver<LoaderState<Vec<TaskListItem>>>,
    show_submenu: bool,
    align_options: AlignOptions,

    timeout: Option<Timeout>,
    last_has_focus: bool,
}

impl ProxmoxRunningTasksButton {
    fn restore_focus(&mut self) {
        if let Some(node) = self.node_ref.get() {
            if let Some(el) = node.dyn_into::<web_sys::HtmlElement>().ok() {
                let _ = el.focus();
            }
        }
    }

    fn align_popup(&self) {
        if self.show_submenu {
            if let Err(err) = align_to(
                &self.node_ref,
                &self.submenu_ref,
                Some(self.align_options.clone()),
            ) {
                log::error!("could not position menu: {}", err.to_string());
            }
        }
    }
}

impl Component for ProxmoxRunningTasksButton {
    type Message = Msg;
    type Properties = RunningTasksButton;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let _listener = props
            .running_tasks
            .add_listener(ctx.link().callback(|_| Msg::Redraw));

        let align_options =
            AlignOptions::new(Point::BottomStart, Point::TopStart, GrowDirection::StartEnd)
                .align_width(true)
                .offset(0.0, 1.0)
                .viewport_padding(5.0);

        Self {
            _listener,
            node_ref: NodeRef::default(),
            submenu_ref: NodeRef::default(),
            show_submenu: false,
            align_options,
            timeout: None,
            last_has_focus: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Redraw => true,
            Msg::ShowMenu => {
                self.show_submenu = true;
                true
            }
            Msg::CloseMenu => {
                self.show_submenu = false;
                self.restore_focus();
                true
            }
            Msg::FocusChange(has_focus) => {
                let link = ctx.link().clone();
                self.timeout = Some(Timeout::new(1, move || {
                    link.send_message(Msg::DelayedFocusChange(has_focus));
                }));
                false
            }
            Msg::DelayedFocusChange(has_focus) => {
                if has_focus == self.last_has_focus {
                    return false;
                }
                self.last_has_focus = has_focus;

                if !has_focus {
                    self.show_submenu = false;
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let count = match &props.running_tasks.read().data {
            Some(Ok(data)) => data.len(),
            _ => 0,
        };

        let show_submenu = self.show_submenu;

        let submenu = show_submenu.then(|| {
            Container::new()
                .attribute("role", "none")
                .class("pwt-submenu")
                .node_ref(self.submenu_ref.clone())
                .with_child(
                    RunningTasks::new(props.running_tasks.clone())
                        .as_dropdown(true)
                        .on_show_task(props.on_show_task.clone())
                        .on_close(ctx.link().callback(|_| Msg::CloseMenu)),
                )
        });

        let button = Button::new(tr!("Tasks") + &format!(": {}", count))
            .node_ref(self.node_ref.clone())
            .attribute("aria-haspopup", "true")
            .attribute("aria-expanded", show_submenu.then_some("true"))
            .show_arrow(true)
            .class(ColorScheme::Primary)
            .icon_class("fa fa-list-alt")
            .onkeydown({
                let link = ctx.link().clone();
                move |event: KeyboardEvent| {
                    match event.key().as_str() {
                        "Escape" => link.send_message(Msg::CloseMenu),
                        "ArrowDown" => link.send_message(Msg::ShowMenu),
                        _ => return,
                    }
                    event.stop_propagation();
                    event.prevent_default();
                }
            })
            .onclick(ctx.link().callback(move |event: MouseEvent| {
                event.stop_propagation();
                if show_submenu {
                    Msg::CloseMenu
                } else {
                    Msg::ShowMenu
                }
            }));

        Container::new()
            .style("display", "contents")
            .attribute("role", "none")
            .onfocusin(ctx.link().callback(|_| Msg::FocusChange(true)))
            .onfocusout(ctx.link().callback(|_| Msg::FocusChange(false)))
            .with_child(button)
            .with_optional_child(submenu)
            .into()
    }
    fn rendered(&mut self, _ctx: &Context<Self>, _first_render: bool) {
        self.align_popup();
    }
}

impl From<RunningTasksButton> for VNode {
    fn from(val: RunningTasksButton) -> Self {
        let comp = VComp::new::<ProxmoxRunningTasksButton>(Rc::new(val), None);
        VNode::from(comp)
    }
}
