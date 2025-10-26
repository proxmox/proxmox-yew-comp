use std::rc::Rc;

use anyhow::Error;

use pwt::widget::form::{Checkbox, Combobox, FormContext};
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::Field;
use pwt::widget::InputPanel;

use crate::form::delete_empty_values;
use crate::percent_encoding::percent_encode_component;

use pwt_macros::builder;

use crate::EditWindow;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct AuthEditOpenID {
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

impl Default for AuthEditOpenID {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthEditOpenID {
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
            "acr-values",
            "autocreate",
            "comment",
            "client-key",
            "scopes",
            "prompt",
        ],
        true,
    );

    let name = form_ctx.read().get_field_text("realm");

    let url = format!("{base_url}/{}", percent_encode_component(&name));

    crate::http_put(&url, Some(data)).await
}

#[doc(hidden)]
pub struct ProxmoxAuthEditOpenID {}

fn render_input_form(form_ctx: FormContext, props: AuthEditOpenID) -> Html {
    let is_edit = props.realm.is_some();

    let username_claim_items = Rc::new(vec!["subject".into(), "username".into(), "email".into()]);

    let prompt_items = Rc::new(vec![
        "none".into(),
        "login".into(),
        "consent".into(),
        "select_account".into(),
    ]);

    InputPanel::new()
        .show_advanced(form_ctx.get_show_advanced())
        .padding(4)
        .with_large_field(
            tr!("Issuer URL"),
            Field::new().name("issuer-url").required(true),
        )
        .with_field(
            tr!("Realm"),
            Field::new()
                .name("realm")
                .required(true)
                .disabled(is_edit)
                .submit(!is_edit),
        )
        .with_right_field(tr!("Autocreate Users"), Checkbox::new().name("autocreate"))
        .with_field(tr!("Default Realm"), Checkbox::new().name("default"))
        .with_right_field(
            tr!("Username Claim"),
            Combobox::new()
                .name("username-claim")
                .disabled(is_edit)
                .submit(!is_edit)
                .editable(true)
                .placeholder(tr!("Default"))
                .items(username_claim_items),
        )
        .with_field(
            tr!("Client ID"),
            Field::new().name("client-id").required(true),
        )
        .with_right_field(
            tr!("Scopes"),
            Field::new()
                .name("scopes")
                .placeholder(tr!("Default") + " (" + &tr!("email profile") + ")"),
        )
        .with_field(tr!("Client Key"), Field::new().name("client-key"))
        .with_right_field(
            tr!("Prompt"),
            Combobox::new()
                .name("prompt")
                .editable(true)
                .placeholder(tr!("Auth-Provider Default"))
                .items(prompt_items),
        )
        .with_large_field(tr!("Comment"), Field::new().name("comment"))
        .with_advanced_spacer()
        .with_large_advanced_field(tr!("ACR Values"), Field::new().name("acr-values"))
        .into()
}

impl Component for ProxmoxAuthEditOpenID {
    type Message = ();
    type Properties = AuthEditOpenID;

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

        EditWindow::new(action + ": " + &tr!("OpenID Connect Server"))
            .advanced_checkbox(true)
            .loader(
                props
                    .realm
                    .as_ref()
                    .map(|realm| format!("{}/{}", props.base_url, percent_encode_component(realm))),
            )
            .renderer({
                let props = props.clone();
                move |form_ctx: &FormContext| render_input_form(form_ctx.clone(), props.clone())
            })
            .on_done(props.on_close.clone())
            .on_submit(on_submit)
            .into()
    }
}

impl From<AuthEditOpenID> for VNode {
    fn from(val: AuthEditOpenID) -> Self {
        let comp = VComp::new::<ProxmoxAuthEditOpenID>(Rc::new(val), None);
        VNode::from(comp)
    }
}
