use std::rc::Rc;

use pwt::css::Overflow;
use pwt::state::PersistentState;
use yew::html::IntoEventCallback;
use yew::prelude::*;
use yew::virtual_dom::{VComp, VNode};

use pwt::widget::form::{Checkbox, Field, Form, FormContext, InputType, ResetButton, SubmitButton};
use pwt::widget::{Column, InputPanel, LanguageSelector, Mask, Row};
use pwt::{prelude::*, AsyncPool};

use proxmox_login::{Authentication, SecondFactorChallenge, TicketResult};

use crate::{tfa::TfaDialog, RealmSelector};

#[derive(Clone, PartialEq, Properties)]
pub struct LoginPanel {
    #[prop_or_default]
    pub on_login: Option<Callback<Authentication>>,

    #[prop_or(AttrValue::from("pam"))]
    pub default_realm: AttrValue,
}

impl Default for LoginPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginPanel {
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    pub fn default_realm(mut self, realm: impl Into<AttrValue>) -> Self {
        self.default_realm = realm.into();
        self
    }

    pub fn on_login(mut self, cb: impl IntoEventCallback<Authentication>) -> Self {
        self.on_login = cb.into_event_callback();
        self
    }
}

pub enum Msg {
    FormDataChange,
    Submit,
    SaveUsername(bool),
    Login(Authentication),
    LoginError(String),
    Challenge(SecondFactorChallenge),
    AbortTfa,
    Totp(String),
    Yubico(String),
    RecoveryKey(String),
    WebAuthn(String),
}

pub struct ProxmoxLoginPanel {
    loading: bool,
    login_error: Option<String>,
    form_ctx: FormContext,
    challenge: Option<Rc<SecondFactorChallenge>>,
    save_username: PersistentState<bool>,
    last_username: PersistentState<String>,
    async_pool: AsyncPool,
}

impl ProxmoxLoginPanel {
    fn send_login(&self, ctx: &Context<Self>, username: String, password: String, realm: String) {
        let link = ctx.link().clone();
        self.async_pool.spawn(async move {
            match crate::http_login(username, password, realm).await {
                Ok(TicketResult::Full(info)) => {
                    link.send_message(Msg::Login(info));
                }
                Ok(TicketResult::TfaRequired(challenge)) => {
                    link.send_message(Msg::Challenge(challenge));
                }
                Err(err) => {
                    link.send_message(Msg::LoginError(err.to_string()));
                }
            }
        });
    }

    fn send_tfa_response(
        &self,
        ctx: &Context<Self>,
        challenge: Rc<proxmox_login::SecondFactorChallenge>,
        response: proxmox_login::Request,
    ) {
        let link = ctx.link().clone();
        self.async_pool.spawn(async move {
            match crate::http_login_tfa(challenge, response).await {
                Ok(info) => {
                    link.send_message(Msg::Login(info));
                }
                Err(err) => {
                    link.send_message(Msg::LoginError(err.to_string()));
                }
            }
        });
    }
}

impl Component for ProxmoxLoginPanel {
    type Message = Msg;
    type Properties = LoginPanel;

