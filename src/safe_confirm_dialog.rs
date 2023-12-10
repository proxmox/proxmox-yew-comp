use std::rc::Rc;

use anyhow::Error;

use pwt::props::RenderFn;
use pwt::widget::InputPanel;
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Field, Form, FormContext, SubmitButton, ValidateFn};
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

    // Form renderer, to display additional data.
    #[prop_or_default]
    pub renderer: Option<RenderFn<FormContext>>,

    /// Close window callback.
    ///
    /// Parameter is set to true if the user confirmed the action.
    #[prop_or_default]
    #[builder_cb(IntoEventCallback, into_event_callback, bool)]
    pub on_close: Option<Callback<bool>>,

    /// Confirm callback.
    #[prop_or_default]
    #[builder_cb(IntoEventCallback, into_event_callback, FormContext)]
    pub on_confirm: Option<Callback<FormContext>>,

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

    pub fn renderer(mut self, renderer: impl 'static + Fn(&FormContext) -> Html) -> Self {
        self.renderer = Some(RenderFn::new(renderer));
        self
    }
}

#[doc(hidden)]
pub struct ProxmoxSafeConfirmDialog {
    form_ctx: FormContext,
    validate: ValidateFn<String>,
}

impl Component for ProxmoxSafeConfirmDialog {
    type Message = ();
    type Properties = SafeConfirmDialog;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        let validate = ValidateFn::new({
            let verify_id = props.verify_id.clone();
            move |value: &String| -> Result<(), Error> {
                if verify_id != value {
                    Err(Error::msg(tr!("Value does not match!")))
                } else {
                    Ok(())
                }
            }
        });

        let form_ctx = FormContext::new().on_change(ctx.link().callback(|_| ()));

        Self {
            form_ctx,
            validate,
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
            .with_custom_child(html! {<span key="message" class="pwt-color-primary">{message}</span>})
            .with_field(
                tr!("Please enter the ID to confirm ({0})", verify_id),
                Field::new()
                    .autofocus(true)
                    .name("verify-id")
                    .required(true)
                    .submit(false)
                    .validate(self.validate.clone()),
            );

        let bbar = Toolbar::new().with_flex_spacer().with_child(
            SubmitButton::new()
                .text(props.submit_text.clone())
                .on_submit({
                    let on_confirm = props.on_confirm.clone();
                    move |form_ctx: FormContext| {
                        if let Some(on_confirm) = &on_confirm {
                            let confirm = form_ctx.read().get_field_text("verify-id") == verify_id;
                            if confirm {
                                on_confirm.emit(form_ctx.clone());
                            }
                        }
                    }
                }),
        );

        let additional_content = props.renderer.as_ref().map({
            let form_ctx = self.form_ctx.clone();
            move |render_fn| render_fn.apply(&form_ctx)
        });

        let form = Form::new()
            .class("pwt-d-flex pwt-flex-direction-column ")
            .form_context(self.form_ctx.clone())
            .with_child(input_panel)
            .with_optional_child(additional_content)
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
