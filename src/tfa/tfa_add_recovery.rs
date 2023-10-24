use std::rc::Rc;

use anyhow::Error;
use serde::Deserialize;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::FormContext;
use pwt::widget::{Button, Column, Container, Dialog, InputPanel, Toolbar};

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

pub enum Msg {
    RecoveryKeys(Vec<String>),
    ShowKeys,
}

#[doc(hidden)]
pub struct ProxmoxTfaAddRecovery {
    recovery_keys: Option<Vec<String>>,
}

fn render_input_form(_form_ctx: FormContext) -> Html {
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

impl ProxmoxTfaAddRecovery {
    fn recovery_keys_dialog(&self, ctx: &Context<Self>, keys: &[String]) -> Html {
        let text: String = keys
            .iter()
            .enumerate()
            .map(|(i, key)| format!("{i}: {key}\n"))
            .collect();

        Dialog::new(tr!("Recovery Keys"))
            .on_close(ctx.props().on_close.clone())
            .with_child(
                Column::new()
                    .with_child(
                        Container::new()
                            .padding(2)
                            .with_child(
                                Container::new()
                                    .tag("pre")
                                    .class("pwt-font-monospace")
                                    .padding(2)
                                    .border(true)
                                    .with_child(text),
                            )
                            .with_child(
                                Container::new()
                                    .class("pwt-color-warning")
                                    .padding_y(2)
                                    .with_child(tr!(
                                    "Please record recovery keys - they will only be displayed now"
                                )),
                            ),
                    )
                    .with_child(
                        Toolbar::new()
                            .with_flex_spacer()
                            .with_child(Button::new("TEST").class("pwt-scheme-primary")),
                    ),
            )
            .into()
    }
}

impl Component for ProxmoxTfaAddRecovery {
    type Message = Msg;
    type Properties = TfaAddRecovery;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            recovery_keys: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::RecoveryKeys(keys) => {
                self.recovery_keys = Some(keys);
                true
            }
            Msg::ShowKeys => {
                if self.recovery_keys.is_none() {
                    if let Some(on_close) = &props.on_close {
                        on_close.emit(());
                    }
                }
                true
            }
        }
    }
    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        if let Some(keys) = &self.recovery_keys {
            return self.recovery_keys_dialog(ctx, keys);
        }

        let base_url = props.base_url.to_string();
        let on_submit = {
            let base_url = base_url.clone();
            let link = ctx.link().clone();
            move |form_context| {
                let base_url = base_url.clone();
                let link = link.clone();
                async move {
                    let data = create_item(form_context, base_url.clone()).await?;
                    link.send_message(Msg::RecoveryKeys(data.recovery));
                    Ok(())
                }
            }
        };

        EditWindow::new(tr!("Add") + ": " + &tr!("TFA recovery keys"))
            .renderer(|form_ctx: &FormContext| render_input_form(form_ctx.clone()))
            .on_done(ctx.link().callback(|_| Msg::ShowKeys))
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
