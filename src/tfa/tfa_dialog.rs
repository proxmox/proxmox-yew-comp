use std::rc::Rc;

use anyhow::{bail, Context as _, Error};
use derivative::Derivative;
use wasm_bindgen::JsValue;

use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Field, Form, FormContext, SubmitButton};
use pwt::widget::{Dialog, SelectionViewRenderInfo, TabBarItem, TabPanel};

use pwt_macros::builder;

use proxmox_login::SecondFactorChallenge;

use super::WebAuthn;

#[derive(Derivative)]
#[derivative(Clone, PartialEq)]
#[derive(Properties)]
#[builder]
pub struct TfaDialog {
    /// The TFA challenge returned by the server.
    #[derivative(PartialEq(compare_with = "Rc::ptr_eq"))]
    challenge: Rc<SecondFactorChallenge>,

    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_totp: Option<Callback<String>>,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_yubico: Option<Callback<String>>,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_recovery: Option<Callback<String>>,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_webauthn: Option<Callback<String>>,

    /// Mobile Layout
    ///
    /// - do not set dialog min-width and min-height
    #[prop_or(false)]
    #[builder]
    pub mobile: bool,
}

impl TfaDialog {
    /// Create a new instance with TFA challenge returned by the server.
    pub fn new(challenge: Rc<SecondFactorChallenge>) -> Self {
        yew::props!(Self { challenge })
    }
}

#[derive(Default)]
pub struct ProxmoxTfaDialog {
    error: Option<String>,
    webauthn_challenge: Option<(JsValue, String)>,
}

fn render_totp(callback: Option<Callback<String>>) -> Html {
    Form::new()
        .padding(2)
        .class("pwt-flex-fill pwt-d-flex pwt-flex-direction-column pwt-gap-2")
        .with_child(html! {<div>{"Please enter your TOTP verification code"}</div>})
        .with_child(Field::new().name("data").required(true).autofocus(true))
        .with_child(html! {<div style="flex: 1 1 auto;"/>})
        .with_child(
            SubmitButton::new()
                .class("pwt-align-self-flex-end")
                .class("pwt-scheme-primary")
                .text("Confirm")
                .on_submit({
                    move |form_ctx: FormContext| {
                        let data = form_ctx.read().get_field_text("data");
                        if let Some(callback) = &callback {
                            callback.emit(data);
                        }
                    }
                }),
        )
        .into()
}

fn render_yubico(callback: Option<Callback<String>>) -> Html {
    Form::new()
        .padding(2)
        .class("pwt-flex-fill pwt-d-flex pwt-flex-direction-column pwt-gap-2")
        .with_child(html! {<div>{"Please enter your Yubico OTP code"}</div>})
        .with_child(Field::new().name("data").required(true).autofocus(true))
        .with_child(html! {<div style="flex: 1 1 auto;"/>})
        .with_child(
            SubmitButton::new()
                .class("pwt-align-self-flex-end")
                .class("pwt-scheme-primary")
                .text("Confirm")
                .on_submit({
                    move |form_ctx: FormContext| {
                        let data = form_ctx.read().get_field_text("data");
                        if let Some(callback) = &callback {
                            callback.emit(data);
                        }
                    }
                }),
        )
        .into()
}

fn render_recovery(callback: Option<Callback<String>>, available_keys: &[usize]) -> Html {
    if available_keys.is_empty() {
        let msg = "No more recovery keys available.";
        return pwt::widget::error_message(msg).padding(2).into();
    }

    Form::new()
        .padding(2)
        .class("pwt-flex-fill pwt-d-flex pwt-flex-direction-column pwt-gap-2")
        .with_child(html! {<div>{"Please enter one of your single-use recovery keys"}</div>})
        .with_child(html! {<div>{format!{"Available recovery keys: {:?}", available_keys}}</div>})
        .with_child(Field::new().name("data").required(true).autofocus(true))
        .with_optional_child((available_keys.len() <= 13).then(|| {
            html! {<div>{
                format!(
                    "Less than {0} recovery keys available. Please generate a new set after login!",
                    available_keys.len() + 1,
                )
            }</div>}
        }))
        .with_child(html! {<div style="flex: 1 1 auto;"/>})
        .with_child(
            SubmitButton::new()
                .class("pwt-align-self-flex-end")
                .class("pwt-scheme-primary")
                .text("Confirm")
                .on_submit({
                    move |form_ctx: FormContext| {
                        let data = form_ctx.read().get_field_text("data");
                        if let Some(callback) = &callback {
                            callback.emit(data);
                        }
                    }
                }),
        )
        .into()
}

impl Component for ProxmoxTfaDialog {
    type Message = ();
    type Properties = TfaDialog;

