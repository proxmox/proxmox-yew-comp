use std::rc::Rc;

use pwt::widget::Mask;
use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::{
    prelude::*,
    widget::{Button, Column},
};
use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct WebAuthn {
    /// Is the panel visible or not?
    ///
    /// We abort the webauthn challenge if the panel is hidden.
    #[prop_or_default]
    #[builder]
    pub visible: bool,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    pub on_webauthn: Option<Callback<String>>,
}

impl WebAuthn {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub enum Msg {
    Start,
}

pub struct ProxmoxWebAuthn {
    running: bool, // fixme: replace with webnauthn promise
}

impl Component for ProxmoxWebAuthn {
    type Message = Msg;
    type Properties = WebAuthn;

    fn create(_ctx: &Context<Self>) -> Self {
        Self { running: false }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Start => {
                self.running = true;
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        let props = ctx.props();

        if props.visible == false {
            log::info!("Abort running Webauthn challenge.");
            self.running = false;
        }

        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let panel = Column::new()
            .padding(2)
            .gap(2)
            .with_child(html! {<div>{"WARNING: THIS IS NOT IMPLEMENTED"}</div>})
            .with_flex_spacer()
            .with_child(
                Button::new("Start WebAuthn challenge")
                    .class("pwt-align-self-flex-end")
                    .class("pwt-scheme-primary")
                    .disabled(self.running)
                    .onclick(ctx.link().callback(|_| Msg::Start))
            );

        Mask::new()
            .class("pwt-flex-fill")
            .text("Please insert your authentication device and press its button")
            .visible(self.running)
            .with_child(panel)
            .into()
    }
}

impl Into<VNode> for WebAuthn {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxWebAuthn>(Rc::new(self), None);
        VNode::from(comp)
    }
}
