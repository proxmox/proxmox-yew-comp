use std::rc::Rc;

use indexmap::IndexMap;

use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};
use yew::html::IntoPropValue;

use pwt::prelude::*;
use pwt::props::RenderFn;
use pwt::state::NavigationContainer;
use pwt::widget::TabBar;

#[derive(Clone, PartialEq, Properties)]
pub struct ConfigPanel {
    pub key: Option<Key>,
    #[prop_or_default]
    pub tabs: IndexMap<Key, RenderFn<()>>,
    #[prop_or_default]
    pub bar: TabBar,

    pub title: String,
}

impl ConfigPanel {

    pub fn new(title: impl Into<String>) -> Self {
        yew::props!(ConfigPanel {
            title: title.into(),
        })
    }

    /// Builder style method to set the yew `key` property
    pub fn key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn with_panel(
        mut self,
        key: impl Into<Key>,
        label: impl Into<AttrValue>,
        icon_class: impl IntoPropValue<Option<AttrValue>>,
        renderer: impl 'static + Fn(&()) -> Html,
    ) -> Self {
        let key = key.into();

        self.bar.add_item(key.clone(), label, icon_class);

        self.tabs.insert(key, RenderFn::new(renderer));

        self
    }
}

pub enum Msg {
    Select(Option<Key>),
}

pub struct PwtConfigPanel {
    active: Option<Key>,
}


impl Component for PwtConfigPanel {
    type Message = Msg;
    type Properties = ConfigPanel;

    fn create(_ctx: &Context<Self>) -> Self {
        Self { active: None }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Select(opt_key) => {
                self.active = opt_key;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let bar = NavigationContainer::new()
            .with_child(
                props.bar.clone()
                    .router(true)
                    .on_select(ctx.link().callback(|key| Msg::Select(key)))
            );

        let content = match &self.active {
            None => html!{},
            Some(key) => {
                if let Some(renderer) = props.tabs.get(key) {
                    renderer.apply(&())
                } else {
                    html!{}
                }
            }
        };

        let content = html!{ <div class="pwt-flex-fill pwt-overflow-auto">{content}</div>};

        html!{
            <div class="pwt-panel pwt-fit">
                <div class="pwt-panel-header">
                    <div class="pwt-panel-header-text pwt-pb-2">
                        {&props.title}
                    </div>
                    {bar}
                </div>
                {content}
            </div>
        }
    }
}

impl Into<VNode> for ConfigPanel {
    fn into(self) -> VNode {
        let key = self.key.clone();
        let comp = VComp::new::<PwtConfigPanel>(Rc::new(self), key);
        VNode::from(comp)
    }
}
