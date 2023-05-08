use std::rc::Rc;

use derivative::Derivative;

use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Field, Form, FormContext, SubmitButton};
use pwt::widget::{Dialog, TabBarItem, TabPanel};

use pwt_macros::builder;

use proxmox_login::SecondFactorChallenge;
#[derive(Derivative)]
#[derivative(Clone, PartialEq)]
#[derive(Properties)]
#[builder]
pub struct TfaDialog {
    /// The TFA challenge returned by the server.
    #[derivative(PartialEq(compare_with = "Rc::ptr_eq"))]
    challenge: Rc<SecondFactorChallenge>,

    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    pub on_close: Option<Callback<()>>,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    pub on_totp: Option<Callback<String>>,

}

impl TfaDialog {
    /// Create a new instance with TFA challenge returned by the server.
    pub fn new(challenge: Rc<SecondFactorChallenge>) -> Self {
        yew::props!(Self { challenge })
    }
}

pub struct PbsTfaDialog {}

fn render_totp(on_totp: Option<Callback<String>>) -> Html {
    Form::new()
        .padding(2)
        .class("pwt-d-flex pwt-flex-direction-column pwt-gap-2")
        .with_child(html! {<div>{"Please enter your TOTP verification code"}</div>})
        .with_child(Field::new().name("data").required(true).autofocus(true))
        .with_child(
            SubmitButton::new()
                .class("pwt-scheme-primary")
                .text("Confirm")
                .on_submit({
                    move |form_ctx: FormContext| {
                        let data = form_ctx.read().get_field_text("data");
                        if let Some(on_totp) = &on_totp {
                            on_totp.emit(data);
                        }
                    }
                }),
        )
        .into()
}

impl Component for PbsTfaDialog {
    type Message = ();
    type Properties = TfaDialog;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let mut panel = TabPanel::new();

        if props.challenge.challenge.totp {
            panel.add_item_builder(TabBarItem::new().key("totp").label("TOPT"), {
                let on_totp = props.on_totp.clone();
                move |_| render_totp(on_totp.clone())
            });
        }

        Dialog::new("Second login factor required")
            .with_child(panel)
            .on_close(props.on_close.clone())
            .into()
    }
}

impl Into<VNode> for TfaDialog {
    fn into(self) -> VNode {
        let comp = VComp::new::<PbsTfaDialog>(Rc::new(self), None);
        VNode::from(comp)
    }
}