    fn create(ctx: &Context<Self>) -> Self {
        let mut this = Self::default();

        if let Some(challenge) = &ctx.props().challenge.challenge.webauthn_raw {
            this.error = 'err: {
                let challenge = match js_sys::JSON::parse(challenge) {
                    Ok(c) => c,
                    Err(err) => {
                        break 'err Some(format!("failed to parse webauthn challenge: {err:?}"));
                    }
                };

                let challenge_string = match fixup_challenge(&challenge) {
                    Ok(s) => s,
                    Err(err) => {
                        break 'err Some(format!("failed to prepare webauthn challenge: {err:?}"));
                    }
                };

                this.webauthn_challenge = Some((challenge, challenge_string));
                None
            };
        }

        this
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let mut panel = TabPanel::new().class("pwt-flex-fill");

        if props.challenge.challenge.totp {
            panel.add_item_builder(TabBarItem::new().key("totp").label("TOTP App"), {
                let on_totp = props.on_totp.clone();
                move |_| render_totp(on_totp.clone())
            });
        }

        if props.challenge.challenge.yubico {
            panel.add_item_builder(TabBarItem::new().key("yubico").label("Yubico OTP"), {
                let on_yubico = props.on_yubico.clone();
                move |_| render_yubico(on_yubico.clone())
            });
        }

        if props.challenge.challenge.recovery.is_available() {
            panel.add_item_builder(TabBarItem::new().key("recovery").label("Recovery Key"), {
                let on_recovery = props.on_recovery.clone();
                let available_keys = props.challenge.challenge.recovery.0.clone();
                move |_| render_recovery(on_recovery.clone(), &available_keys)
            });
        }

        /*
        // FIXME: switch to decoded value when we have a wasm-compatible webauthn-rs crate.
        if props.challenge.challenge.webauthn.is_some() {
            panel.add_item_builder(TabBarItem::new().key("webauthn").label("WebAuthn"), {
                let on_webauthn = props.on_webauthn.clone();
                move |info: &SelectionViewRenderInfo| {
                    WebAuthn::new()
                        .visible(info.visible)
                        .on_webauthn(on_webauthn.clone())
                        .into()
                }
            });
        }
        */
        if let Some((challenge, challenge_string)) = self.webauthn_challenge.clone() {
            panel.add_item_builder(TabBarItem::new().key("webauthn").label("WebAuthn"), {
                let on_webauthn = props.on_webauthn.clone();
                move |info: &SelectionViewRenderInfo| {
                    WebAuthn::new()
                        .visible(info.visible)
                        .on_webauthn(on_webauthn.clone())
                        .challenge(challenge.clone())
                        .challenge_string(challenge_string.clone())
                        .into()
                }
            });
        }

        let mut dialog = Dialog::new("Second login factor required")
            .resizable(true)
            .with_child(panel)
            .on_close(props.on_close.clone());

        if !props.mobile {
            dialog.set_min_width(600);
            dialog.set_min_height(300);
        }

        dialog.into()
    }
}

impl From<TfaDialog> for VNode {
    fn from(val: TfaDialog) -> Self {
        let comp = VComp::new::<ProxmoxTfaDialog>(Rc::new(val), None);
        VNode::from(comp)
    }
}

fn fixup_challenge(value: &JsValue) -> Result<String, Error> {
    use js_sys::Reflect;
    use wasm_bindgen::JsCast;

    /*
    challenge.publicKey.challenge = Proxmox.Utils.base64url_to_bytes(challenge.string);
    for (const cred of challenge.publicKey.allowCredentials) {
        cred.id = Proxmox.Utils.base64url_to_bytes(cred.id);
    }
    */

    let public_key = Reflect::get(value, &"publicKey".into())
        .ok()
        .context("missing 'publicKey' value in webauthn challenge")?;
    let challenge_string = turn_b64u_member_into_buffer(&public_key, "challenge")?;

    let allow_credentials = Reflect::get(&public_key, &"allowCredentials".into())
        .ok()
        .context("failed to query list of allowed credentials")?
        .dyn_into::<js_sys::Array>()
        .ok()
        .context("allowed credential list was not an array")?;
    for cred in allow_credentials {
        turn_b64u_member_into_buffer(&cred, "id")?;
    }

    Ok(challenge_string)
}

/// For convenience this returns the previous base64url string so we can keep the 'challenge'
/// member around.
pub(super) fn turn_b64u_member_into_buffer(
    obj: &JsValue,
    member_str: &str,
) -> Result<String, Error> {
    use js_sys::Reflect;

    let member = member_str.into();

    let mem_string = Reflect::get(obj, &member)
        .ok()
        .with_context(|| format!("failed to get '{member_str}' in object"))?
        .as_string()
        .with_context(|| format!("'{member_str}' in object was not a string"))?;
    let buffer: js_sys::Uint8Array = proxmox_base64::url::decode_no_pad(&mem_string)
        .with_context(|| format!("failed to decode '{member_str}'"))?[..]
        .into();

    if !Reflect::set(obj, &member, &buffer)
        .ok()
        .with_context(|| format!("failed to turn '{member_str}' into buffer"))?
    {
        bail!("failed to place buffer in '{member_str}' in object");
    }

    Ok(mem_string)
}
