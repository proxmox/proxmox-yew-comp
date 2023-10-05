use yew::html::IntoPropValue;

use pwt::{prelude::*, widget::Container};

use crate::sanitize_html;

use pwt_macros::{builder, widget};

/// Markdown Viewer
#[widget(comp=ProxmoxMarkdown, @element)]
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct Markdown {
    /// Markdown text
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    text: Option<AttrValue>,
}

impl Markdown {
    /// Creates a new instance.
    pub fn new() -> Self {
        yew::props! {Self {}}
    }
}

#[doc(hidden)]
pub struct ProxmoxMarkdown {
    html: Html,
}

/// Convert Markdown to sanitized Html
pub fn markdown_to_html(text: &str) -> Html {
    let options = pulldown_cmark::Options::all();
    let parser = pulldown_cmark::Parser::new_ext(text, options);

    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);

    match sanitize_html(&html_output) {
        Ok(html) => Html::from_html_unchecked(html.into()),
        Err(err) => {
            log::error!("sanitize html failed: {}", err);
            html! {text}
        }
    }
}

impl Component for ProxmoxMarkdown {
    type Message = ();
    type Properties = Markdown;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let html = match &props.text {
            Some(text) => markdown_to_html(text),
            None => html! {},
        };

        Self { html }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if props.text != old_props.text {
            self.html = match &props.text {
                Some(text) => markdown_to_html(text),
                None => html! {},
            };
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        Container::new()
            .class("pwt-embedded-html")
            .with_std_props(&props.std_props)
            .listeners(&props.listeners)
            .with_child(self.html.clone())
            .into()
    }
}
