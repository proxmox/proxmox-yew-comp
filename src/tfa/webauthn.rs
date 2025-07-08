use std::rc::Rc;

use anyhow::{Context as _, Error};
use js_sys::Reflect;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;
use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::widget::Mask;
use pwt::{convert_js_error, AsyncPool};
use pwt::{
    prelude::*,
    widget::{Button, Column},
};
use pwt_macros::builder;

use pwt::WebSysAbortGuard;

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
    running: Option<WebSysAbortGuard>,
    error: Option<String>,
    async_pool: AsyncPool,
}

impl ProxmoxWebAuthn {
    fn handle_hw_rsp(hw_rsp: JsValue, challenge_string: String) -> Result<String, Error> {
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
            Ok(proxmox_base64::url::encode_no_pad(&bytes))
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

    fn start(&mut self, ctx: &Context<Self>) -> Result<(), Error> {
        let running = WebSysAbortGuard::new()?;

        let challenge = &ctx.props().challenge;
        Reflect::set(challenge, &"signal".into(), &running.signal())
            .ok()
            .context("failed to set 'signal' property in challenge")?;

        let promise = WasmWindow::from(gloo_utils::window())
            .navigator()
            .credentials()
            .get_with_options(challenge)
            .map_err(convert_js_error)
            .context("failed to start webauthn authentication")?;

        let challenge_string = ctx.props().challenge_string.clone();
        let link = ctx.link().clone();

        self.async_pool.spawn(async move {
            match wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map_err(convert_js_error)
                .and_then(|rsp| Self::handle_hw_rsp(rsp, challenge_string))
            {
                Ok(rsp) => link.send_message(Msg::Respond(rsp)),
                Err(err) => link.send_message(Msg::Error(err)),
            }
        });

        self.running = Some(running);

        Ok(())
    }
}

impl Component for ProxmoxWebAuthn {
    type Message = Msg;
    type Properties = WebAuthn;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            running: None,
            error: None,
            async_pool: AsyncPool::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Error(err) => {
                self.running = None;
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
                if let Err(err) = self.start(ctx) {
                    self.error = Some(format!("{err:?}"));
                }
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        let props = ctx.props();

        if !props.visible {
            log::info!("Abort running Webauthn challenge.");
            self.running = None;
        }

        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let text = tr!("Click the button to start the authentication");
        let text = match self.error.as_deref() {
            Some(err) => html! { <div>{text}<br/><i class="fa fa-warning"/> {err}</div> },
            None => html! { <div>{text}</div> },
        };

        let panel = Column::new()
            .padding(2)
            .gap(2)
            .with_child(text)
            .with_flex_spacer()
            .with_child(
                Button::new("Start WebAuthn challenge")
                    .class("pwt-align-self-flex-end")
                    .class("pwt-scheme-primary")
                    .disabled(self.running.is_some())
                    .onclick(ctx.link().callback(|_| Msg::Start)),
            );

        Mask::new(panel)
            .class("pwt-flex-fill")
            .text("Please insert your authentication device and press its button")
            .visible(self.running.is_some())
            .into()
    }
}

impl From<WebAuthn> for VNode {
    fn from(val: WebAuthn) -> Self {
        let comp = VComp::new::<ProxmoxWebAuthn>(Rc::new(val), None);
        VNode::from(comp)
    }
}
