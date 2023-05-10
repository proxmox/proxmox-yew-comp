use std::rc::Rc;

use yew::prelude::*;

use pwt::prelude::*;
use pwt::widget::form::{Field, Form, FormContext, ResetButton, SubmitButton};
use pwt::widget::{Column, InputPanel, Mask, Row};

use proxmox_login::{Authentication, SecondFactorChallenge, TicketResult};

use crate::{RealmSelector, TfaDialog};

#[derive(Clone, PartialEq, Properties)]
pub struct LoginPanelProps {
    pub onlogin: Callback<Authentication>,
}

pub enum Msg {
    FormDataChange,
    Submit,
    Login,
    LoginError(String),
    Challenge(SecondFactorChallenge),
    AbortTfa,
    Totp(String),
    Yubico(String),
    RecoveryKey(String),
    WebAuthn(String),
}

pub struct LoginPanel {
    loading: bool,
    login_error: Option<String>,
    form_ctx: FormContext,
    challenge: Option<Rc<SecondFactorChallenge>>,
}

impl LoginPanel {
    fn send_login(
        ctx: &Context<Self>,
        username: String,
        password: String,
        realm: String,
    ) {
        let link = ctx.link().clone();
        let onlogin = ctx.props().onlogin.clone();

        wasm_bindgen_futures::spawn_local(async move {
            match crate::http_login(username, password, realm).await {
                Ok(TicketResult::Full(info)) => {
                    onlogin.emit(info);
                    link.send_message(Msg::Login);
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
        ctx: &Context<Self>,
        challenge: Rc<proxmox_login::SecondFactorChallenge>,
        response: proxmox_login::Request,
    ) {
        let link = ctx.link().clone();
        let onlogin = ctx.props().onlogin.clone();

        wasm_bindgen_futures::spawn_local(async move {
            match crate::http_login_tfa(challenge, response).await {
                Ok(info) => {
                    onlogin.emit(info);
                    link.send_message(Msg::Login);
                }
                Err(err) => {
                    link.send_message(Msg::LoginError(err.to_string()));
                }
            }
        });
    }
}

impl Component for LoginPanel {
    type Message = Msg;
    type Properties = LoginPanelProps;

    fn create(ctx: &Context<Self>) -> Self {
        let form_ctx = FormContext::new().on_change(ctx.link().callback(|_| Msg::FormDataChange));
        Self {
            form_ctx,
            loading: false,
            login_error: None,
            challenge: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
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

                Self::send_tfa_response(ctx, challenge, response);
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

                Self::send_tfa_response(ctx, challenge, response);
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

                Self::send_tfa_response(ctx, challenge, response);
                true
            }

            Msg::WebAuthn(_data) => {
                let _challenge = match self.challenge.take() {
                    Some(challenge) => challenge,
                    None => return true, // should never happen
                };

                /* diabled for now (requires feature webauthn)
                let response = match challenge.respond_webauthn(&data) {
                    Ok(response) => response,
                    Err(err) => {
                        ctx.link().send_message(Msg::LoginError(err.to_string()));
                        return true;
                    }
                };

                Self::send_tfa_response(ctx, challenge, response);
                */
                true
            }
            Msg::Submit => {
                self.loading = true;

                let username = self.form_ctx.read().get_field_text("username");
                let password = self.form_ctx.read().get_field_text("password");
                let realm = self.form_ctx.read().get_field_text("realm");

                Self::send_login(ctx, username, password, realm);
                true
            }
            Msg::Login => {
                self.loading = false;
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

        let input_panel = InputPanel::new()
            .class("pwt-p-2")
            .with_field(
                "User name",
                Field::new()
                    .name("username")
                    .default("root")
                    .required(true)
                    .autofocus(true),
            )
            .with_field(
                "Password",
                Field::new()
                    .name("password")
                    .required(true)
                    .input_type("password"),
            )
            .with_field("Realm", RealmSelector::new().name("realm"));

        let tfa_dialog = match &self.challenge {
            Some(challenge) => Some(
                TfaDialog::new(challenge.clone())
                    .on_close(ctx.link().callback(|_| Msg::AbortTfa))
                    .on_totp(ctx.link().callback(Msg::Totp))
                    .on_yubico(ctx.link().callback(Msg::Yubico))
                    .on_recovery(ctx.link().callback(Msg::RecoveryKey))
                    .on_webauthn(ctx.link().callback(Msg::WebAuthn))
            ),
            None => None,
        };

        let toolbar = Row::new()
            .padding(2)
            .gap(2)
            .class("pwt-border-top pwt-bg-color-neutral-emphased")
            .with_flex_spacer()
            .with_child(ResetButton::new())
            .with_child(
                SubmitButton::new()
                    .text("Login")
                    .on_submit(link.callback(move |_| Msg::Submit)),
            );

        let form_panel = Column::new()
            .with_child(input_panel)
            .with_optional_child(tfa_dialog)
            .with_optional_child(
                self.login_error
                    .as_ref()
                    .map(|msg| pwt::widget::error_message(msg, "pwt-p-2")),
            )
            .with_child(toolbar);

        Mask::new()
            .visible(self.loading)
            .with_child(
                Form::new()
                    .form_context(self.form_ctx.clone())
                    .with_child(form_panel),
            )
            .into()
    }
}
