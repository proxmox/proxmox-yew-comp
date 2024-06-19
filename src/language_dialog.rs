use std::rc::Rc;

use pwt::state::Language;
use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::{Button, Container, Dialog, LanguageSelector, Row};

use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct LanguageDialog {
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,
}

impl LanguageDialog {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub enum Msg {
    Select(String),
    Apply,
}

#[doc(hidden)]
pub struct ProxmoxLanguageDialog {
    orig_lang: String,
    lang: Option<String>,
}

impl Component for ProxmoxLanguageDialog {
    type Message = Msg;
    type Properties = LanguageDialog;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            orig_lang: Language::load(),
            lang: None,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Select(lang) => {
                self.lang = Some(lang);
                true
            }
            Msg::Apply => {
                if let Some(lang) = &self.lang {
                    Language::store(lang);
                }
                true
            }
        }
    }
    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let changed = match &self.lang {
            None => false,
            Some(lang) => lang != &self.orig_lang,
        };

        Dialog::new(tr!("Language"))
            .min_width(300)
            .on_close(props.on_close.clone())
            .with_child(
                Row::new()
                    .gap(2)
                    .class("pwt-align-items-baseline")
                    .padding(2)
                    .with_child(Container::new().with_child(tr! {"Language"}))
                    .with_child(
                        LanguageSelector::new()
                            .class("pwt-flex-fill")
                            .on_change(ctx.link().callback(Msg::Select)),
                    ),
            )
            .with_child(
                Row::new().padding(2).with_flex_spacer().with_child(
                    Button::new(tr!("Apply"))
                        .class("pwt-scheme-primary")
                        .disabled(!changed)
                        .onclick(ctx.link().callback(|_| Msg::Apply)),
                ),
            )
            .into()
    }
}

impl Into<VNode> for LanguageDialog {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxLanguageDialog>(Rc::new(self), None);
        VNode::from(comp)
    }
}
