use std::rc::Rc;

use anyhow::{bail, Error};
use serde_json::Value;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Field, FormContext};
use pwt::widget::{Button, InputPanel, Row};

use crate::percent_encoding::percent_encode_component;

use pwt_macros::builder;

use crate::{EditWindow, AuthidSelector};

fn extract_totp_link(form_ctx: &FormContext) -> String {
    let userid = form_ctx.read().get_field_text("userid");
    let issuer = form_ctx.read().get_field_text("issuer");
    let secret = form_ctx.read().get_field_text("secret");

    format!(
        "otpauth://totp/{}:{}?secret={secret}&period=30&digits=6&algorithm=SHA1&issuer={0}",
        percent_encode_component(&issuer),
        percent_encode_component(&userid),
    )
}

async fn create_item(form_ctx: FormContext, base_url: String) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();

    let userid = form_ctx.read().get_field_text("userid");

    let url = format!("{base_url}/{}", percent_encode_component(&userid));

    // Google Authenticator ignores period and digits and generates bogus data
    let totp_link = extract_totp_link(&form_ctx);

    data["type"] = "totp".into();
    data["totp"] = totp_link.into();

    let _: Value = crate::http_post(url, Some(data)).await?;
    Ok(())
}

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct TfaAddTotp {
    /// Close/Abort callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    #[prop_or("/access/tfa".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,
}

impl TfaAddTotp {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[doc(hidden)]
pub struct ProxmoxTfaAddTotp {
    default_secret: AttrValue,
}

fn render_input_form(form_ctx: FormContext, secret: AttrValue) -> Html {
    let totp_link = extract_totp_link(&form_ctx);

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
                .submit(false)
        )
        .with_field(
            tr!("Description"),
            Field::new()
                .name("description")
                .required(true)
                .autofocus(true)
                .placeholder(tr!(
                    "For example: TFA device ID, required to identify multiple factors."
                )),
        )
        .with_field(
            tr!("Issuer Name"),
            Field::new().name("issuer").submit(false).default("Proxmox"),
        )
        .with_field(
            tr!("Secret"),
            Field::new()
                .default(secret)
                .validate(validate_secret)
                .name("secret")
                .submit(false),
        )
        .with_custom_child(
            Row::new()
                .padding_bottom(2)
                .with_flex_spacer()
                .with_child(
                    Button::new(tr!("Randomize"))
                        .class("pwt-scheme-primary")
                        .onclick({
                            let form_ctx = form_ctx.clone();
                            move |_| {
                                let secret: Value = randomize_secret().into();
                                form_ctx.write().set_field_value("secret", secret);
                            }
                        })
                    )
        )
          .with_custom_child(
            html! {<div key="qrcode" style="text-align:center;">{render_qrcode(&totp_link)}</div>},
        )
        .with_field(
            tr!("Verify Code"),
            Field::new().name("value").required(true).placeholder(tr!(
                "Scan QR code in a TOTP app and enter an auth. code here"
            )),
        )
        .into()
}

impl Component for ProxmoxTfaAddTotp {
    type Message = ();
    type Properties = TfaAddTotp;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            default_secret: randomize_secret().into(),
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let base_url = props.base_url.to_string();
        let on_submit = {
            let base_url = base_url.clone();
            move |form_context| {
                let base_url = base_url.clone();
                async move { create_item(form_context, base_url.clone()).await }
            }
        };

        EditWindow::new(tr!("Add a TOTP login factor"))
            .renderer({
                let secret = self.default_secret.clone();
                move |form_ctx: &FormContext| {
                    render_input_form(form_ctx.clone(), secret.clone())
                }
            })
            .on_done(props.on_close.clone())
            .on_submit(on_submit)
            .into()
    }
}

impl Into<VNode> for TfaAddTotp {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxTfaAddTotp>(Rc::new(self), None);
        VNode::from(comp)
    }
}

fn validate_secret(secret: &String) -> Result<(), Error> {
    let invalid = secret
        .chars()
        .find(|c| !matches!(c, '2'..='7' | 'A'..='Z' | '='))
        .is_some();

    if invalid {
        bail!(tr!("Must be base32 [A-Z2-7=]"));
    }

    Ok(())
}

fn render_qrcode(text: &str) -> Html {
    let code = qrcode::QrCode::new(text).unwrap();
    let svg_xml = code.render::<qrcode::render::svg::Color>().build();
    let parsed = Html::from_html_unchecked(AttrValue::from(svg_xml));
    parsed
}

fn randomize_secret() -> String {
    let window = web_sys::window().unwrap();
    let mut rnd: [u8; 32] = [0u8; 32];

    let crypto = window.crypto().unwrap();
    let _ = crypto.get_random_values_with_u8_array(&mut rnd);

    let mut data = String::new();
    for b in rnd {
        let b = b & 0x1f;
        if b < 26 {
            // A..Z
            data.push(char::from(b + 0x41));
        } else {
            // 2..7
            data.push(char::from(b - 26 + 0x32));
        }
    }
    data
}