    fn create(ctx: &Context<Self>) -> Self {
        let form_ctx = FormContext::new().on_change(ctx.link().callback(|_| Msg::FormDataChange));

        let save_username = PersistentState::<bool>::new("ProxmoxLoginPanelSaveUsername");
        let last_username = PersistentState::<String>::new("ProxmoxLoginPanelUsername");

        Self {
            form_ctx,
            loading: false,
            login_error: None,
            challenge: None,
            save_username,
            last_username,
            async_pool: AsyncPool::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::FormDataChange => {
                self.login_error = None;
                true
            }
            Msg::Challenge(challenge) => {
                self.challenge = Some(Rc::new(challenge));
                true
            }
            Msg::AbortTfa => {
                self.loading = false;
                self.challenge = None;
                true
            }
            Msg::Totp(data) => {
                let challenge = match self.challenge.take() {
                    Some(challenge) => challenge,
                    None => return true, // should never happen
                };

                let response = match challenge.respond_totp(&data) {
                    Ok(response) => response,
                    Err(err) => {
                        ctx.link().send_message(Msg::LoginError(err.to_string()));
                        return true;
                    }
                };

                self.send_tfa_response(ctx, challenge, response);
                true
            }
            Msg::Yubico(data) => {
                let challenge = match self.challenge.take() {
                    Some(challenge) => challenge,
                    None => return true, // should never happen
                };

                let response = match challenge.respond_yubico(&data) {
                    Ok(response) => response,
                    Err(err) => {
                        ctx.link().send_message(Msg::LoginError(err.to_string()));
                        return true;
                    }
                };

                self.send_tfa_response(ctx, challenge, response);
                true
            }
            Msg::RecoveryKey(data) => {
                let challenge = match self.challenge.take() {
                    Some(challenge) => challenge,
                    None => return true, // should never happen
                };

                let response = match challenge.respond_recovery(&data) {
                    Ok(response) => response,
                    Err(err) => {
                        ctx.link().send_message(Msg::LoginError(err.to_string()));
                        return true;
                    }
                };

                self.send_tfa_response(ctx, challenge, response);
                true
            }

            Msg::WebAuthn(data) => {
                let challenge = match self.challenge.take() {
                    Some(challenge) => challenge,
                    None => return true, // should never happen
                };

                // FIXME: once proxmox-login/tfa build with webauthn support, use
                // `respond_webauthn`.
                let response = challenge.respond_raw(&format!("webauthn:{data}"));
                /*
                let response = match challenge.respond_webauthn(&data) {
                    Ok(response) => response,
                    Err(err) => {
                        ctx.link().send_message(Msg::LoginError(err.to_string()));
                        return true;
                    }
                };
                */

                self.send_tfa_response(ctx, challenge, response);
                true
            }
            Msg::SaveUsername(save_username) => {
                self.save_username.update(save_username);
                true
            }
            Msg::Submit => {
                self.loading = true;

                let username = self.form_ctx.read().get_field_text("username");
                let password = self.form_ctx.read().get_field_text("password");
                let realm = self.form_ctx.read().get_field_text("realm");

                self.send_login(ctx, username, password, realm);
                true
            }
            Msg::Login(info) => {
                self.loading = false;
                if *self.save_username {
                    self.last_username.update(info.userid.clone());
                }
                if let Some(on_login) = &props.on_login {
                    on_login.emit(info);
                }
                true
            }
            Msg::LoginError(msg) => {
                self.loading = false;
                self.challenge = None;
                self.login_error = Some(msg);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link().clone();

        let mut default_username = String::from("root");
        let mut default_realm = ctx.props().default_realm.to_owned();

        if *self.save_username {
            let last_userid: String = (*self.last_username).to_string();
            if !last_userid.is_empty() {
                if let Some((user, realm)) = last_userid.rsplit_once('@') {
                    default_username = user.to_owned();
                    default_realm = realm.to_owned().into();
                }
            }
        }

        let input_panel = InputPanel::new()
            .class(Overflow::Auto)
            .padding(4)
            .with_field(
                "User name",
                Field::new()
                    .name("username")
                    .default(default_username)
                    .required(true)
                    .autofocus(true),
            )
            .with_field(
                "Password",
                Field::new()
                    .name("password")
                    .required(true)
                    .input_type(InputType::Password),
            )
            .with_field(
                "Realm",
                RealmSelector::new().name("realm").default(default_realm),
            );

        let tfa_dialog = match &self.challenge {
            Some(challenge) => Some(
                TfaDialog::new(challenge.clone())
                    .on_close(ctx.link().callback(|_| Msg::AbortTfa))
                    .on_totp(ctx.link().callback(Msg::Totp))
                    .on_yubico(ctx.link().callback(Msg::Yubico))
                    .on_recovery(ctx.link().callback(Msg::RecoveryKey))
                    .on_webauthn(ctx.link().callback(Msg::WebAuthn)),
            ),
            None => None,
        };

        let save_username_label_id = pwt::widget::get_unique_element_id();
        let save_username_field = Checkbox::new()
            .margin_start(1)
            .label_id(save_username_label_id.clone())
            .checked(*self.save_username)
            .on_change(ctx.link().callback(Msg::SaveUsername));

        let save_username = Row::new()
            .class("pwt-align-items-center")
            .with_child(html! {<label id={save_username_label_id} style="user-select:none;">{tr!("Save User name")}</label>})
            .with_child(save_username_field);

        let toolbar = Row::new()
            .padding(2)
            .gap(2)
            .class("pwt-bg-color-surface")
            .class("pwt-align-items-baseline")
            .with_child(LanguageSelector::new())
            .with_flex_spacer()
            .with_child(save_username)
            .with_child(ResetButton::new())
            .with_child(
                SubmitButton::new()
                    .class("pwt-scheme-primary")
                    .text("Login")
                    .on_submit(link.callback(move |_| Msg::Submit)),
            );

        let form_panel = Column::new()
            .class("pwt-flex-fill pwt-overflow-auto")
            .with_child(input_panel)
            .with_optional_child(tfa_dialog)
            .with_optional_child(
                self.login_error
                    .as_ref()
                    .map(|msg| pwt::widget::error_message(msg).padding(2)),
            )
            .with_flex_spacer()
            .with_child(toolbar);

        let form = Form::new()
            .class("pwt-overflow-auto")
            .form_context(self.form_ctx.clone())
            .with_child(form_panel);

        Mask::new(form)
            .class("pwt-flex-fill")
            .visible(self.loading)
            .into()
    }
}

impl From<LoginPanel> for VNode {
    fn from(val: LoginPanel) -> Self {
        let comp = VComp::new::<ProxmoxLoginPanel>(Rc::new(val), None);
        VNode::from(comp)
    }
}
