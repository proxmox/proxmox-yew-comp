use std::rc::Rc;

use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{DisplayField, Field, FormContext};
use pwt::widget::InputPanel;

use crate::utils::render_epoch;
use crate::EditWindow;

use pwt_macros::builder;

use super::tfa_view::TfaEntry;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct TfaConfirmRemove {
    /// Close/Abort callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    /// Confirm callback
    #[builder_cb(IntoEventCallback, into_event_callback, Option<String>)]
    #[prop_or_default]
    pub on_confirm: Option<Callback<Option<String>>>,

    /// The TFA Entry from the TFA View.
    entry: TfaEntry,
}

impl TfaConfirmRemove {
    pub(super) fn new(entry: TfaEntry) -> Self {
        yew::props!(Self { entry })
    }
}

#[doc(hidden)]
pub struct ProxmoxTfaConfirmRemove {}

fn render_input_form(_form_ctx: FormContext, entry: TfaEntry) -> Html {
    let message = tr!("Are you sure you want to remove this TFA entry?");

    let panel = InputPanel::new()
        .padding(4)
        .class("pwt-flex-fit")
        .with_large_custom_child(html! { {message} })
        .with_field(tr!("User"), DisplayField::new().value(entry.user_id))
        .with_field(
            tr!("Type"),
            DisplayField::new().value(entry.tfa_type.to_string()),
        )
        .with_right_field(
            tr!("Created"),
            DisplayField::new().value(render_epoch(entry.created)),
        )
        .with_right_field(
            tr!("Description"),
            Field::new()
                .name("description")
                .disabled(true)
                .value(entry.description),
        );

    super::add_password_field(panel, true).into()
}

impl Component for ProxmoxTfaConfirmRemove {
    type Message = ();
    type Properties = TfaConfirmRemove;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let on_confirm = props.on_confirm.clone();
        let on_submit = {
            move |form_ctx: FormContext| {
                let on_confirm = on_confirm.clone();
                let password = form_ctx.read().get_field_text("password");
                let password = (!password.is_empty()).then_some(password);
                async move {
                    if let Some(on_confirm) = on_confirm {
                        on_confirm.emit(password);
                    }
                    Ok(())
                }
            }
        };

        let entry = props.entry.clone();
        EditWindow::new(tr!("Confirm") + ": " + &tr!("TFA Removal"))
            .renderer(move |form_ctx: &FormContext| {
                render_input_form(form_ctx.clone(), entry.clone())
            })
            .on_done(props.on_close.clone())
            .on_submit(on_submit)
            .submit_text("Remove")
            .into()
    }
}

impl From<TfaConfirmRemove> for VNode {
    fn from(val: TfaConfirmRemove) -> Self {
        let comp = VComp::new::<ProxmoxTfaConfirmRemove>(Rc::new(val), None);
        VNode::from(comp)
    }
}
