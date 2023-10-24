use std::rc::Rc;

use anyhow::Error;
use serde::Deserialize;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::FormContext;
use pwt::widget::InputPanel;

use crate::percent_encoding::percent_encode_component;

use pwt_macros::builder;

use crate::{AuthidSelector, EditWindow};

#[derive(Debug, Deserialize)]
struct RecoveryKeyList {
    recovery: Vec<String>,
}

async fn create_item(form_ctx: FormContext, base_url: String) -> Result<RecoveryKeyList, Error> {
    let mut data = form_ctx.get_submit_data();

    let userid = form_ctx.read().get_field_text("userid");

    let url = format!("{base_url}/{}", percent_encode_component(&userid));

    data["type"] = "recovery".into();

    crate::http_post(url, Some(data)).await
}

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct TfaAddRecovery {
    /// Close/Abort callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    #[prop_or("/access/tfa".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,
}

impl TfaAddRecovery {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[doc(hidden)]
pub struct ProxmoxTfaAddRecovery {}

fn render_input_form(form_ctx: FormContext) -> Html {
    InputPanel::new()
        .attribute("style", "min-width: 600px;")
        .label_width("120px")
        .class("pwt-p-4")
        .with_field(
            tr!("User"),
            AuthidSelector::new()
                .include_tokens(false)
                .name("userid")
                .required(true)
                .submit(false),
        )
        .into()
}

impl Component for ProxmoxTfaAddRecovery {
    type Message = ();
    type Properties = TfaAddRecovery;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let base_url = props.base_url.to_string();
        let on_submit = {
            let base_url = base_url.clone();
            move |form_context| {
                let base_url = base_url.clone();
                async move {
                    let data = create_item(form_context, base_url.clone()).await?;
                    log::info!("GOT {:?}", data);
                    Ok(())
                }
            }
        };

        EditWindow::new(tr!("Add") + ": " + &tr!("TFA recovery keys"))
            .renderer(|form_ctx: &FormContext| render_input_form(form_ctx.clone()))
            .on_done(props.on_close.clone())
            .on_submit(on_submit)
            .into()
    }
}

impl Into<VNode> for TfaAddRecovery {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxTfaAddRecovery>(Rc::new(self), None);
        VNode::from(comp)
    }
}
