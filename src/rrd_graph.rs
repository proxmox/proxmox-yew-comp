use std::rc::Rc;

use serde_json::json;
use wasm_bindgen::JsValue;

use yew::html::IntoPropValue;
use yew::prelude::*;
use yew::virtual_dom::{VComp, VNode};

use pwt::dom::DomSizeObserver;
use pwt::prelude::*;
use pwt::widget::Panel;

#[derive(Clone, PartialEq, Properties)]
pub struct RRDGraph {
    #[prop_or_default]
    pub title: Option<AttrValue>,
    // Legend Label
    #[prop_or_default]
    pub label: Option<String>,
    #[prop_or_default]
    pub class: Classes,

    pub data: Rc<(Vec<i64>, Vec<f64>)>,
}

impl RRDGraph {
    pub fn new(data: Rc<(Vec<i64>, Vec<f64>)>) -> Self {
        yew::props!(RRDGraph { data })
    }

    pub fn title(mut self, title: impl IntoPropValue<Option<AttrValue>>) -> Self {
        self.set_title(title);
        self
    }

    pub fn set_title(&mut self, title: impl IntoPropValue<Option<AttrValue>>) {
        self.title = title.into_prop_value();
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Builder style method to add a html class
    pub fn class(mut self, class: impl Into<Classes>) -> Self {
        self.add_class(class);
        self
    }

    /// Method to add a html class
    pub fn add_class(&mut self, class: impl Into<Classes>) {
        self.class.push(class);
    }
}

pub enum Msg {
    Reload,
    ViewportResize(f64, f64),
}

pub struct PwtRRDGraph {
    node_ref: NodeRef,
    size_observer: Option<DomSizeObserver>,
    width: usize,
    uplot: Option<JsValue>,
}

const DEFAULT_RRD_WIDTH: usize = 800;
const DEFAULT_RRD_HEIGHT: usize = 250;

impl Component for PwtRRDGraph {
    type Message = Msg;
    type Properties = RRDGraph;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Reload);
        Self {
            node_ref: NodeRef::default(),
            size_observer: None,
            width: DEFAULT_RRD_WIDTH,
            uplot: None,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Reload => true,
            Msg::ViewportResize(width, _height) => {
                self.width = width as usize;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Panel::new()
            .title(props.title.clone())
            .class(props.class.clone())
            .with_child(
                Container::new()
                    .padding(2)
                    .with_child(html! {<div ref={self.node_ref.clone()}>}),
            )
            .into()
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        if let Some(uplot) = &self.uplot {
            let data = pwt::to_js_value(ctx.props().data.as_ref()).unwrap();
            crate::uplot_set_size(uplot, self.width, DEFAULT_RRD_HEIGHT);
            crate::uplot_set_data(uplot, &data);
        }
        true
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            if let Some(el) = self.node_ref.cast::<web_sys::Element>() {
                let link = ctx.link().clone();
                let size_observer = DomSizeObserver::new(&el, move |(width, height)| {
                    link.send_message(Msg::ViewportResize(width, height));
                });

                self.size_observer = Some(size_observer);
            }

            let props = ctx.props();

            let mut serie1 = json!({
                // initial toggled state (optional)
                "show": true,

                "spanGaps": false,

                // series style
                "stroke": "#94ae0a",
                "fill": "#94ae0a80",
                "width": 1,
            });

            if let Some(ref label) = props.label {
                serie1["label"] = label.as_str().into();
            }

            let opts = json!({
                "width": self.width,
                "height": DEFAULT_RRD_HEIGHT,
                "series": [ {}, serie1 ],
            });

            let opts = pwt::to_js_value(&opts).unwrap();

            let data = pwt::to_js_value(props.data.as_ref()).unwrap();

            self.uplot = Some(crate::uplot(&opts, &data, self.node_ref.get().unwrap()));
        }
    }
}

impl Into<VNode> for RRDGraph {
    fn into(self) -> VNode {
        let comp = VComp::new::<PwtRRDGraph>(Rc::new(self), None);
        VNode::from(comp)
    }
}
