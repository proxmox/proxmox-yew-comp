use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use pwt::css::ColorScheme;
use pwt::props::PwtSpace;
use pwt::state::PersistentState;
use pwt::touch::{SnackBar, SnackBarContextExt};
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::prelude::*;
use yew::virtual_dom::{VComp, VNode};

use pwt::widget::form::{Checkbox, Field, Form, FormContext, InputType, ResetButton, SubmitButton};
use pwt::widget::{Column, FieldLabel, InputPanel, LanguageSelector, Mask, Row};
use pwt::{prelude::*, AsyncPool};

use proxmox_login::api::CreateTicketResponse;
use proxmox_login::{Authentication, SecondFactorChallenge, Ticket, TicketResult};

use crate::common_api_types::BasicRealmInfo;
use crate::utils;
use crate::{tfa::TfaDialog, RealmSelector};

use pwt_macros::builder;

static OPENID_LOGIN: OnceLock<()> = OnceLock::new();

/// Proxmox login panel
///
/// Should support all proxmox product and TFA.
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct LoginPanel {
    /// Login callback (called after successful login)
    #[prop_or_default]
    #[builder_cb(IntoEventCallback, into_event_callback, Authentication)]
    pub on_login: Option<Callback<Authentication>>,

    /// Default realm.
    #[prop_or_default]
    #[builder]
    pub default_realm: Option<AttrValue>,

    /// Determines if the realm box is shown/used
    #[prop_or(true)]
    #[builder]
    pub realm_selectable: bool,

    /// Mobile Layout
    ///
    /// Use special layout for mobile apps. For example shows error in a [SnackBar]
    /// if a [SnackBarController](pwt::touch::SnackBarController) context is available.
    ///
    /// Note: Always use saved userid to avoid additional checkbox.
    #[prop_or(false)]
    #[builder]
    pub mobile: bool,

    /// The path to the domain api call
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or("/access/domains".into())]
    pub domain_path: AttrValue,
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
    UpdateRealm(BasicRealmInfo),
    OpenIDLogin,
    OpenIDAuthorization(HashMap<String, String>),
}

pub struct ProxmoxLoginPanel {
    loading: bool,
    login_error: Option<String>,
    form_ctx: FormContext,
    challenge: Option<Rc<SecondFactorChallenge>>,
    save_username: PersistentState<bool>,
    last_username: PersistentState<String>,
    async_pool: AsyncPool,
    selected_realm: Option<BasicRealmInfo>,
}

