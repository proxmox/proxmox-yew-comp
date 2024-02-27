use std::rc::Rc;

use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::{Container, Dialog, ThemeDensitySelector, ThemeModeSelector, ThemeNameSelector};

use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct ThemeDialog {
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,
}

impl ThemeDialog {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[doc(hidden)]
pub struct ProxmoxThemeDialog {}

impl Component for ProxmoxThemeDialog {
    type Message = ();
    type Properties = ThemeDialog;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Dialog::new(tr!("Theme"))
            .style("min-width: 400px;")
            .on_close(props.on_close.clone())
            .with_child(
                Container::new()
                    .class("pwt-p-4 pwt-gap-2 pwt-d-grid pwt-align-items-baseline")
                    .attribute("style", "grid-template-columns: 1fr 1fr;")
                    .with_child(tr!("Theme name"))
                    .with_child(ThemeNameSelector::new())
                    .with_child(tr!("Density"))
                    .with_child(ThemeDensitySelector::new())
                    .with_child(tr!("Theme mode"))
                    .with_child(ThemeModeSelector::new()),
            )
            .into()
    }
}

impl Into<VNode> for ThemeDialog {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxThemeDialog>(Rc::new(self), None);
        VNode::from(comp)
    }
}
