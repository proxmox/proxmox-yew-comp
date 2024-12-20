use std::rc::Rc;

use yew::html::IntoPropValue;
use yew::prelude::*;

use pwt::widget::form::Combobox;

use pwt::props::{FieldBuilder, WidgetBuilder};
use pwt_macros::widget;

#[widget(comp=ProxmoxBondModeSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
pub struct BondModeSelector {
    /// The default value.
    #[prop_or_default]
    pub default: Option<AttrValue>,
}

impl BondModeSelector {
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

pub struct ProxmoxBondModeSelector {
    items: Rc<Vec<AttrValue>>,
}

impl Component for ProxmoxBondModeSelector {
    type Message = ();
    type Properties = BondModeSelector;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            items: Rc::new(
                BOND_MODES
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
static BOND_MODES: &'static [&'static str] = &[
    "balance-rr",
    "active-backup",
    "balance-xor",
    "broadcast",
    "802.3ad",
    "balance-tlb",
    "balance-alb",
];
