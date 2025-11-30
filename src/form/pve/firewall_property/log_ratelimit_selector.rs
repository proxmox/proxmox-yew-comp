use std::rc::Rc;

use pwt::prelude::*;
use pwt::widget::form::{Combobox, Number};

use pwt::widget::{Labelable, Row};

use pwt_macros::builder;

use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct LogRatelimitSelector {
    /// Field name used by rate input ([u64])
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(LogRatelimitSelector::RATE_NAME)]
    pub rate_name: AttrValue,

    /// Field name used by unit input ([String])
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(LogRatelimitSelector::RATE_UNIT)]
    pub unit_name: AttrValue,

    /// Field disabled flag.
    #[builder]
    #[prop_or_default]
    pub disabled: bool,

    #[prop_or_default]
    label_id: Option<AttrValue>,
}

impl LogRatelimitSelector {
    pub const RATE_NAME: AttrValue = AttrValue::Static("_lograte_");
    pub const RATE_UNIT: AttrValue = AttrValue::Static("_lograte_unit_");

    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

// impl Labelable, so that we can use it with InputPanel
impl Labelable for LogRatelimitSelector {
    fn name(&self) -> Option<AttrValue> {
        Some(self.rate_name.clone())
    }
    fn set_label_id(&mut self, label_id: AttrValue) {
        self.label_id = Some(label_id);
    }
    fn disabled(&self) -> bool {
        self.disabled
    }
}

pub struct LogRatelimitSelectorComp {
    units: Rc<Vec<AttrValue>>,
}

impl Component for LogRatelimitSelectorComp {
    type Message = ();
    type Properties = LogRatelimitSelector;

    fn create(_ctx: &Context<Self>) -> Self {
        let units = Rc::new(vec![
            AttrValue::Static("second"),
            AttrValue::Static("minute"),
            AttrValue::Static("hour"),
            AttrValue::Static("day"),
        ]);
        Self { units }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        Row::new()
            .gap(1)
            .with_child(
                Number::<u64>::new()
                    .style("min-width", "0")
                    .name(&props.rate_name)
                    .label_id(props.label_id.clone())
                    .disabled(props.disabled)
                    .submit(false)
                    .placeholder("1")
                    .min(1)
                    .max(99),
            )
            .with_child(
                Combobox::new()
                    .name(&props.unit_name)
                    .items(self.units.clone())
                    .required(true)
                    .disabled(props.disabled)
                    .default("seconds")
                    .submit(false),
            )
            .into()
    }
}

impl Into<VNode> for LogRatelimitSelector {
    fn into(self) -> VNode {
        let comp = VComp::new::<LogRatelimitSelectorComp>(Rc::new(self), None);
        VNode::from(comp)
    }
}
