use std::rc::Rc;

use anyhow::{Context as _, Error};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;
use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::convert_js_error;
use pwt::widget::Mask;
use pwt::{
    prelude::*,
    widget::{Button, Column},
};
use pwt_macros::builder;

//
// Web API definition:
//
// We define this manually, since we'd need a lot of Provides in the debian package and want to
// switch over to webauthn-rs at some point and this was faster for now...
//

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = ::js_sys::Object, js_name = Window)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(super) type WasmWindow;

    #[wasm_bindgen(method, getter, js_class = "Window")]
    pub(super) fn navigator(this: &WasmWindow) -> WasmNavigator;
}

impl From<web_sys::Window> for WasmWindow {
    fn from(window: web_sys::Window) -> Self {
        let value: &JsValue = window.as_ref();
        Self::from(value.clone())
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = ::js_sys::Object, js_name = Navigator)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(super) type WasmNavigator;

    #[wasm_bindgen(method, getter, js_class = "Navigator")]
    pub(super) fn credentials(this: &WasmNavigator) -> WasmCredentialsContainer;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = ::js_sys::Object, js_name = CredentialsContainer)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(super) type WasmCredentialsContainer;

    #[wasm_bindgen(
        catch,
        method,
        js_class = "CredentialsContainer",
        js_name = get,
    )]
    fn get_with_options(
        this: &WasmCredentialsContainer,
        options: &JsValue,
    ) -> Result<::js_sys::Promise, JsValue>;

    #[wasm_bindgen(
        catch,
        method,
        js_class = "CredentialsContainer",
        js_name = create,
    )]
    pub(super) fn create(
        this: &WasmCredentialsContainer,
        options: &JsValue,
    ) -> Result<::js_sys::Promise, JsValue>;
}

//
// UI Code
//

#[derive(Clone, Default, PartialEq, Properties)]
#[builder]
pub struct WebAuthn {
    /// Is the panel visible or not?
    ///
    /// We abort the webauthn challenge if the panel is hidden.
    #[prop_or_default]
    #[builder]
    pub visible: bool,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_webauthn: Option<Callback<String>>,

    #[prop_or_default]
    #[builder]
    pub challenge: JsValue,

    #[prop_or_default]
    #[builder]
    pub challenge_string: String,
}

impl WebAuthn {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub enum Msg {
    Start,
    Respond(String),
    Error(Error),
}

pub struct ProxmoxWebAuthn {
    running: bool, // fixme: replace with webnauthn promise
    error: Option<String>,
}

impl ProxmoxWebAuthn {
    fn handle_hw_rsp(hw_rsp: JsValue, challenge_string: String) -> Result<String, Error> {
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
        let authenticator_data = get_b64u(&response, "authenticatorData")?;
        let client_data_json = get_b64u(&response, "clientDataJSON")?;
        let signature = get_b64u(&response, "signature")?;

        serde_json::to_string(&serde_json::json!({
            "id": id,
            "type": ty,
            "challenge": challenge_string,
            "rawId": raw_id,
            "response": {
                "authenticatorData": authenticator_data,
                "clientDataJSON": client_data_json,
                "signature": signature,
            },
        }))
        .context("failed to build response json object")
    }
}

impl Component for ProxmoxWebAuthn {
    type Message = Msg;
    type Properties = WebAuthn;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            running: false,
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Error(err) => {
                self.error = Some(format!("{err:?}"));
                true
            }
            Msg::Respond(response) => match &ctx.props().on_webauthn {
                Some(cb) => {
                    self.error = None;
                    cb.emit(response);
                    true
                }
                None => {
                    self.error = Some("no response callback available".to_string());
                    true
                }
            },
            Msg::Start => {
                self.running = true;
                match WasmWindow::from(web_sys::window().unwrap())
                    .navigator()
                    .credentials()
                    .get_with_options(&ctx.props().challenge)
                    .map_err(convert_js_error)
                {
                    Err(err) => self.error = Some(format!("{err:?}")),
                    Ok(promise) => {
                        let challenge_string = ctx.props().challenge_string.clone();
                        ctx.link().send_future(async move {
                            match wasm_bindgen_futures::JsFuture::from(promise)
                                .await
                                .map_err(convert_js_error)
                                .and_then(|rsp| Self::handle_hw_rsp(rsp, challenge_string))
                            {
                                Ok(rsp) => Msg::Respond(rsp),
                                Err(err) => Msg::Error(err),
                            }
                        });
                    }
                }
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        let props = ctx.props();

        if props.visible == false {
            log::info!("Abort running Webauthn challenge.");
            self.running = false;
        }

        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let text = self
            .error
            .as_deref()
            .unwrap_or("Click the button to start the authentication");

        let panel = Column::new()
            .padding(2)
            .gap(2)
            .with_child(html! {<div>{text}</div>})
            .with_flex_spacer()
            .with_child(
                Button::new("Start WebAuthn challenge")
                    .class("pwt-align-self-flex-end")
                    .class("pwt-scheme-primary")
                    .disabled(self.running)
                    .onclick(ctx.link().callback(|_| Msg::Start)),
            );

        Mask::new(panel)
            .class("pwt-flex-fill")
            .text("Please insert your authentication device and press its button")
            .visible(self.running)
            .into()
    }
}

impl Into<VNode> for WebAuthn {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxWebAuthn>(Rc::new(self), None);
        VNode::from(comp)
    }
}
