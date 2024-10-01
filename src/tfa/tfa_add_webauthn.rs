use std::rc::Rc;

use anyhow::{Context as _, Error};
use serde_json::json;
use wasm_bindgen::JsValue;

use proxmox_tfa::TfaUpdateInfo;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::convert_js_error;
use pwt::prelude::*;
use pwt::widget::form::{Field, FormContext};
use pwt::widget::InputPanel;

use crate::percent_encoding::percent_encode_component;

use pwt_macros::builder;

use crate::{AuthidSelector, EditWindow};

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct TfaAddWebauthn {
    /// Close/Abort callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    #[prop_or("/access/tfa".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,
}

impl TfaAddWebauthn {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[doc(hidden)]
pub struct ProxmoxTfaAddWebauthn {}

fn render_input_form(_form_ctx: FormContext) -> Html {
    InputPanel::new()
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
        .into()
}

impl Component for ProxmoxTfaAddWebauthn {
    type Message = ();
    type Properties = TfaAddWebauthn;

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
                async move { create_item(form_context, base_url.clone()).await }
            }
        };

        EditWindow::new(tr!("Add") + ": " + &tr!("Webauthn Device"))
            .renderer(move |form_ctx: &FormContext| render_input_form(form_ctx.clone()))
            .on_done(props.on_close.clone())
            .submit_text(tr!("Register Webauthn Device"))
            .on_submit(on_submit)
            .into()
    }
}

impl Into<VNode> for TfaAddWebauthn {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxTfaAddWebauthn>(Rc::new(self), None);
        VNode::from(comp)
    }
}

async fn create_item(form_ctx: FormContext, base_url: String) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();
    let password = data["password"].clone();

    let userid = form_ctx.read().get_field_text("userid");

    let url = format!("{base_url}/{}", percent_encode_component(&userid));

    data["type"] = "webauthn".into();
    let update: TfaUpdateInfo = crate::http_post(&url, Some(data)).await?;

    let challenge = js_sys::JSON::parse(
        &update
            .challenge
            .context("missing webauthn challenge in response")?,
    )
    .map_err(convert_js_error)
    .context(tr!("failed to parse webauthn registration challenge"))?;

    let challenge_string = fixup_challenge(&challenge)?;

    let promise = super::webauthn::WasmWindow::from(web_sys::window().unwrap())
        .navigator()
        .credentials()
        .create(&challenge)
        .map_err(convert_js_error)?;

    let hw_rsp = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(convert_js_error)?;

    let response_string = handle_hw_rsp(hw_rsp)?;

    let mut response = json!({
        "type": "webauthn",
        "challenge": challenge_string,
        "value": response_string,
    });

    if !password.is_null() {
        response["password"] = password;
    }

    let _: TfaUpdateInfo = crate::http_post(url, Some(response)).await?;

    Ok(())
}

fn handle_hw_rsp(hw_rsp: JsValue) -> Result<String, Error> {
    use js_sys::Reflect;

    fn get_string(value: &JsValue, name: &str) -> Result<String, Error> {
        Reflect::get(value, &name.into())
            .ok()
            .with_context(|| format!("missing '{name}' property in hardware response"))?
            .as_string()
            .with_context(|| format!("'{name}' in hardware response was not a string"))
    }
    fn get_b64u(value: &JsValue, name: &str) -> Result<String, Error> {
        let value = Reflect::get(value, &name.into())
            .ok()
            .with_context(|| format!("missing '{name}' property in hardware response"))?;
        let bytes = js_sys::Uint8Array::new(&value).to_vec();
        Ok(base64::encode_config(&bytes, base64::URL_SAFE_NO_PAD))
    }

    let id = get_string(&hw_rsp, "id")?;
    let ty = get_string(&hw_rsp, "type")?;
    let raw_id = get_b64u(&hw_rsp, "rawId")?;
    let response = Reflect::get(&hw_rsp, &"response".into())
        .ok()
        .context("missing 'response' property in hardware response")?;
    let attestation_object = get_b64u(&response, "attestationObject")?;
    let client_data_json = get_b64u(&response, "clientDataJSON")?;

    serde_json::to_string(&serde_json::json!({
        "id": id,
        "type": ty,
        "rawId": raw_id,
        "response": {
            "attestationObject": attestation_object,
            "clientDataJSON": client_data_json,
        },
    }))
    .context("failed to build response json object")
}

fn fixup_challenge(value: &JsValue) -> Result<String, Error> {
    use js_sys::Reflect;
    use wasm_bindgen::JsCast;

    use super::tfa_dialog::turn_b64u_member_into_buffer;

    let public_key = Reflect::get(value, &"publicKey".into())
        .ok()
        .context("missing 'publicKey' value in webauthn challenge")?;
    let challenge_string = turn_b64u_member_into_buffer(&public_key, "challenge")?;

    let user = Reflect::get(&public_key, &"user".into())
        .ok()
        .context("missing 'publicKey.user' value in webauthn challenge")?;
    turn_b64u_member_into_buffer(&user, "id")?;

    let exclude_credentials = Reflect::get(&public_key, &"excludeCredentials".into())
        .ok()
        .context("failed to query list of excluded credentials")?
        .dyn_into::<js_sys::Array>()
        .ok()
        .context("excluded credentials list was not an array")?;
    for cred in exclude_credentials {
        turn_b64u_member_into_buffer(&cred, "id")?;
    }

    Ok(challenge_string)
}
