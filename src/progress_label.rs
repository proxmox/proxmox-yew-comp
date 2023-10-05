use pwt::prelude::*;
use pwt::widget::{Container, Progress, Row};

use pwt_macros::{builder, widget};

#[widget(comp=ProxmoxProgressLabel, @element)]
#[derive(Properties, Clone, PartialEq)]
#[builder]
pub struct ProgressLabel {
    pub title: AttrValue,

    #[prop_or_default]
    pub icon_class: Option<Classes>,

    #[prop_or(0f32)]
    #[builder]
    pub value: f32,

    #[prop_or_default]
    pub status: Option<Html>,
}

impl ProgressLabel {
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
}

#[doc(hidden)]
pub struct ProxmoxProgressLabel {}

impl Component for ProxmoxProgressLabel {
    type Message = ();
    type Properties = ProgressLabel;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();


        let icon = props.icon_class.as_ref().map(|icon_class| {
            let class = classes!(icon_class.clone(), "pwt-pe-2");
            html!{<i {class}/>}
        });

        let status = match &props.status {
            Some(text) => text.clone(),
            None => html!{format!("{:.2} %", props.value * 100.0)},
        };

        let text_row = Row::new()
            .gap(2)
            .with_child(html!{<div class="pwt-white-space-nowrap">{icon}{props.title.clone()}</div>})
            .with_flex_spacer()
            .with_child(html!{<div class="pwt-white-space-nowrap">{status}</div>});

        Container::new()
            .with_std_props(&props.std_props)
            .listeners(&props.listeners)
            .with_child(text_row)
            .with_child(
                Progress::new().class("pwt-mt-1").value(props.value)
            )
            .into()
    }
}
