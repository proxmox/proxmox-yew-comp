use pwt::css::{Display, FlexDirection};
use pwt::prelude::*;
use pwt::widget::{Container, SizeObserver};

use pwt_macros::widget;

#[widget(comp=ProxmoxRRDGrid, @container)]
#[derive(Default, PartialEq, Clone, Properties)]
pub struct RRDGrid {}

impl RRDGrid {
    /// Create a new instance.
    pub fn new() -> Self {
        Self::default()
    }
}
pub enum Msg {
    ViewportResize(f64, f64),
}

#[doc(hidden)]
pub struct ProxmoxRRDGrid {
    size_observer: Option<SizeObserver>,
    cols: usize,
    col_width: usize,
}

impl Component for ProxmoxRRDGrid {
    type Message = Msg;
    type Properties = RRDGrid;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            size_observer: None,
            cols: 1,
            col_width: 800,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::ViewportResize(width, _height) => {
                let width = width as usize;
                let cw = 800;
                let width = width.max(cw);
                let padding = 6;
                let mut cols = (width / cw) as usize;
                if cols == 0 {
                    cols = 1;
                }
                let col_width = (width as usize - 2 * padding) / cols;
                self.cols = cols;
                self.col_width = col_width - padding;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Container::form_widget_props(props.std_props.clone(), None)
            .class(Display::Flex)
            .class(FlexDirection::Column)
            .class("pwt-overflow-auto")
            .with_child(
                Container::new()
                    .class(Display::Grid)
                    .class("pwt-gap-2 pwt-w-100")
                    .attribute(
                        "style",
                        format!("grid-template-columns:repeat({}, 1fr);", self.cols),
                    )
                    .children(props.children.clone()),
            )
            .with_child(html!{<div class="pwt-flex-fill"/>})
            .into()
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        let props = ctx.props();
        if first_render {
            if let Some(el) = props.std_props.node_ref.cast::<web_sys::Element>() {
                let link = ctx.link().clone();
                let size_observer = SizeObserver::new(&el, move |(width, height)| {
                    link.send_message(Msg::ViewportResize(width, height));
                });
                self.size_observer = Some(size_observer);
            }
        }
    }
}
