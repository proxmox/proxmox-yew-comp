use yew::prelude::*;

use proxmox_schema::ApiType;
use pbs_api_types::{Username, PASSWORD_SCHEMA};

use pwt::prelude::*;
use crate::LoginInfo;
use pwt::widget::{Column, InputPanel, Mask, Row};
use pwt::widget::form2::{Field, Form, FormContext, SubmitButton, ResetButton};

use crate::RealmSelector;

#[derive(Clone, PartialEq, Properties)]
pub struct LoginPanelProps {
   pub onlogin: Callback<LoginInfo>,
}

pub enum Msg {
    FormDataChange,
    Submit,
    Login,
    LoginError(String),
}

pub struct LoginPanel {
    loading: bool,
    login_error: Option<String>,
    form_ctx: FormContext,
}

impl Component for LoginPanel {
    type Message = Msg;
    type Properties = LoginPanelProps;

    fn create(ctx: &Context<Self>) -> Self {
        let form_ctx = FormContext::new()
            .on_change(ctx.link().callback(|_| Msg::FormDataChange));
        Self {
            form_ctx,
            loading: false,
            login_error: None,
        }
    }


    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FormDataChange => {
                self.login_error = None;
                true
            }
            Msg::Submit => {
                self.loading = true;

                //let data = self.form_state.get_submit_data();
                //log::info!("Submit Data {:?}", data);

                let props = ctx.props().clone();
                let link = ctx.link().clone();

                let username = self.form_ctx.read().get_field_text("username");
                let password = self.form_ctx.read().get_field_text("password");
                let realm = self.form_ctx.read().get_field_text("realm");

                //log::info!("Submit {} {}", username, realm);
                wasm_bindgen_futures::spawn_local(async move {
                    match crate::http_login(username, password, realm).await {
                        Ok(info) => {
                            props.onlogin.emit(info);
                            link.send_message(Msg::Login);
                        }
                        Err(err) => {
                            log::error!("ERROR: {:?}", err);
                            link.send_message(Msg::LoginError(err.to_string()));
                        }
                    }

                 });

                true
            }
            Msg::Login => {
                self.loading = false;
                true
            }
            Msg::LoginError(msg) => {
                self.loading = false;
                self.login_error = Some(msg);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link().clone();

        let validate_username = |value: &String| {
            Username::API_SCHEMA.parse_simple_value(value)?;
            Ok(())
        };

        let validate_password = |value: &String| {
            PASSWORD_SCHEMA.parse_simple_value(value)?;
            Ok(())
        };

        let input_panel = InputPanel::new()
            .class("pwt-p-2")
            .with_field(
                "User name",
                Field::new()
                    .name("username")
                    .validate(validate_username)
                    .default("root")
                    .autofocus(true)
            )
            .with_field(
                "Password",
                Field::new()
                    .name("password")
                    .validate(validate_password)
                    .input_type("password")
            )
            .with_field(
                "Realm",
                RealmSelector::new().name("realm"),
            );

        let toolbar = Row::new()
            .padding(2)
            .gap(2)
            .class("pwt-border-top pwt-bg-color-neutral-emphased")
            .with_flex_spacer()
            .with_child(ResetButton::new())
            .with_child(
                SubmitButton::new()
                    .text("Login")
                    .on_submit(link.callback(move |_| Msg::Submit))
            );

        let form_panel = Column::new()
            .with_child(input_panel)
            .with_optional_child(self.login_error.as_ref().map(|msg| {
                pwt::widget::error_message(msg, "pwt-p-2")
            }))
            .with_child(toolbar);

        Mask::new()
            .visible(self.loading)
            .with_child(
                Form::new()
                    .form_context(self.form_ctx.clone())
                    .with_child(form_panel)
            )
            .into()
    }
}
