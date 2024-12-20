use std::rc::Rc;

use pwt::prelude::*;
use pwt::widget::form::Combobox;

use pwt_macros::widget;

#[widget(comp=ProxmoxTaskTypeSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
pub struct TaskTypeSelector {}

impl TaskTypeSelector {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct ProxmoxTaskTypeSelector {
    pub items: Rc<Vec<AttrValue>>,
}

impl Component for ProxmoxTaskTypeSelector {
    type Message = ();
    type Properties = TaskTypeSelector;

    fn create(_ctx: &Context<Self>) -> Self {
        let mut items = crate::utils::registered_task_types();

        items.sort();

        let items = items.into_iter().map(|a| a.into()).collect();
        Self {
            items: Rc::new(items),
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        Combobox::new()
            .with_std_props(&props.std_props)
            .with_input_props(&props.input_props)
            .items(self.items.clone())
            .editable(true)
            .into()
    }
}
