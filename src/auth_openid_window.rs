use std::rc::Rc;

use pwt::widget::form::FormContext;
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
pub struct AuthOpenIDWindow {
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

impl AuthOpenIDWindow {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[doc(hidden)]
pub struct ProxmoxAuthOpenIDWindow {}

fn render_input_form(form_ctx: FormContext, props: AuthOpenIDWindow) -> Html {
    let is_edit = props.realm.is_some();

    InputPanel::new()
        .show_advanced(form_ctx.get_show_advanced())
        .class("pwt-p-2")
        .with_field(tr!("Realm"), Field::new().name("realm").disabled(is_edit))
        .with_large_field(tr!("Comment"), Field::new().name("comment"))
        .with_advanced_spacer()
        .with_advanced_field(tr!("ACR Values"), Field::new().name("acr-values"))
        .into()
}

impl Component for ProxmoxAuthOpenIDWindow {
    type Message = ();
    type Properties = AuthOpenIDWindow;

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

impl Into<VNode> for AuthOpenIDWindow {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxAuthOpenIDWindow>(Rc::new(self), None);
        VNode::from(comp)
    }
}
