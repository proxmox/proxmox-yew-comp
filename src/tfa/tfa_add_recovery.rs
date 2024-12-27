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

use crate::utils::copy_to_clipboard;
use crate::{AuthidSelector, EditWindow};

#[derive(Debug, Deserialize)]
struct RecoveryKeyList {
    recovery: Vec<String>,
}

async fn create_item(form_ctx: FormContext, base_url: String) -> Result<RecoveryKeyInfo, Error> {
    let mut data = form_ctx.get_submit_data();

    let userid = form_ctx.read().get_field_text("userid");

    let url = format!("{base_url}/{}", percent_encode_component(&userid));

    data["type"] = "recovery".into();

    let res: RecoveryKeyList = crate::http_post(url, Some(data)).await?;

    Ok(RecoveryKeyInfo {
        userid,
        keys: res.recovery,
    })
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

impl Default for TfaAddRecovery {
    fn default() -> Self {
        Self::new()
    }
}

impl TfaAddRecovery {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[derive(Clone)]
pub struct RecoveryKeyInfo {
    userid: String,
    keys: Vec<String>,
}

#[allow(clippy::enum_variant_names)]
pub enum Msg {
    RecoveryKeys(RecoveryKeyInfo),
    ShowKeys,
    PrintKeys,
}

#[doc(hidden)]
pub struct ProxmoxTfaAddRecovery {
    recovery_keys: Option<RecoveryKeyInfo>,
    container_ref: NodeRef,
    print_counter: usize,
    print_portal: Option<Html>,
}

fn render_input_form(_form_ctx: FormContext) -> Html {
    let panel = InputPanel::new()
        .min_width(600)
        .label_width("120px")
        .padding(4)
        .with_field(
            tr!("User"),
            AuthidSelector::new()
                .include_tokens(false)
                .default(crate::http_get_auth().map(|auth| auth.userid))
                .name("userid")
                .required(true)
                .submit(false),
        );

    super::add_password_field(panel, false).into()
}

impl ProxmoxTfaAddRecovery {
    fn recovery_keys_dialog(&self, ctx: &Context<Self>, data: &RecoveryKeyInfo) -> Html {
        use std::fmt::Write;
        let text: String = data
            .keys
            .iter()
            .enumerate()
            .fold(String::new(), |mut acc, (i, key)| {
                let _ = writeln!(acc, "{i}: {key}\n");
                acc
            });

        Dialog::new(tr!("Recovery Keys for user '{}'", data.userid))
            .on_close(ctx.props().on_close.clone())
            .with_child(
                Column::new()
                    .with_child(
                        Container::new()
                            .padding(2)
                            .with_child(
                                Container::from_tag("pre")
                                    .node_ref(self.container_ref.clone())
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
                            .with_child(
                                Button::new(tr!("Copy Recovery Keys"))
                                    .icon_class("fa fa-clipboard")
                                    .class("pwt-scheme-primary")
                                    .onclick({
                                        let container_ref = self.container_ref.clone();
                                        move |_| copy_to_clipboard(&container_ref)
                                    }),
                            )
                            .with_child(
                                Button::new(tr!("Print Recovery Keys"))
                                    .icon_class("fa fa-print")
                                    .class("pwt-scheme-primary")
                                    .onclick(ctx.link().callback(|_| Msg::PrintKeys)),
                            ),
                    )
                    .with_optional_child(self.print_portal.clone()),
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
            container_ref: NodeRef::default(),
            print_portal: None,
            print_counter: 0,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::RecoveryKeys(data) => {
                self.recovery_keys = Some(data);
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
            Msg::PrintKeys => {
                if let Some(data) = &self.recovery_keys {
                    let window = web_sys::window().unwrap();
                    let document = window.document().unwrap();
                    let body = document.body().unwrap();
                    let print_page = create_paperkey_page(data, self.print_counter);
                    self.print_counter += 1;
                    self.print_portal = Some(create_portal(print_page, body.into()));
                }
                true
            }
        }
    }
    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        if let Some(data) = &self.recovery_keys {
            return self.recovery_keys_dialog(ctx, data);
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
                    link.send_message(Msg::RecoveryKeys(data));
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

impl From<TfaAddRecovery> for VNode {
    fn from(val: TfaAddRecovery) -> Self {
        let comp = VComp::new::<ProxmoxTfaAddRecovery>(Rc::new(val), None);
        VNode::from(comp)
    }
}

fn create_paperkey_page(data: &RecoveryKeyInfo, print_counter: usize) -> Html {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    let userid = data.userid.clone();
    let title = document.title();
    let host = document.location().unwrap().host().unwrap();

    use std::fmt::Write;
    let key_text: String = data
        .keys
        .iter()
        .enumerate()
        .fold(String::new(), |mut acc, (i, key)| {
            let _ = writeln!(acc, "{i}: {key}");
            acc
        });

    let html = format!(
        r###"
    <html>
        <head>
            <script>
                window.addEventListener('DOMContentLoaded', (ev) => window.print());
            </script>
            <style>@media print and (max-height: 150mm) {{
                h4, p  {{ margin: 0; font-size: 1em; }}
            }}</style>
        </head>
        <body style="padding: 5px;">
            <h4>Recovery Keys for '{userid}' - {title} ({host})</h4>
            <p style="font-size:1.5em;line-height:1.5em;font-family:monospace;white-space:pre-wrap;overflow-wrap:break-word;">
{key_text}
            </p>
        </body>
    </html>`;
"###
    );

    let data_url = format!("data:text/html;base64,{}", base64::encode(html));

    html! {<iframe key={print_counter} src={data_url}/>}
}
