use std::rc::Rc;

use yew::html::IntoPropValue;
use yew::prelude::*;

use pwt::widget::form::Combobox;

use pwt::props::{FieldBuilder, WidgetBuilder};
use pwt_macros::widget;

#[widget(comp=ProxmoxBondXmitHashPolicySelector, @input)]
#[derive(Clone, Properties, PartialEq)]
pub struct BondXmitHashPolicySelector {
    /// The default value.
    #[prop_or_default]
    pub default: Option<AttrValue>,
}

impl BondXmitHashPolicySelector {
    /// Create a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    /// Builder style method to set the default item.
    pub fn default(mut self, default: impl IntoPropValue<Option<AttrValue>>) -> Self {
        self.set_default(default);
        self
    }

    /// Method to set the default item.
    pub fn set_default(&mut self, default: impl IntoPropValue<Option<AttrValue>>) {
        self.default = default.into_prop_value();
    }
}

pub struct ProxmoxBondXmitHashPolicySelector {
    items: Rc<Vec<AttrValue>>,
}

impl Component for ProxmoxBondXmitHashPolicySelector {
    type Message = ();
    type Properties = BondXmitHashPolicySelector;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            items: Rc::new(
                XMIT_HASH_POLICY
                    .iter()
                    .map(|s| AttrValue::from(*s))
                    .collect::<Vec<AttrValue>>(),
            ),
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Combobox::new()
            .with_std_props(&props.std_props)
            .with_input_props(&props.input_props)
            .default(&props.default)
            .items(Rc::clone(&self.items))
            .into()
    }
}

#[allow(dead_code)]
static XMIT_HASH_POLICY: &'static [&'static str] = &["layer2", "layer2+3", "layer3+4"];
