use std::rc::Rc;

use pwt::widget::form::{FormContext, Boolean, Combobox};
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::InputPanel;
use pwt::widget::form::{Field};

use crate::percent_encoding::percent_encode_component;

use pwt_macros::builder;

use crate::EditWindow;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct AuthEditOpenID {
    /// Close/Abort callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    pub on_close: Option<Callback<()>>,

    #[prop_or("/access/domains".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,

    /// Edit existing realm
    #[builder(IntoPropValue, into_prop_value)]
    pub realm: Option<AttrValue>,
}

impl AuthEditOpenID {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[doc(hidden)]
pub struct ProxmoxAuthEditOpenID {}

fn render_input_form(form_ctx: FormContext, props: AuthEditOpenID) -> Html {
    let is_edit = props.realm.is_some();

    let username_claim_items = Rc::new(vec![
        "subject".into(),
        "username".into(),
        "email".into(),
    ]);

    let prompt_items = Rc::new(vec![
        "none".into(),
        "login".into(),
        "consent".into(),
        "select_account".into(),
    ]);

    InputPanel::new()
        .show_advanced(form_ctx.get_show_advanced())
        .class("pwt-p-2")
        .with_large_field(
            tr!("Issuer URL"),
            Field::new().name("issuer-url").required(true)
        )

        .with_field(
            tr!("Realm"),
            Field::new().name("realm").required(true).disabled(is_edit))
        .with_right_field(
            tr!("Autocreate Users"),
            Boolean::new().name("autocreate")
        )

        .with_field(
            tr!("Client ID"),
            Field::new().name("client-id").required(true))
        .with_right_field(
            tr!("Username Claim"),
            Combobox::new()
                .name("username-claim")
                .disabled(is_edit)
                .editable(true)
                .placeholder(tr!("Default"))
                .items(username_claim_items)
        )

        .with_field(
            tr!("Client Key"),
            Field::new().name("client-key"))
        .with_right_field(
            tr!("Scopes"),
            Field::new()
                .name("scopes")
                .placeholder(tr!("Default") + " (" + &tr!("email profile") + ")")
        )

        .with_field(
            tr!("Prompt"),
            Combobox::new()
                .name("prompt")
                .editable(true)
                .placeholder(tr!("Auth-Provider Default"))
                .items(prompt_items)
        )

        .with_large_field(tr!("Comment"), Field::new().name("comment"))
        .with_advanced_spacer()
        .with_advanced_field(tr!("ACR Values"), Field::new().name("acr-values"))
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

        EditWindow::new(action + ": " + &tr!("OpenID Connect Server"))
            //.resizable(false)
            //.style("width: 840px; height:600px;")
            .advanced_checkbox(true)
            .loader(props.realm.as_ref().map(|realm| format!("{}/{}", props.base_url, percent_encode_component(realm))))
            .renderer({
                let props = props.clone();
                move |form_ctx: &FormContext| render_input_form(form_ctx.clone(), props.clone())
            })
            .on_done(props.on_close.clone())
            //.with_child(panel)
            .into()
    }
}

impl Into<VNode> for AuthEditOpenID {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxAuthEditOpenID>(Rc::new(self), None);
        VNode::from(comp)
    }
}
