use std::rc::Rc;

use anyhow::Error;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Boolean, Field, FormContext};
use pwt::widget::InputPanel;

use crate::percent_encoding::percent_encode_component;

use pwt_macros::builder;

use crate::EditWindow;

async fn update_item(form_ctx: FormContext, url: String) -> Result<(), Error> {
    let data = form_ctx.get_submit_data();
    crate::http_put(&url, Some(data)).await
}

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct TfaEdit {
    /// Close/Abort callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    #[prop_or("/access/tfa".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,

    user_id: AttrValue,
    entry_id: AttrValue,
}

impl TfaEdit {
    pub fn new(user_id: AttrValue, entry_id: AttrValue) -> Self {
        yew::props!(Self { user_id, entry_id })
    }
}

#[doc(hidden)]
pub struct ProxmoxTfaEdit {}

fn render_input_form(_form_ctx: FormContext, props: TfaEdit) -> Html {
    InputPanel::new()
        .padding(4)
        .with_field(
            tr!("User"),
            Field::new()
                .value(props.user_id)
                .required(true)
                .disabled(true)
                .submit(false),
        )
        .with_large_field(
            tr!("Description"),
            Field::new()
                .name("description")
                .autofocus(true)
                .submit_empty(true),
        )
        .with_field(tr!("Enabled"), Boolean::new().name("enable").default(true))
        .into()
}

impl Component for ProxmoxTfaEdit {
    type Message = ();
    type Properties = TfaEdit;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let url = format!(
            "{}/{}/{}",
            props.base_url,
            percent_encode_component(&*props.user_id),
            percent_encode_component(&*props.entry_id),
        );

        let on_submit = {
            let url = url.clone();
            move |form_context| {
                let url = url.clone();
                async move { update_item(form_context, url.clone()).await }
            }
        };

        EditWindow::new(tr!("Modify a TFA entry's description"))
            .loader(url)
            .renderer({
                let props = props.clone();
                move |form_ctx: &FormContext| render_input_form(form_ctx.clone(), props.clone())
            })
            .on_done(props.on_close.clone())
            .on_submit(on_submit)
            .into()
    }
}

impl Into<VNode> for TfaEdit {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxTfaEdit>(Rc::new(self), None);
        VNode::from(comp)
    }
}
