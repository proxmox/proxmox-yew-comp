use std::rc::Rc;

use yew::html::IntoPropValue;

use pwt::prelude::*;
use pwt::widget::form::Combobox;

use pwt::props::{FieldBuilder, WidgetBuilder};
use pwt_macros::{builder, widget};

#[widget(comp=ProxmoxAcmeChallengeTypeSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct AcmeChallengeTypeSelector {
    /// The default value.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub default: Option<AttrValue>,
}

impl Default for AcmeChallengeTypeSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl AcmeChallengeTypeSelector {
    /// Create a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct ProxmoxAcmeChallengeTypeSelector {}

impl Component for ProxmoxAcmeChallengeTypeSelector {
    type Message = ();
    type Properties = AcmeChallengeTypeSelector;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Combobox::new()
            .with_std_props(&props.std_props)
            .with_input_props(&props.input_props)
            .default(&props.default)
            .items(ACME_CHALLENGE_TYPE_ITEMS.with(Rc::clone))
            .into()
    }
}

thread_local! {
    static ACME_CHALLENGE_TYPE_ITEMS: Rc<Vec<AttrValue>> = Rc::new(vec![
        AttrValue::Static("DNS"),
        AttrValue::Static("HTTP"),
    ]);
}
