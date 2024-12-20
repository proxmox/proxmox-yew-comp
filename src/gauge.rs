use std::rc::Rc;

use pwt::widget::Column;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::props::IntoOptionalInlineHtml;
use pwt::widget::canvas::{Canvas, Path, Text};

use pwt_macros::builder;

#[derive(Properties, Clone, PartialEq)]
#[builder]
pub struct Gauge {
    #[prop_or(0f32)]
    #[builder]
    pub value: f32,

    #[prop_or_default]
    pub status: Option<Html>,

    #[prop_or(0.95)]
    #[builder]
    pub critical_threshold: f32,

    #[prop_or(0.8)]
    #[builder]
    pub warning_threshold: f32,
}

impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

impl Gauge {
    /// Create a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    /// Builder style method to set the status text.
    pub fn status(mut self, status: impl IntoOptionalInlineHtml) -> Self {
        self.set_status(status);
        self
    }

    /// Method to set the status text.
    pub fn set_status(&mut self, status: impl IntoOptionalInlineHtml) {
        self.status = status.into_optional_inline_html();
    }
}

#[doc(hidden)]
pub struct ProxmoxGauge {}

impl Component for ProxmoxGauge {
    type Message = ();
    type Properties = Gauge;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let fraction = props.value;

        let fraction = fraction.max(0f32).min(1f32);

        let r = 100f32;
        let stroke_width = 10.0;
        let space = stroke_width / 2.0;

        let x = (r + space) - (std::f32::consts::PI * fraction).cos() * r;
        let y = (r + space) - (std::f32::consts::PI * fraction).sin() * r;

        let color_class = if fraction > props.critical_threshold {
            "pwt-stroke-error"
        } else if fraction > props.warning_threshold {
            "pwt-stroke-warning"
        } else {
            "pwt-stroke-primary"
        };

        let percentage = (fraction * 1000.0).round() / 10.0;
        let percentage = format!("{}%", percentage);

        let canvas = Canvas::new()
            .width(2.0 * (r + space))
            .height(r + space)
            .with_child(
                Path::new()
                    .fill("none")
                    .class("pwt-stroke-surface")
                    .stroke_width(stroke_width)
                    .d(format!(
                        "M {},{} A {r},{r} 0,0,1 {},{}",
                        space,
                        space + r,
                        2.0 * r + space,
                        r + space,
                    )),
            )
            .with_child(
                Path::new()
                    .fill("none")
                    .class(color_class)
                    .stroke_width(stroke_width)
                    .d(format!(
                        "M {},{} A {r},{r} 0,0,1 {},{}",
                        space,
                        space + r,
                        x,
                        y,
                    )),
            )
            .with_child(
                Text::new(percentage)
                    .class("pwt-font-display-medium")
                    .attribute("text-anchor", "middle")
                    .position(r + space, r - 15.0),
            );

        let mut column = Column::new()
            .class("pwt-align-items-center")
            .gap(2)
            .with_child(canvas);

        if let Some(status) = props.status.as_ref() {
            column.add_child(html! {
                status.clone()
            });
        }

        column.into()
    }
}

impl Into<VNode> for Gauge {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxGauge>(Rc::new(self), None);
        VNode::from(comp)
    }
}
