use std::rc::Rc;

use yew::prelude::*;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::Button;

#[derive(Clone, PartialEq, Properties)]
pub struct HelpButton {
    #[prop_or_default]
    section: Option<String>,
    #[prop_or_default]
    class: Classes,
}

impl Default for HelpButton {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpButton {
    pub fn new() -> Self {
        yew::props!(Self {})
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

    pub fn section(mut self, section: impl Into<String>) -> Self {
        self.section = Some(section.into());
        self
    }
}

#[function_component(PbsHelpButton)]
pub fn pbs_help_button(props: &HelpButton) -> Html {
    let button = if props.section.is_some() {
        Button::new("?").class("circle").aria_label("help")
    } else {
        Button::new("Documentation")
            .icon_class("fa fa-book")
            .aria_label("documentation")
    };

    button
        .class(props.class.clone())
        .onclick({
            let url = get_help_link(props.section.as_ref().map(|s| s.as_str()));
            move |_| {
                let window = web_sys::window().unwrap();
                let _ = window.open_with_url_and_target(&url, "top");
            }
        })
        .into()
}

impl From<HelpButton> for VNode {
    fn from(val: HelpButton) -> Self {
        let comp = VComp::new::<PbsHelpButton>(Rc::new(val), None);
        VNode::from(comp)
    }
}

fn get_help_link(_section: Option<&str>) -> String {
    // TODO:
    format!("/docs/index.html")
}
