use std::rc::Rc;

use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

#[derive(Properties, PartialEq, Clone)]
pub struct JournalView {
    #[prop_or_default]
    pub key: Option<Key>,
    #[prop_or(AttrValue::Static("/nodes/localhost/journal"))]
    pub url: AttrValue,
}

impl JournalView {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct ProxmoxJournalView {


}

impl Component for ProxmoxJournalView {
    type Message = ();
    type Properties = JournalView;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        //let props = ctx.props();

        // FIXME: implement something useful
        html!{
            <div class="pwt-fit">
                //<div style="height:34210072px;position: relative;">
                <div style="height:33210072px;position: relative;box-sizing: border-box;">
                    <div class="log-content" style="position:absolute;top:2000px;">{"TEST LOG LINE"}</div>
                </div>
            </div>
        }
    }
}

impl Into<VNode> for JournalView {
    fn into(self) -> VNode {
        let key = self.key.clone();
        let comp = VComp::new::<ProxmoxJournalView>(Rc::new(self), key);
        VNode::from(comp)
    }
}
