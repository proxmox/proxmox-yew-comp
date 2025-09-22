use std::rc::Rc;

use anyhow::Error;
use proxmox_client::ApiResponseData;
use pwt::css::{Flex, Overflow};

use pwt::widget::form::{Checkbox, Combobox, FormContext, InputType, Number};
use serde_json::Value;
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{delete_empty_values, Field, TristateBoolean};
use pwt::widget::{Container, InputPanel, TabBarItem, TabPanel};

use crate::percent_encoding::percent_encode_component;

use pwt_macros::builder;

use crate::EditWindow;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct AuthEditLDAP {
    /// Close/Abort callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    #[prop_or("/access/domains".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,

    /// Edit existing realm
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub realm: Option<AttrValue>,

    /// Whether this panel is for an Active Directory realm
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub ad_realm: Option<bool>,
}

impl Default for AuthEditLDAP {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthEditLDAP {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

async fn load_realm(url: impl Into<String>) -> Result<ApiResponseData<Value>, Error> {
    let mut response: ApiResponseData<Value> = crate::http_get_full(url, None).await?;

    response.data["anonymous_search"] = Value::Bool(!response.data["bind-dn"].is_string());

    if let Value::String(sync_default_options) = response.data["sync-defaults-options"].take() {
        let split = sync_default_options.split(",");

        for part in split {
            let mut part = part.split("=");

            match part.next() {
                Some("enable-new") => {
                    response.data["enable-new"] = Value::Bool(part.next() == Some("true"))
                }
                Some("remove-vanished") => {
                    if let Some(part) = part.next() {
                        for vanished_opt in part.split(";") {
                            response.data[&format!("remove-vanished-{vanished_opt}")] =
                                Value::Bool(true)
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if let Value::String(sync_attributes) = response.data["sync-attributes"].take() {
        let split = sync_attributes.split(",");

        for opt in split {
            let mut opt = opt.split("=");
            if let (Some(name), Some(val)) = (opt.next(), opt.next()) {
                response.data[name] = Value::String(val.to_string());
            }
        }
    }

    Ok(response)
}

fn format_sync_and_default_options(data: &mut Value) -> Value {
    let mut sync_default_options: Option<String> = None;

    if let Value::Bool(val) = data["enable-new"].take() {
        sync_default_options = Some(format!("enable-new={val}"))
    }

    let mut remove_vanished: Vec<&str> = Vec::new();

    for prop in ["acl", "entry", "properties"] {
        let prop_name = format!("remove-vanished-{prop}");
        if data[&prop_name].take() == Value::Bool(true) {
            remove_vanished.push(prop);
        }
    }

    if !remove_vanished.is_empty() {
        let vanished = format!("remove-vanished={}", remove_vanished.join(";"));

        sync_default_options = sync_default_options
            .map(|f| format!("{f},{vanished}"))
            .or(Some(vanished));
    }

    if let Some(defaults) = sync_default_options {
        data["sync-defaults-options"] = Value::String(defaults);
    }

    let mut sync_attributes = Vec::new();

    for attribute in ["firstname", "lastname", "email"] {
        if let Value::String(val) = &data[attribute].take() {
            sync_attributes.push(format!("{attribute}={val}"));
        }
    }

    if !sync_attributes.is_empty() {
        data["sync-attributes"] = Value::String(sync_attributes.join(","));
    }

    let mut new = serde_json::json!({});

    for (param, v) in data.as_object().unwrap().iter() {
        if !v.is_null() {
            new[param] = v.clone();
        }
    }

    new
}

async fn create_item(form_ctx: FormContext, base_url: String) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();
    let data = format_sync_and_default_options(&mut data);
    crate::http_post(base_url, Some(data)).await
}

async fn update_item(form_ctx: FormContext, base_url: String) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();

    let data = format_sync_and_default_options(&mut data);

    let data = delete_empty_values(
        &data,
        &[
            "server2",
            "port",
            "mode",
            "verify",
            "comment",
            "user-classes",
            "filter",
            "sync-attributes",
            "sync-defaults-options",
        ],
        true,
    );

    let name = form_ctx.read().get_field_text("realm");

    let url = format!("{base_url}/{}", percent_encode_component(&name));

    crate::http_put(&url, Some(data)).await
}

#[doc(hidden)]
pub struct ProxmoxAuthEditLDAP {}

fn render_panel(form_ctx: FormContext, props: AuthEditLDAP) -> Html {
    TabPanel::new()
        .with_item(TabBarItem::new().key("general").label(tr!("General")), {
            let props = props.clone();
            let form_ctx = form_ctx.clone();
            render_general_form(form_ctx.clone(), props.clone())
        })
        .with_item(
            TabBarItem::new().key("sync").label(tr!("Sync Options")),
            render_sync_form(form_ctx.clone(), props.clone()),
        )
        .force_render_all(true)
        .into()
}

fn render_sync_form(_form_ctx: FormContext, _props: AuthEditLDAP) -> Html {
    //let is_edit = props.realm.is_some();

    InputPanel::new()
        .class(Flex::Fill)
        .class(Overflow::Auto)
        .padding(4)
        .with_field(tr!("First Name attribute"), Field::new().name("firstname"))
        .with_right_field(
            tr!("User classes"),
            Field::new()
                .name("user-classes")
                .placeholder("inetorgperson, posixaccount, person, user"),
        )
        .with_field(tr!("Last Name attribute"), Field::new().name("lastname"))
        .with_right_field(tr!("User Filter"), Field::new().name("filter"))
        .with_field(tr!("E-Mail attribute"), Field::new().name("email"))
        .with_large_custom_child(
            Container::new()
                .class("pwt-font-title-medium")
                .padding_top(2)
                .with_child(tr!("Default Sync Options")),
        )
        .with_field(
            tr!("Enable new users"),
            TristateBoolean::new()
                .name("enable-new")
                .submit_empty(true)
                .null_text(tr!("Default") + " (" + &tr!("Yes") + ")"),
        )
        .with_large_custom_child(
            Container::new()
                .class("pwt-font-title-medium")
                .padding_top(2)
                .with_child(tr!("Remove Vanished Options")),
        )
        .with_field(
            tr!("Remove ACLs of vanished users"),
            Checkbox::new().name("remove-vanished-acl"),
        )
        .with_field(
            tr!("Remove vanished user"),
            Checkbox::new().name("remove-vanished-entry"),
        )
        .with_field(
            tr!("Remove vanished properties"),
            Checkbox::new().name("remove-vanished-properties"),
        )
        .into()
}

fn render_general_form(form_ctx: FormContext, props: AuthEditLDAP) -> Html {
    let is_edit = props.realm.is_some();

    let mode_items = Rc::new(vec!["ldap".into(), "ldap+starttls".into(), "ldaps".into()]);

    let anonymous_search = form_ctx
        .read()
        .get_field_value("anonymous_search")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let tls_enabled = form_ctx
        .read()
        .get_field_value("mode")
        .map(|v| matches!(v.as_str(), Some("ldap+starttls") | Some("ldaps")))
        .unwrap_or(false);

    let mut input_panel = InputPanel::new()
        .class(Flex::Fill)
        .class(Overflow::Auto)
        .padding(4)
        .with_field(
            tr!("Realm"),
            Field::new()
                .name("realm")
                .required(true)
                .disabled(is_edit)
                .submit(!is_edit),
        )
        .with_right_field(tr!("Server"), Field::new().name("server1").required(true))
        .with_field(tr!("Default Realm"), Checkbox::new().name("default"));

    if !props.ad_realm.unwrap_or_default() {
        input_panel = input_panel
            .with_field(
                tr!("Base Domain Name"),
                Field::new()
                    .name("base-dn")
                    .required(true)
                    .placeholder("cn=Users,dc=company,dc=net"),
            )
            .with_field(
                tr!("User Attribute Name"),
                Field::new()
                    .name("user-attr")
                    .required(true)
                    .placeholder("uid / sAMAccountName"),
            )
    }

    input_panel
        .with_right_field(tr!("Fallback Server"), Field::new().name("server2"))
        .with_right_field(
            tr!("Port"),
            Number::<u16>::new()
                .name("port")
                .placeholder(tr!("Default"))
                .min(1),
        )
        .with_field(
            tr!("Anonymous Search"),
            Checkbox::new()
                .name("anonymous_search")
                .submit(false)
                .default(true),
        )
        .with_right_field(
            tr!("Mode"),
            Combobox::new()
                .name("mode")
                .default("ldap")
                .required(true)
                .items(mode_items)
                .render_value(|mode: &AttrValue| {
                    let text = match mode.as_str() {
                        "ldap" => "LDAP",
                        "ldap+starttls" => "STARTTLS",
                        "ldaps" => "LDAPS",
                        unknown => unknown,
                    };
                    html! {text}
                }),
        )
        .with_field(
            tr!("Bind Domain Name"),
            Field::new()
                .name("bind-dn")
                .required(!anonymous_search)
                .disabled(anonymous_search)
                .placeholder(
                    props
                        .ad_realm
                        .map(|_| "user@company.net")
                        .unwrap_or("cn=user,dc=company,dc=net"),
                ),
        )
        .with_right_field(
            tr!("Verify Certificate"),
            Checkbox::new().name("verify").disabled(!tls_enabled),
        )
        .with_field(
            tr!("Bind Password"),
            Field::new()
                .name("password")
                .disabled(anonymous_search)
                .input_type(InputType::Password)
                .placeholder(is_edit.then(|| tr!("Unchanged")))
                .show_peek_icon(true),
        )
        .with_large_field(tr!("Comment"), Field::new().name("comment"))
        .into()
}

impl Component for ProxmoxAuthEditLDAP {
    type Message = ();
    type Properties = AuthEditLDAP;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let is_edit = props.realm.is_some();

        let action = if is_edit { tr!("Edit") } else { tr!("Add") };

        let base_url = props.base_url.to_string();
        let on_submit = move |form_context| {
            let base_url = base_url.clone();
            async move {
                if is_edit {
                    update_item(form_context, base_url.clone()).await
                } else {
                    create_item(form_context, base_url.clone()).await
                }
            }
        };

        let title = if props.ad_realm.unwrap_or_default() {
            tr!("Active Directory Server")
        } else {
            tr!("LDAP Server")
        };

        EditWindow::new(action + ": " + &title)
            .loader(
                props
                    .realm
                    .as_ref()
                    .map(|realm| format!("{}/{}", props.base_url, percent_encode_component(realm)))
                    .map(|url| move || load_realm(url.clone())),
            )
            .renderer({
                let props = props.clone();
                move |form_ctx: &FormContext| render_panel(form_ctx.clone(), props.clone())
            })
            .on_done(props.on_close.clone())
            .on_submit(on_submit)
            .into()
    }
}

impl From<AuthEditLDAP> for VNode {
    fn from(val: AuthEditLDAP) -> Self {
        let comp = VComp::new::<ProxmoxAuthEditLDAP>(Rc::new(val), None);
        VNode::from(comp)
    }
}
