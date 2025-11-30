use std::rc::Rc;

use pwt::prelude::*;
use pwt::widget::form::Combobox;

use pwt::props::{FieldBuilder, WidgetBuilder};
use pwt_macros::{builder, widget};

#[widget(comp=PveLogLevelSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct LogLevelSelector {}

impl LogLevelSelector {
    /// Create a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct PveLogLevelSelector {
    items: Rc<Vec<AttrValue>>,
}

impl Component for PveLogLevelSelector {
    type Message = ();
    type Properties = LogLevelSelector;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            items: Rc::new(vec![
                AttrValue::Static("nolog"),
                AttrValue::Static("emerg"),
                AttrValue::Static("alert"),
                AttrValue::Static("crit"),
                AttrValue::Static("err"),
                AttrValue::Static("warning"),
                AttrValue::Static("notice"),
                AttrValue::Static("info"),
                AttrValue::Static("debug"),
            ]),
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Combobox::new()
            .with_std_props(&props.std_props)
            .with_input_props(&props.input_props)
            .items(self.items.clone())
            .placeholder("nolog")
            .show_filter(false)
            .default("nolog")
            .into()
    }
}
