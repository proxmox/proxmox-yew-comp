use yew::html::IntoPropValue;

use pwt::prelude::*;
use pwt::widget::{Container, Meter, Row};

use pwt_macros::{builder, widget};

#[widget(comp=ProxmoxMeterLabel, @element)]
#[derive(Properties, Clone, PartialEq)]
#[builder]
pub struct MeterLabel {
    pub title: AttrValue,

    #[prop_or_default]
    pub icon_class: Option<Classes>,

    /// Minimum value (default 0)
    ///
    /// Lower numeric bound. This must be less than the maximum value.
    #[prop_or(0.0)]
    #[builder(IntoPropValue, into_prop_value, 1.0)]
    pub min: f32,

    /// Maximum value (default 1)
    ///
    /// Upper numeric bound. This must be greater than the minimum
    /// value.
    #[prop_or(1.0)]
    #[builder(IntoPropValue, into_prop_value, 1.0)]
    pub max: f32,

    /// Define the low end range.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub low: Option<f32>,

    /// Define the high end range.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub high: Option<f32>,

    /// Optimal value.
    ///
    /// This gives an indication where along the range is considered
    /// preferable. For example, if it is between the min attribute
    /// and the low attribute, then the lower range is considered
    /// preferred. The meter's bar color depends on whether the value
    /// is less than or equal to the optimum value.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub optimum: Option<f32>,

    /// Current value (default 0).
    ///
    /// This must be between the minimum and maximum values. If
    /// specified, but not within the range given by the min attribute
    /// and max attribute, the value is equal to the nearest end of
    /// the range.
    #[prop_or(0f32)]
    #[builder]
    pub value: f32,

    #[prop_or_default]
    pub status: Option<Html>,
}

impl MeterLabel {
    /// Create a new instance.
    pub fn new(title: impl Into<AttrValue>) -> Self {
        yew::props!(Self {
            title: title.into(),
        })
    }

    pub fn with_zero_optimum(title: impl Into<AttrValue>) -> Self {
        Self::new(title).low(0.75).high(0.9).optimum(0.0)
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
pub struct ProxmoxMeterLabel {}

impl Component for ProxmoxMeterLabel {
    type Message = ();
    type Properties = MeterLabel;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let icon = props.icon_class.as_ref().map(|icon_class| {
            let class = classes!(icon_class.clone(), "pwt-pe-2");
            html! {<i {class}/>}
        });

        let status = match &props.status {
            Some(text) => text.clone(),
            None => html! {format!("{:.2} %", props.value * 100.0)},
        };

        let text_row = Row::new()
            .gap(2)
            .with_child(
                html! {<div class="pwt-white-space-nowrap">{icon}{props.title.clone()}</div>},
            )
            .with_flex_spacer()
            .with_child(html! {<div class="pwt-white-space-nowrap">{status}</div>});

        Container::new()
            .with_std_props(&props.std_props)
            .listeners(&props.listeners)
            .with_child(text_row)
            .with_child(
                Meter::new()
                    .class("pwt-mt-1")
                    .value(props.value)
                    .min(props.min)
                    .max(props.max)
                    .low(props.low)
                    .high(props.high)
                    .optimum(props.optimum),
            )
            .into()
    }
}
