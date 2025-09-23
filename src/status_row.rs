use pwt::css::Display;

use pwt::prelude::*;
use pwt::widget::{Container, Row};

use pwt_macros::widget;

#[widget(comp=ProxmoxStatusRow, @element)]
#[derive(Properties, Clone, PartialEq)]
pub struct StatusRow {
    pub title: AttrValue,

    #[prop_or_default]
    pub icon_class: Option<Classes>,

    #[prop_or_default]
    pub status: Option<Html>,

    #[prop_or_default]
    pub icon_right: bool,
}

impl StatusRow {
    /// Create a new instance.
    pub fn new(title: impl Into<AttrValue>) -> Self {
        yew::props!(Self {
            title: title.into(),
        })
    }

    /// Builder style method to set the icon class.
    pub fn icon_class(mut self, icon_class: impl Into<Classes>) -> Self {
        self.set_icon_class(icon_class);
        self
    }

    /// Method to set the icon class.
    pub fn set_icon_class(&mut self, icon_class: impl Into<Classes>) {
        self.icon_class = Some(icon_class.into());
    }

    /// Builder style method to set the status text.
    pub fn status(mut self, status: impl Into<Html>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Method to control which side the icon should go on.
    pub fn set_icon_right(&mut self, icon_right: impl Into<bool>) {
        self.icon_right = icon_right.into();
    }

    /// Builder style method to control which side the icon should go on.
    pub fn icon_right(mut self, icon_right: impl Into<bool>) -> Self {
        self.set_icon_right(icon_right);
        self
    }
}

#[doc(hidden)]
pub struct ProxmoxStatusRow {}

impl Component for ProxmoxStatusRow {
    type Message = ();
    type Properties = StatusRow;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let (left_icon, right_icon) = match &props.icon_class {
            Some(icon_class) => {
                let icon = Container::from_tag("i").class(icon_class.clone());
                if props.icon_right {
                    (None, Some(icon))
                } else {
                    (Some(icon), None)
                }
            }
            None => (None, None),
        };

        let status = match &props.status {
            Some(text) => text.clone(),
            None => html! {"-"},
        };

        Row::new()
            .with_std_props(&props.std_props)
            .class(Display::Flex) // we need to set this again
            .listeners(&props.listeners)
            .gap(2)
            .with_optional_child(left_icon)
            .with_child(html! {<div class="pwt-white-space-nowrap">{props.title.clone()}</div>})
            .with_flex_spacer()
            .with_child(html! {<div class="pwt-white-space-nowrap">{status}</div>})
            .with_optional_child(right_icon)
            .into()
    }
}
