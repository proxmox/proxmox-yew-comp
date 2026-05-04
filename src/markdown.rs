use std::collections::HashMap;

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};
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

impl Default for Markdown {
    fn default() -> Self {
        Self::new()
    }
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

/// Mirrors marked's `Slugger.serialize` (adapted from marked.js v4.2.x):
/// lower-case + trim whitespace, drop the punctuation set marked enumerates, replace remaining
/// whitespace runs with `-` (each whitespace char becomes one `-`, matching marked's `\s -> -`).
fn serialize_heading(value: &str) -> String {
    let mut out = String::new();
    for c in value.trim().chars() {
        // marked's removal set: U+2000-U+206F, U+2E00-U+2E7F, plus the listed ASCII punct.
        // `-` and `_` and alphanumerics (incl. non-ASCII letters) are kept on purpose.
        let removed = matches!(c,
            '\u{2000}'..='\u{206F}' | '\u{2E00}'..='\u{2E7F}'
            | '\\' | '\'' | '!' | '"' | '#' | '$' | '%' | '&'
            | '(' | ')' | '*' | '+' | ',' | '.' | '/' | ':' | ';'
            | '<' | '=' | '>' | '?' | '@' | '[' | ']' | '^' | '`'
            | '{' | '|' | '}' | '~'
        );
        if removed {
            continue;
        }
        if c.is_whitespace() {
            out.push('-');
        } else {
            for lower in c.to_lowercase() {
                out.push(lower);
            }
        }
    }
    out
}

/// Mirrors marked's `Slugger.getNextSafeSlug`: dedups by appending `-N` and skipping any suffixed
/// candidate that has itself already been emitted, so a literal `## Foo 1` after `## Foo` doesn't
/// collide with the auto-suffixed second `## Foo`.
struct Slugger {
    seen: HashMap<String, u32>,
}

impl Slugger {
    fn new() -> Self {
        Self {
            seen: HashMap::new(),
        }
    }

    fn slug(&mut self, value: &str) -> String {
        let original = serialize_heading(value);
        // Empty slug means the heading has no usable text to derive id from. Don't pollute `seen`
        // with empty key to avoid that subsequent empty ones start emitting id="-1", id="-2", ...
        if original.is_empty() {
            return original;
        }
        let mut slug = original.clone();
        let mut counter: u32 = 0;
        if let Some(&c) = self.seen.get(&slug) {
            counter = c;
            loop {
                counter += 1;
                slug = format!("{original}-{counter}");
                if !self.seen.contains_key(&slug) {
                    break;
                }
            }
        }
        self.seen.insert(original, counter);
        self.seen.insert(slug.clone(), 0);
        slug
    }

    /// Reserve an explicit slug (e.g. set via the `{#anchor}` syntax) so a later auto-id that
    /// happens to slugify to the same value de-duplicates against it.
    fn reserve(&mut self, slug: &str) {
        self.seen.entry(slug.to_string()).or_insert(0);
    }
}

/// Convert Markdown to sanitized Html
pub fn markdown_to_html(text: &str) -> Html {
    let options = Options::all();
    let parser = Parser::new_ext(text, options);

    // Auto-derive `id` for headings without an explicit one, using a slug of the heading text.
    // Mirrors marked's `headerIds: true` on the JS side so fragment links like
    // `[link](#section)` resolve to the matching heading; the sanitizer then namespaces both
    // the heading id and the link href with the same prefix, keeping them in sync.
    let mut events: Vec<Event<'_>> = Vec::new();
    let mut heading_start: Option<usize> = None;
    let mut heading_text = String::new();
    let mut slugger = Slugger::new();

    for event in parser {
        match &event {
            Event::Start(Tag::Heading { id, .. }) => {
                if let Some(explicit) = id {
                    // An explicit `{#anchor}` does not need an auto-id, but reserve it in the
                    // slugger so a later heading slugifying to the same value gets `-1`.
                    slugger.reserve(explicit);
                } else {
                    heading_start = Some(events.len());
                    heading_text.clear();
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(idx) = heading_start.take() {
                    let slug = slugger.slug(&heading_text);
                    if !slug.is_empty() {
                        if let Event::Start(Tag::Heading {
                            level,
                            classes,
                            attrs,
                            ..
                        }) = events[idx].clone()
                        {
                            events[idx] = Event::Start(Tag::Heading {
                                level,
                                id: Some(CowStr::from(slug)),
                                classes,
                                attrs,
                            });
                        }
                    }
                }
            }
            // Capture all text-bearing events so a heading like `## $a + b$` or
            // `## **bold** title` still yields a non-empty slug.
            Event::Text(t) | Event::Code(t) | Event::InlineMath(t) | Event::DisplayMath(t) => {
                if heading_start.is_some() {
                    heading_text.push_str(t);
                }
            }
            _ => {}
        }
        events.push(event);
    }

    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, events.into_iter());

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Marked-parity points where a naive slug differs: ASCII punctuation is dropped (not replaced
    /// with `-`), `-` and `_` are kept verbatim, each whitespace char becomes one `-`.
    #[test]
    fn serialize_marked_parity() {
        assert_eq!(serialize_heading("Inline HTML"), "inline-html");
        assert_eq!(
            serialize_heading("Use the `printf()` function"),
            "use-the-printf-function",
        );
        assert_eq!(serialize_heading("Foo - Bar"), "foo---bar");
    }

    /// Some Slugger edge-cases
    #[test]
    fn slug_dedup_contract() {
        /// `## Foo`, `## Foo 1`, `## Foo` -> `foo`, `foo-1`, `foo-2` (do-while skip),
        let mut s = Slugger::new();
        assert_eq!(s.slug("Foo"), "foo");
        assert_eq!(s.slug("Foo 1"), "foo-1");
        assert_eq!(s.slug("Foo"), "foo-2");

        /// all-punct headings don't pollute `seen` and so don't start emitting `-1`, `-2`,
        let mut s = Slugger::new();
        assert_eq!(s.slug("!!!"), "");
        assert_eq!(s.slug("!!!"), "");

        /// explicit `{#anchor}` ids reserve their slot against a later auto-id collision.
        let mut s = Slugger::new();
        s.reserve("bar");
        assert_eq!(s.slug("Bar"), "bar-1");
    }
}
