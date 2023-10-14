use std::rc::Rc;

use derivative::Derivative;

use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Field, Form, FormContext, SubmitButton};
use pwt::widget::{Dialog, TabBarItem, TabPanel, SelectionViewRenderInfo};

use pwt_macros::builder;

use proxmox_login::SecondFactorChallenge;

use super::WebAuthn;

#[derive(Derivative)]
#[derivative(Clone, PartialEq)]
#[derive(Properties)]
#[builder]
pub struct TfaDialog {
    /// The TFA challenge returned by the server.
    #[derivative(PartialEq(compare_with = "Rc::ptr_eq"))]
    challenge: Rc<SecondFactorChallenge>,

    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_totp: Option<Callback<String>>,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_yubico: Option<Callback<String>>,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_recovery: Option<Callback<String>>,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_webauthn: Option<Callback<String>>,
}

impl TfaDialog {
    /// Create a new instance with TFA challenge returned by the server.
    pub fn new(challenge: Rc<SecondFactorChallenge>) -> Self {
        yew::props!(Self { challenge })
    }
}

pub struct ProxmoxTfaDialog {}

fn render_totp(callback: Option<Callback<String>>) -> Html {
    Form::new()
        .padding(2)
        .class("pwt-flex-fill pwt-d-flex pwt-flex-direction-column pwt-gap-2")
        .with_child(html! {<div>{"Please enter your TOTP verification code"}</div>})
        .with_child(Field::new().name("data").required(true).autofocus(true))
        .with_child(html!{<div style="flex: 1 1 auto;"/>})
        .with_child(
            SubmitButton::new()
                .class("pwt-align-self-flex-end")
                .class("pwt-scheme-primary")
                .text("Confirm")
                .on_submit({
                    move |form_ctx: FormContext| {
                        let data = form_ctx.read().get_field_text("data");
                        if let Some(callback) = &callback {
                            callback.emit(data);
                        }
                    }
                }),
        )
        .into()
}

fn render_yubico(callback: Option<Callback<String>>) -> Html {
    Form::new()
        .padding(2)
        .class("pwt-flex-fill pwt-d-flex pwt-flex-direction-column pwt-gap-2")
        .with_child(html! {<div>{"Please enter your Yubico OTP code"}</div>})
        .with_child(Field::new().name("data").required(true).autofocus(true))
        .with_child(html!{<div style="flex: 1 1 auto;"/>})
        .with_child(
            SubmitButton::new()
                .class("pwt-align-self-flex-end")
                .class("pwt-scheme-primary")
                .text("Confirm")
                .on_submit({
                    move |form_ctx: FormContext| {
                        let data = form_ctx.read().get_field_text("data");
                        if let Some(callback) = &callback {
                            callback.emit(data);
                        }
                    }
                }),
        )
        .into()
}

fn render_recovery(callback: Option<Callback<String>>, available_keys: &[usize]) -> Html {
    if available_keys.is_empty() {
        let msg = "No more recovery keys available.";
        return pwt::widget::error_message(msg, "pwt-p-2");
    }

    Form::new()
        .padding(2)
        .class("pwt-flex-fill pwt-d-flex pwt-flex-direction-column pwt-gap-2")
        .with_child(html! {<div>{"Please enter one of your single-use recovery keys"}</div>})
        .with_child(html! {<div>{format!{"Available recovery keys: {:?}", available_keys}}</div>})
        .with_child(Field::new().name("data").required(true).autofocus(true))
        .with_optional_child((available_keys.len() <= 13).then(|| {
            html! {<div>{
                format!(
                    "Less than {0} recovery keys available. Please generate a new set after login!",
                    available_keys.len() + 1,
                )
            }</div>}
        }))
        .with_child(html!{<div style="flex: 1 1 auto;"/>})
        .with_child(
            SubmitButton::new()
                .class("pwt-align-self-flex-end")
                .class("pwt-scheme-primary")
                .text("Confirm")
                .on_submit({
                    move |form_ctx: FormContext| {
                        let data = form_ctx.read().get_field_text("data");
                        if let Some(callback) = &callback {
                            callback.emit(data);
                        }
                    }
                }),
        )
        .into()
}

impl Component for ProxmoxTfaDialog {
    type Message = ();
    type Properties = TfaDialog;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let mut panel = TabPanel::new().class("pwt-flex-fill");

        if props.challenge.challenge.totp {
            panel.add_item_builder(TabBarItem::new().key("totp").label("TOTP App"), {
                let on_totp = props.on_totp.clone();
                move |_| render_totp(on_totp.clone())
            });
        }

        if props.challenge.challenge.yubico {
            panel.add_item_builder(TabBarItem::new().key("yubico").label("Yubico OTP"), {
                let on_yubico = props.on_yubico.clone();
                move |_| render_yubico(on_yubico.clone())
            });
        }

        if props.challenge.challenge.recovery.is_available() {
            panel.add_item_builder(TabBarItem::new().key("recovery").label("Recovery Key"), {
                let on_recovery = props.on_recovery.clone();
                let available_keys = props.challenge.challenge.recovery.0.clone();
                move |_| render_recovery(on_recovery.clone(), &available_keys)
            });
        }

        // webauthn not implemented - delayed to debian bookworm for newer rust packages ..
        if true /* props.challenge.challenge.webauthn.is_some() */ {
            panel.add_item_builder(TabBarItem::new().key("webauthn").label("WebAuthN"), {
                let on_webauthn = props.on_webauthn.clone();
                move |info: &SelectionViewRenderInfo| {
                    WebAuthn::new()
                        .visible(info.visible)
                        .on_webauthn(on_webauthn.clone())
                        .into()
                }
            });
        }

        Dialog::new("Second login factor required")
            .style("min-width:600px;min-height:300px;")
            .resizable(true)
            .with_child(panel)
            .on_close(props.on_close.clone())
            .into()
    }
}

impl Into<VNode> for TfaDialog {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxTfaDialog>(Rc::new(self), None);
        VNode::from(comp)
    }
}
