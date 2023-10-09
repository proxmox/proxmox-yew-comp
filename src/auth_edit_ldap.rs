use std::rc::Rc;

use anyhow::Error;
use pwt::css::{Flex, Overflow};

use pwt::widget::form::{Boolean, Combobox, FormContext, Number};
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{delete_empty_values, Field, TristateBoolean};
use pwt::widget::{InputPanel, TabBarItem, TabPanel};

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
}

impl AuthEditLDAP {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

async fn create_item(form_ctx: FormContext, base_url: String) -> Result<(), Error> {
    let data = form_ctx.get_submit_data();
    crate::http_post(base_url, Some(data)).await
}

async fn update_item(form_ctx: FormContext, base_url: String) -> Result<(), Error> {
    let data = form_ctx.get_submit_data();

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
        ],
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
        .into()
}

fn render_sync_form(_form_ctx: FormContext, _props: AuthEditLDAP) -> Html {
    //let is_edit = props.realm.is_some();

    InputPanel::new()
        .class(Flex::Fill)
        .class(Overflow::Auto)
        .class("pwt-p-4")
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
        .with_field(
            tr!("Enable new users"),
            TristateBoolean::new()
                .name("enable-new")
                .submit_empty(true)
                .null_text(tr!("Default") + " (" + &tr!("Yes") + ")"),
        )
        .with_field(
            tr!("Remove ACLs of vanished users"),
            Boolean::new().name("remove-vanished-acl"),
        )
        .with_field(
            tr!("Remove vanished user"),
            Boolean::new().name("remove-vanished-entry"),
        )
        .with_field(
            tr!("Remove vanished properties"),
            Boolean::new().name("remove-vanished-properties"),
        )
        .into()
}

fn render_general_form(form_ctx: FormContext, props: AuthEditLDAP) -> Html {
    let is_edit = props.realm.is_some();

    let mode_items = Rc::new(vec!["ldap".into(), "ldap+starttls".into(), "ldaps".into()]);

    let anonymous_search = form_ctx
        .read()
        .get_field_value("anonymous_search")
        .map(|v| v.as_bool())
        .flatten()
        .unwrap_or(false);

    let tls_enabled = form_ctx
        .read()
        .get_field_value("mode")
        .map(|v| match v.as_str() {
            Some("ldap+starttls") => true,
            Some("ldaps") => true,
            _ => false,
        })
        .unwrap_or(false);

    InputPanel::new()
        .class(Flex::Fill)
        .class(Overflow::Auto)
        .class("pwt-p-4")
        .with_field(
            tr!("Realm"),
            Field::new()
                .name("realm")
                .required(true)
                .disabled(is_edit)
                .submit(!is_edit),
        )
        .with_right_field(tr!("Server"), Field::new().name("server1").required(true))
        .with_field(
            tr!("Base Domain Name"),
            Field::new()
                .name("base-dn")
                .required(true)
                .placeholder("cn=Users,dc=company,dc=net"),
        )
        .with_right_field(tr!("Fallback Server"), Field::new().name("server2"))
        .with_field(
            tr!("User Attribute Name"),
            Field::new()
                .name("user-attr")
                .required(true)
                .placeholder("uid / sAMAccountName"),
        )
        .with_right_field(
            tr!("Port"),
            Number::<u16>::new()
                .name("port")
                .placeholder(tr!("Default"))
                .min(1),
        )
        .with_field(
            tr!("Anonymous Search"),
            Boolean::new()
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
                .placeholder("cn=user,dc=company,dc=net"),
        )
        .with_right_field(
            tr!("Verify Certificate"),
            Boolean::new().name("verify").disabled(!tls_enabled),
        )
        .with_field(
            tr!("Bind Password"),
            Field::new()
                .name("password")
                .disabled(anonymous_search)
                .input_type("password")
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

        EditWindow::new(action + ": " + &tr!("LDAP Server"))
            .loader(
                props
                    .realm
                    .as_ref()
                    .map(|realm| format!("{}/{}", props.base_url, percent_encode_component(realm))),
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

impl Into<VNode> for AuthEditLDAP {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxAuthEditLDAP>(Rc::new(self), None);
        VNode::from(comp)
    }
}
