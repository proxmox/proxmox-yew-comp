use std::rc::Rc;

use anyhow::{bail, Error};
use serde_json::json;

use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Boolean, Field, FormContext, ValidateFn};
use pwt::widget::InputPanel;

use crate::EditWindow;

use super::{AcmeDirectoryListItem, AcmeDirectorySelector};

use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct AcmeRegisterAccount {
    /// Done callback, called after Close, Abort or Submit.
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_done: Option<Callback<()>>,
}

impl AcmeRegisterAccount {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub enum Msg {
    AcmeDirectory(Option<AcmeDirectoryListItem>),
    TermsOfService(Option<Result<String, String>>),
}

#[doc(hidden)]
pub struct ProxmoxAcmeRegisterAccount {
    validate_tos: ValidateFn<bool>,
    tos_url: Option<Result<String, String>>,
}

impl Component for ProxmoxAcmeRegisterAccount {
    type Message = Msg;
    type Properties = AcmeRegisterAccount;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            validate_tos: ValidateFn::new(|value: &bool| {
                if !value {
                    bail!("Please accept the Terms Of Service")
                }
                Ok(())
            }),
            tos_url: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::TermsOfService(tos) => {
                self.tos_url = tos;
                true
            }
            Msg::AcmeDirectory(entry) => {
                self.tos_url = None;
                let link = ctx.link().clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let msg = if let Some(entry) = entry {
                        let param = Some(json!({ "directory": &entry.url }));
                        let tos: Result<String, Error> =
                            crate::http_get("/config/acme/tos", param).await;
                        Msg::TermsOfService(Some(tos.map_err(|err| err.to_string())))
                    } else {
                        Msg::TermsOfService(None)
                    };
                    link.send_message(msg);
                });
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let link = ctx.link().clone();
        let validate_tos = self.validate_tos.clone();
        let tos_url = self.tos_url.clone();

        EditWindow::new(tr!("Register Account"))
            .on_done(props.on_done.clone())
            .renderer(move |form_ctx: &FormContext| {
                let mut panel = InputPanel::new()
                    .attribute("style", "width: 500px;")
                    .class("pwt-flex-fit pwt-p-4")
                    .with_field(
                        tr!("Account Name"),
                        Field::new().name("name").placeholder("default"),
                    )
                    .with_field(tr!("E-Mail"), Field::new().name("contact").required(true))
                    .with_field(
                        tr!("Directory"),
                        AcmeDirectorySelector::new()
                            .name("directory")
                            .required(true)
                            .on_change(link.callback(Msg::AcmeDirectory)),
                    );

                let has_acme_dir = !form_ctx.read().get_field_text("directory").is_empty();

                if has_acme_dir {

                    panel.add_custom_child(html!{
                        <div key="tos_header" class="pwt-pt-4">
                        {tr!("Terms Of Service")}
                        </div>
                    });

                    match &tos_url {
                        Some(Ok(tos_url)) => {
                            panel.add_custom_child(
                                html! {<a key="tos_url" target="_blank" href={tos_url.clone()}>{&tos_url}</a>},
                            );
                            panel.add_field(
                                false,
                                tr!("Accept TOS"),
                                Boolean::new()
                                    .name("tos_checkbox")
                                    .submit(false)
                                    .validate(validate_tos.clone()),
                            );
                        }
                        Some(Err(err)) => {
                            let msg = pwt::widget::error_message(&tr!("Loading TOS failed: {0}", err));
                            panel.add_custom_child(html! {<span key="tos_url">{msg}</span>});
                            panel.add_field(
                                false,
                                tr!("Accept TOS"),
                                Boolean::new()
                                    .name("tos_checkbox_disabled")
                                    .disabled(true)
                                    .submit(false)
                                    .validate(validate_tos.clone()),
                            );
                        }
                        None => {
                            if has_acme_dir {
                                panel.add_custom_child(html! {<span key="tos_url">{tr!("Loading")}</span>});
                            }
                        }
                    }
                }

                panel.into()
            })
            .on_submit({
                let tos_url = self.tos_url.clone();
                move |form_ctx: FormContext| {
                    let mut data = form_ctx.get_submit_data();
                    if let Some(Ok(tos_url)) = &tos_url {
                        data["tos_url"] = tos_url.clone().into();
                    }
                    async move {
                        let upid = crate::http_post("/config/acme/account", Some(data)).await;
                        crate::http_task_result(upid).await?;
                        Ok(())
                    }
                }
            })
            .into()
    }
}

impl Into<VNode> for AcmeRegisterAccount {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxAcmeRegisterAccount>(Rc::new(self), None);
        VNode::from(comp)
    }
}
