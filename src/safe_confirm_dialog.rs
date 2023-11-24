use std::rc::Rc;

use anyhow::Error;

use pwt::widget::InputPanel;
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Field, Form, FormContext, SubmitButton};
use pwt::widget::{Dialog, Toolbar};

use pwt_macros::builder;

use super::default_confirm_remove_message;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct SafeConfirmDialog {
    /// Dialog title (defaults to "Confirm")
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub title: Option<AttrValue>,

    /// Submit button text.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub submit_text: Option<AttrValue>,

    /// Close window callback.
    ///
    /// Parameter is set to true if the user confirmed the action.
    #[prop_or_default]
    #[builder_cb(IntoEventCallback, into_event_callback, bool)]
    pub on_close: Option<Callback<bool>>,

    /// The message.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub message: Option<AttrValue>,

    /// The user needs to input this text to confirm the action.
    pub verify_id: AttrValue,
}
impl SafeConfirmDialog {
    pub fn new(verify_id: impl Into<AttrValue>) -> Self {
        yew::props!(Self {
            verify_id: verify_id.into(),
        })
    }
}

#[doc(hidden)]
pub struct ProxmoxSafeConfirmDialog {
    form_ctx: FormContext,
}

impl Component for ProxmoxSafeConfirmDialog {
    type Message = ();
    type Properties = SafeConfirmDialog;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            form_ctx: FormContext::new(),
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let verify_id = props.verify_id.clone();

        let message = props
            .message
            .as_ref()
            .map(|m| m.to_string())
            .unwrap_or(default_confirm_remove_message(Some(&*verify_id)));

        let input_panel = InputPanel::new()
            .class("pwt-p-4 pwt-flex-fit")
            .label_width("300px")
            .field_width("120px")
            .with_custom_child(html! {<span class="pwt-color-primary">{message}</span>})
            .with_field(
                tr!("Please enter the ID to confirm ({0})", verify_id),
                Field::new()
                    .autofocus(true)
                    .name("verify-id")
                    .required(true)
                    .validate({
                        let verify_id = verify_id.clone();
                        move |value: &String| -> Result<(), Error> {
                            if verify_id != value {
                                Err(Error::msg(tr!("Value does not match!")))
                            } else {
                                Ok(())
                            }
                        }
                    }),
            );

        let bbar = Toolbar::new().with_flex_spacer().with_child(
            SubmitButton::new()
                .text(props.submit_text.clone())
                .on_submit({
                    let on_close = props.on_close.clone();
                    move |form_ctx: FormContext| {
                        if let Some(on_close) = &on_close {
                            let confirm = form_ctx.read().get_field_text("verify-id") == verify_id;
                            on_close.emit(confirm);
                        }
                    }
                }),
        );

        let form = Form::new()
            .class("pwt-d-flex pwt-flex-direction-column ")
            .form_context(self.form_ctx.clone())
            .with_child(input_panel)
            .with_child(bbar);

        let title = match &props.title {
            Some(title) => title.to_string(),
            None => tr!("Confirm"),
        };

        Dialog::new(title)
            .on_close({
                let on_close = props.on_close.clone();
                move |_| {
                    if let Some(on_close) = &on_close {
                        on_close.emit(false);
                    }
                }
            })
            .with_child(form)
            .into()
    }
}

impl From<SafeConfirmDialog> for VNode {
    fn from(val: SafeConfirmDialog) -> Self {
        let comp = VComp::new::<ProxmoxSafeConfirmDialog>(Rc::new(val), None);
        VNode::from(comp)
    }
}