impl ProxmoxLoginPanel {
    fn send_login(&self, ctx: &Context<Self>, username: String, password: String, realm: String) {
        let link = ctx.link().clone();
        self.async_pool.spawn(async move {
            match crate::http_login(username, password, realm).await {
                // TODO: eventually deprecate support for `TicketResult::Full` and
                // throw an error. this package should only ever be used in a browser
                // context where authentication info should be set via HttpOnly cookies.
                Ok(TicketResult::Full(info)) | Ok(TicketResult::HttpOnly(info)) => {
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

    fn openid_redirect(&self, ctx: &Context<Self>) {
        let link = ctx.link().clone();
        let Some(realm) = self.selected_realm.as_ref() else {
            return;
        };
        let Ok(location) = gloo_utils::window().location().origin() else {
            return;
        };

        let data = serde_json::json!({
            "realm": realm.realm,
            "redirect-url": location,
        });

        self.async_pool.spawn(async move {
            match crate::http_post::<String>("/access/openid/auth-url", Some(data)).await {
                Ok(data) => {
                    let _ = gloo_utils::window().location().assign(&data);
                }
                Err(err) => {
                    link.send_message(Msg::LoginError(err.to_string()));
                }
            }
        });
    }

    fn openid_login(&self, ctx: &Context<Self>, mut auth: HashMap<String, String>) {
        let link = ctx.link().clone();
        let save_username = ctx.props().mobile || *self.save_username;
        let Ok(origin) = gloo_utils::window().location().origin() else {
            return;
        };

        auth.insert("redirect-url".into(), origin.clone());

        let Ok(auth) = serde_json::to_value(auth) else {
            return;
        };

        // run this only once, an openid state is only valid for one round trip. so resending it
        // here will just fail. also use an unabortable future here for the same reason. otherwise
        // we could be interrupted by, for example, the catalog loader needing to re-render the
        // app.
        OPENID_LOGIN.get_or_init(|| {
            wasm_bindgen_futures::spawn_local(async move {
                match crate::http_post::<CreateTicketResponse>("/access/openid/login", Some(auth))
                    .await
                {
                    Ok(creds) => {
                        let Some(ticket) = creds
                            .ticket
                            .or(creds.ticket_info)
                            .and_then(|t| t.parse::<Ticket>().ok())
                        else {
                            log::error!("neither ticket nor ticket-info in openid login response!");
                            return;
                        };

                        let Some(csrfprevention_token) = creds.csrfprevention_token else {
                            log::error!("no CSRF prevention token in the openid login response!");
                            return;
                        };

                        let auth = Authentication {
                            api_url: "".to_string(),
                            userid: creds.username,
                            ticket,
                            clustername: None,
                            csrfprevention_token,
                        };

                        // update the authentication, set the realm and user for the next login and
                        // reload without the query parameters.
                        crate::http_set_auth(auth.clone());
                        if save_username {
                            PersistentState::<String>::new("ProxmoxLoginPanelUsername")
                                .update(auth.userid.clone());
                        }
                        let _ = gloo_utils::window().location().assign(&origin);
                    }
                    Err(err) => link.send_message(Msg::LoginError(err.to_string())),
                }
            });
        });
    }

    fn get_defaults(&self, props: &LoginPanel) -> (String, Option<AttrValue>) {
        let mut default_username = String::from("root");
        let mut default_realm = props.default_realm.clone();

        if props.mobile || *self.save_username {
            let last_userid: String = (*self.last_username).to_string();
            if !last_userid.is_empty() {
                if let Some((user, realm)) = last_userid.rsplit_once('@') {
                    default_username = user.to_owned();
                    default_realm = Some(AttrValue::from(realm.to_owned()));
                }
            }
        }
        (default_username, default_realm)
    }

    fn mobile_view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let link = ctx.link().clone();

        let (default_username, default_realm) = self.get_defaults(props);

        let username_label_id = pwt::widget::get_unique_element_id();
        let password_label_id = pwt::widget::get_unique_element_id();
        let realm_label_id = pwt::widget::get_unique_element_id();

        let tfa_dialog = self.challenge.as_ref().map(|challenge| {
            TfaDialog::new(challenge.clone())
                .mobile(true)
                .on_close(ctx.link().callback(|_| Msg::AbortTfa))
                .on_totp(ctx.link().callback(Msg::Totp))
                .on_yubico(ctx.link().callback(Msg::Yubico))
                .on_recovery(ctx.link().callback(Msg::RecoveryKey))
                .on_webauthn(ctx.link().callback(Msg::WebAuthn))
        });

        let mut form_panel = Column::new()
            .class(pwt::css::FlexFit)
            .padding(2)
            .with_flex_spacer();

        if self
            .selected_realm
            .as_ref()
            .map(|r| r.ty != "openid")
            .unwrap_or(true)
        {
            form_panel = form_panel
                .with_child(
                    FieldLabel::new(tr!("User name"))
                        .id(username_label_id.clone())
                        .padding_bottom(PwtSpace::Em(0.25)),
                )
                .with_child(
                    Field::new()
                        .name("username")
                        .label_id(username_label_id)
                        .default(default_username)
                        .required(true)
                        .validate({
                            let realm_selectable = props.realm_selectable;
                            move |value: &String| {
                                if realm_selectable {
                                    return Ok(());
                                } else if let Some((user, realm)) = value.rsplit_once('@') {
                                    if !user.is_empty() && !realm.is_empty() {
                                        return Ok(());
                                    }
                                }
                                anyhow::bail!("{}", tr!("invalid username"));
                            }
                        })
                        .autofocus(true),
                )
                .with_child(
                    FieldLabel::new(tr!("Password"))
                        .id(password_label_id.clone())
                        .padding_top(1)
                        .padding_bottom(PwtSpace::Em(0.25)),
                )
                .with_child(
                    Field::new()
                        .name("password")
                        .label_id(password_label_id)
                        .input_type(InputType::Password),
                );
        }

        let submit_button = SubmitButton::new().class(ColorScheme::Primary).margin_y(4);

        let submit_button = if self
            .selected_realm
            .as_ref()
            .map(|r| r.ty == "openid")
            .unwrap_or_default()
        {
            submit_button
                .text(tr!("Login (OpenID redirect)"))
                .check_dirty(false)
                .on_submit(link.callback(move |_| Msg::OpenIDLogin))
        } else {
            submit_button
                .text(tr!("Login"))
                .on_submit(link.callback(move |_| Msg::Submit))
        };

        let form_panel = form_panel
            .with_optional_child(props.realm_selectable.then_some(
                FieldLabel::new(tr!("Realm"))
                    .id(realm_label_id.clone())
                    .padding_top(1)
                    .padding_bottom(PwtSpace::Em(0.25)),
            ))
            .with_optional_child(props.realm_selectable.then_some(
                RealmSelector::new()
                    .name("realm")
                    .label_id(realm_label_id)
                    .path(props.domain_path.clone())
                    .on_change({
                        let link = link.clone();
                        move |r: BasicRealmInfo| link.send_message(Msg::UpdateRealm(r))
                    })
                    .default(default_realm),
            ))
            .with_child(submit_button)
            .with_optional_child(self.login_error.as_ref().map(|msg| {
                let icon_class = classes!("fa-lg", "fa", "fa-align-center", "fa-exclamation-triangle");
                let text = tr!("Login failed. Please try again ({0})", msg);
                Row::new()
                    .class("pwt-align-items-center")
                    .with_child(
                        html! {<span class={"pwt-message-sign"} role="none"><i class={icon_class}/></span>},
                    )
                    .with_child(html! {<p style={"overflow-wrap: anywhere;"}>{text}</p>})
                    .padding_bottom(2)
            }))
            .with_flex_spacer()
            .with_child(Row::new().with_child(LanguageSelector::new()))
            .with_optional_child(tfa_dialog);

        let form = Form::new()
            .width(500)
            .class(pwt::css::FlexFit)
            .form_context(self.form_ctx.clone())
            .with_child(form_panel);

        Mask::new(form)
            .class(pwt::css::FlexFit)
            .visible(self.loading)
            .into()
    }

    fn standard_view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let link = ctx.link().clone();

        let (default_username, default_realm) = self.get_defaults(props);

        let mut input_panel = InputPanel::new()
            .class(pwt::css::Overflow::Auto)
            .width("initial") // don't try to minimize size
            .padding(4);

        if self
            .selected_realm
            .as_ref()
            .map(|r| r.ty != "openid")
            .unwrap_or(true)
        {
            input_panel = input_panel
                .with_field(
                    tr!("User name"),
                    Field::new()
                        .name("username")
                        .default(default_username)
                        .required(true)
                        .autofocus(true),
                )
                .with_field(
                    tr!("Password"),
                    Field::new()
                        .name("password")
                        .required(true)
                        .input_type(InputType::Password),
                );
        }

        if props.realm_selectable {
            input_panel = input_panel.with_field(
                tr!("Realm"),
                RealmSelector::new()
                    .name("realm")
                    .path(props.domain_path.clone())
                    .on_change({
                        let link = link.clone();
                        move |r: BasicRealmInfo| link.send_message(Msg::UpdateRealm(r))
                    })
                    .default(default_realm),
            );
        }

        let tfa_dialog = self.challenge.as_ref().map(|challenge| {
            TfaDialog::new(challenge.clone())
                .on_close(ctx.link().callback(|_| Msg::AbortTfa))
                .on_totp(ctx.link().callback(Msg::Totp))
                .on_yubico(ctx.link().callback(Msg::Yubico))
                .on_recovery(ctx.link().callback(Msg::RecoveryKey))
                .on_webauthn(ctx.link().callback(Msg::WebAuthn))
        });

        let save_username_label_id = pwt::widget::get_unique_element_id();
        let save_username_field = Checkbox::new()
            .margin_start(1)
            .label_id(save_username_label_id.clone())
            .checked(*self.save_username)
            .on_change(ctx.link().callback(Msg::SaveUsername));

        let save_username = Row::new()
            .class(pwt::css::AlignItems::Center)
            .with_child(html! {<label id={save_username_label_id} style="user-select:none;">{tr!("Save User name")}</label>})
            .with_child(save_username_field);

        let submit_button = SubmitButton::new().class(ColorScheme::Primary);

        let submit_button = if self
            .selected_realm
            .as_ref()
            .map(|r| r.ty == "openid")
            .unwrap_or_default()
        {
            submit_button
                .text(tr!("Login (OpenID redirect)"))
                .check_dirty(false)
                .on_submit(link.callback(move |_| Msg::OpenIDLogin))
        } else {
            submit_button
                .text(tr!("Login"))
                .on_submit(link.callback(move |_| Msg::Submit))
        };

        let toolbar = Row::new()
            .padding(2)
            .gap(2)
            .class("pwt-bg-color-surface")
            .class(pwt::css::AlignItems::Baseline)
            .with_child(LanguageSelector::new())
            .with_flex_spacer()
            .with_child(save_username)
            .with_child(ResetButton::new())
            .with_child(submit_button);

        let form_panel = Column::new()
            .class(pwt::css::FlexFit)
            .with_child(input_panel)
            .with_optional_child(tfa_dialog)
            .with_optional_child(self.login_error.as_ref().map(|msg| {
                pwt::widget::error_message(&tr!("Login failed. Please try again ({0})", msg))
                    .padding(2)
            }))
            .with_flex_spacer()
            .with_child(toolbar);

        let form = Form::new()
            .width(500)
            .class(pwt::css::Overflow::Auto)
            .form_context(self.form_ctx.clone())
            .with_child(form_panel);

        Mask::new(form)
            .class(pwt::css::Flex::Fill)
            .visible(self.loading)
            .into()
    }
}

impl Component for ProxmoxLoginPanel {
    type Message = Msg;
    type Properties = LoginPanel;

    fn create(ctx: &Context<Self>) -> Self {
        let form_ctx = FormContext::new().on_change(ctx.link().callback(|_| Msg::FormDataChange));

        let save_username = PersistentState::<bool>::new("ProxmoxLoginPanelSaveUsername");
        let last_username = PersistentState::<String>::new("ProxmoxLoginPanelUsername");

        if let Some(auth) = utils::openid_redirection_authorization() {
            ctx.link().send_message(Msg::OpenIDAuthorization(auth));
        }

        Self {
            form_ctx,
            loading: false,
            login_error: None,
            challenge: None,
            save_username,
            last_username,
            async_pool: AsyncPool::new(),
            selected_realm: None,
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

                let password = self.form_ctx.read().get_field_text("password");
                let (username, realm) = if props.realm_selectable {
                    let username = self.form_ctx.read().get_field_text("username");
                    let realm = self.form_ctx.read().get_field_text("realm");
                    (username, realm)
                } else {
                    self.form_ctx
                        .read()
                        .get_field_text("username")
                        .rsplit_once('@')
                        .map(|(user, realm)| (user.to_string(), realm.to_string()))
                        .unwrap_or_default()
                };

                self.send_login(ctx, username, password, realm);
                if let (true, Some(controller)) = (props.mobile, ctx.link().snackbar_controller()) {
                    controller.dismiss_all()
                }
                true
            }
            Msg::Login(info) => {
                self.loading = false;
                if props.mobile || *self.save_username {
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
                match (props.mobile, ctx.link().snackbar_controller()) {
                    (true, Some(controller)) => {
                        controller.show_snackbar(SnackBar::new().message(msg));
                    }
                    _ => {
                        self.login_error = Some(msg);
                    }
                }
                true
            }
            Msg::UpdateRealm(realm) => {
                self.selected_realm = Some(realm);
                true
            }
            Msg::OpenIDLogin => {
                self.loading = true;
                self.openid_redirect(ctx);
                false
            }
            Msg::OpenIDAuthorization(auth) => {
                self.loading = true;
                self.openid_login(ctx, auth);
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if ctx.props().mobile {
            self.mobile_view(ctx)
        } else {
            self.standard_view(ctx)
        }
    }
}

impl From<LoginPanel> for VNode {
    fn from(val: LoginPanel) -> Self {
        let comp = VComp::new::<ProxmoxLoginPanel>(Rc::new(val), None);
        VNode::from(comp)
    }
}
