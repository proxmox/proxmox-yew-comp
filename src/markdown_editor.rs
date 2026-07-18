use anyhow::Error;
use gloo_timers::callback::Timeout;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::JsCast;
use web_sys::HtmlTextAreaElement;
use yew::html::{IntoEventCallback, IntoPropValue};

use pwt::css::ColorScheme;
use pwt::prelude::*;
use pwt::widget::form::{
    IntoValidateFn, ManagedField, ManagedFieldContext, ManagedFieldMaster, ManagedFieldScopeExt,
    ManagedFieldState, ValidateFn,
};
use pwt::widget::{Button, Column, Container, Row, SegmentedButton};

use pwt_macros::{builder, widget};

use crate::Markdown;

/// How far the preview trails the textarea. Rendering it means parsing and sanitizing the whole
/// document, which is too much work to redo on every keystroke.
const PREVIEW_DEBOUNCE_MS: u32 = 300;

/// Which pane(s) a [`MarkdownEditor`] shows. Serializable so a consumer can persist the user's
/// pick and hand it back as [`MarkdownEditor::initial_mode`].
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum MarkdownViewMode {
    /// Only the textarea.
    #[default]
    Write,
    /// Textarea and preview side by side.
    Split,
    /// Only the rendered preview.
    Preview,
}

/// Markdown editor form field: a plain-text textarea with a formatting toolbar and a live,
/// sanitized preview rendered through the [`Markdown`] viewer.
#[widget(comp=ManagedFieldMaster<MarkdownEditorField>, @input, @element)]
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct MarkdownEditor {
    /// Force value (controlled use without a `FormContext`). Ignored if `name` is set.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub value: Option<AttrValue>,
    /// Force validation result (controlled use). Only honoured together with `value`.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub valid: Option<Result<Value, String>>,
    /// Default value.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default: Option<AttrValue>,
    /// Optional extra validation run on the raw markdown text.
    #[prop_or_default]
    pub validate: Option<ValidateFn<String>>,
    /// Minimum visible rows. The textarea auto-grows beyond this to fit its content, this only
    /// sets the lower bound. Default value is 4.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub rows: Option<u32>,
    /// Initial view mode. Default: [`MarkdownViewMode::Write`].
    #[builder]
    #[prop_or(MarkdownViewMode::Write)]
    pub initial_mode: MarkdownViewMode,
    /// Hide the formatting toolbar.
    #[builder]
    #[prop_or_default]
    pub hide_toolbar: bool,
    /// Emitted on every value change (including `FormContext` driven changes).
    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_change: Option<Callback<String>>,
    /// Emitted when the user types.
    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_input: Option<Callback<String>>,
    /// Emitted when the user switches the view mode, for consumers that persist the choice.
    #[builder_cb(IntoEventCallback, into_event_callback, MarkdownViewMode)]
    #[prop_or_default]
    pub on_mode_change: Option<Callback<MarkdownViewMode>>,
}

impl Default for MarkdownEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownEditor {
    /// Creates a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    /// Builder style method to set the validate callback.
    pub fn validate(mut self, validate: impl IntoValidateFn<String>) -> Self {
        self.validate = validate.into_validate_fn();
        self
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub enum Msg {
    /// User typed in the textarea.
    Input(String),
    /// Switch view mode.
    SetMode(MarkdownViewMode),
    /// Surround the selection with `prefix`/`suffix` (e.g. `**`, `` ` ``).
    /// `placeholder` is inserted when nothing is selected.
    Wrap(&'static str, &'static str, &'static str),
    /// Prefix every selected line (e.g. `## `, `- `, `> `).
    Prefix(&'static str),
    /// Insert a `[text](url)` link around the selection.
    Link,
    /// Catch the preview up with the current text, after the debounce elapsed.
    SyncPreview,
}

#[doc(hidden)]
pub struct MarkdownEditorField {
    input_ref: NodeRef,
    mode: MarkdownViewMode,
    /// Selection (UTF-16 units) to restore after the next render, set by toolbar edits.
    pending_selection: Option<(u32, u32)>,
    /// Text the preview renders, trailing the live value by [`PREVIEW_DEBOUNCE_MS`].
    preview_text: String,
    /// Pending catch-up; dropping it cancels the timer, which is what debounces typing.
    preview_timeout: Option<Timeout>,
    state: ManagedFieldState,
}

pwt::impl_deref_mut_property!(MarkdownEditorField, state, ManagedFieldState);

fn value_to_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

fn str_to_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn utf16_to_string(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

impl MarkdownEditorField {
    fn apply(&mut self, edit: Msg) {
        let el = match self.input_ref.cast::<HtmlTextAreaElement>() {
            Some(el) => el,
            None => return,
        };

        // selection{Start,End} and set_selection_range are UTF-16 offsets, so we
        // read and slice in UTF-16 to stay correct for non-ASCII text
        let text = el.value();
        let u = str_to_utf16(&text);
        let len = u.len() as u32;
        let start = el.selection_start().ok().flatten().unwrap_or(0).min(len);
        let end = el.selection_end().ok().flatten().unwrap_or(start).min(len);
        let selected = utf16_to_string(&u[start as usize..end as usize]);

        let (range_start, replacement, select) = match edit {
            Msg::Wrap(prefix, suffix, placeholder) => {
                let content = if selected.is_empty() {
                    placeholder.to_string()
                } else {
                    selected
                };
                let replacement = format!("{prefix}{content}{suffix}");
                let a = start + utf16_len(prefix);
                (start, replacement, (a, a + utf16_len(&content)))
            }
            Msg::Link => {
                let label = if selected.is_empty() {
                    "text".to_string()
                } else {
                    selected
                };
                let replacement = format!("[{label}](url)");
                let a = start + 1 + utf16_len(&label) + 2;
                (start, replacement, (a, a + 3))
            }
            Msg::Prefix(prefix) => {
                let line_start = u[..start as usize]
                    .iter()
                    .rposition(|&c| c == 0x000A)
                    .map(|i| i as u32 + 1)
                    .unwrap_or(0);
                let region = utf16_to_string(&u[line_start as usize..end as usize]);
                let replacement = region
                    .split('\n')
                    .map(|line| format!("{prefix}{line}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let caret = line_start + utf16_len(&replacement);
                (line_start, replacement, (caret, caret))
            }
            _ => return,
        };

        // Replace [range_start, end) via the browser's native editing
        //
        // though according to MDN, this is not supported anymore and obsolete,
        // there is no alternative. as browsers cannot agree on the implementation, it
        // is marked as this in the HTML5 spec.
        //
        // so, consider this a FIXME later, once there is a proper alternative
        let _ = el.focus();
        let _ = el.set_selection_range(range_start, end);
        if let Some(doc) = el
            .owner_document()
            .and_then(|d| d.dyn_into::<web_sys::HtmlDocument>().ok())
        {
            let _ = doc.exec_command_with_show_ui_and_value("insertText", false, &replacement);
        }
        self.pending_selection = Some(select);
    }

    /// Grow/shrink the textarea so its height matches its content, with the
    /// `min-height` set in `view` as the lower bound.
    ///
    /// First, the height is collapsed so the box can shrink, then `scrollHeight`
    /// (the content height) is read which is used to set the new height.
    fn autosize(&self) {
        let el = match self.input_ref.cast::<HtmlTextAreaElement>() {
            Some(el) => el,
            None => return,
        };
        let style = el.style();
        let _ = style.set_property("height", "0px");
        let border = el.offset_height() - el.client_height();
        let height = el.scroll_height() + border;
        let _ = style.set_property("height", &format!("{height}px"));
    }
}

#[derive(PartialEq)]
pub struct ValidateClosure {
    required: bool,
    validate: Option<ValidateFn<String>>,
}

impl ManagedField for MarkdownEditorField {
    type Properties = MarkdownEditor;
    type Message = Msg;
    type ValidateClosure = ValidateClosure;

    fn validation_args(props: &Self::Properties) -> Self::ValidateClosure {
        ValidateClosure {
            required: props.input_props.required,
            validate: props.validate.clone(),
        }
    }

    fn validator(props: &Self::ValidateClosure, value: &Value) -> Result<Value, Error> {
        let text = value_to_text(value);

        if text.is_empty() {
            if props.required {
                return Err(Error::msg(tr!("Field may not be empty.")));
            }
            return Ok(Value::String(String::new()));
        }

        if let Some(validate) = &props.validate {
            validate.apply(&text)?;
        }

        Ok(Value::String(text))
    }

    fn create(ctx: &ManagedFieldContext<Self>) -> Self {
        let props = ctx.props();
        let mut text = String::new();

        if let Some(default) = &props.default {
            text = default.to_string();
        }

        if let Some(force) = &props.value {
            text = force.to_string();
        }

        let default: Value = props.default.as_deref().unwrap_or("").into();

        Self {
            input_ref: NodeRef::default(),
            mode: props.initial_mode,
            pending_selection: None,
            preview_text: text.clone(),
            preview_timeout: None,
            state: ManagedFieldState::new(Value::String(text), default),
        }
    }

    fn update(&mut self, ctx: &ManagedFieldContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Input(text) => {
                ctx.link().update_value(text.clone());
                if let Some(on_input) = &ctx.props().on_input {
                    on_input.emit(text);
                }
                true
            }
            Msg::SetMode(mode) => {
                self.mode = mode;
                // the preview may have just become visible, show the current text at once
                self.preview_timeout = None;
                self.preview_text = value_to_text(&self.state.value);
                if let Some(on_mode_change) = &ctx.props().on_mode_change {
                    on_mode_change.emit(mode);
                }
                true
            }
            Msg::SyncPreview => {
                self.preview_timeout = None;
                let text = value_to_text(&self.state.value);
                if self.preview_text == text {
                    return false;
                }
                self.preview_text = text;
                true
            }
            edit => {
                self.apply(edit);
                // The value sync + re-render come from the `input` event that
                // execCommand emits (via `oninput` -> `Msg::Input`), so don't
                // render here with the not-yet-updated state.
                false
            }
        }
    }

    fn value_changed(&mut self, ctx: &ManagedFieldContext<Self>) {
        if let Some(on_change) = &ctx.props().on_change {
            on_change.emit(value_to_text(&self.state.value));
        }

        // Nothing renders the preview in write mode, so leave it stale and let the mode switch
        // catch it up; otherwise restart the debounce, dropping any timer still pending.
        if self.mode != MarkdownViewMode::Write {
            let link = ctx.link().clone();
            self.preview_timeout = Some(Timeout::new(PREVIEW_DEBOUNCE_MS, move || {
                link.send_message(Msg::SyncPreview)
            }));
        }
    }

    fn changed(&mut self, ctx: &ManagedFieldContext<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if props.value != old_props.value || props.valid != old_props.valid {
            if let Some(forced) = &props.value {
                ctx.link()
                    .force_value(Some(forced.to_string()), props.valid.clone());
            }
        }

        true
    }

    fn view(&self, ctx: &ManagedFieldContext<Self>) -> Html {
        let props = ctx.props();
        let link = ctx.link();
        let value = value_to_text(&self.state.value);
        let valid = self.state.result.is_ok();
        let disabled = props.input_props.disabled;

        let toolbar = (!props.hide_toolbar).then(|| {
            let fmt_disabled = disabled || self.mode == MarkdownViewMode::Preview;
            let fmt_btn = |icon: &'static str, msg: Msg| {
                Button::new_icon(icon)
                    .disabled(fmt_disabled)
                    .on_activate(link.callback(move |_| msg.clone()))
            };
            // Icon-only and grouped: the view mode is a small, rarely-touched control that should
            // not compete with the formatting actions for attention. The label lives in the
            // tooltip and the accessible name.
            let mode_btn = |icon: &'static str, tip: String, mode: MarkdownViewMode| {
                let active = self.mode == mode;
                Button::new_icon(icon)
                    .pressed(active)
                    .class(active.then_some(ColorScheme::Primary))
                    .aria_label(tip.clone())
                    .attribute("title", tip)
                    .on_activate(link.callback(move |_| Msg::SetMode(mode)))
            };
            Row::new()
                .class("pwt-align-items-center")
                .gap(1)
                .with_child(fmt_btn("fa fa-bold", Msg::Wrap("**", "**", "bold")))
                .with_child(fmt_btn("fa fa-italic", Msg::Wrap("*", "*", "italic")))
                .with_child(fmt_btn("fa fa-code", Msg::Wrap("`", "`", "code")))
                .with_child(fmt_btn("fa fa-header", Msg::Prefix("## ")))
                .with_child(fmt_btn("fa fa-list-ul", Msg::Prefix("- ")))
                .with_child(fmt_btn("fa fa-quote-left", Msg::Prefix("> ")))
                .with_child(fmt_btn("fa fa-link", Msg::Link))
                .with_flex_spacer()
                .with_child(
                    SegmentedButton::new()
                        .aria_label(tr!("View mode"))
                        .with_button(mode_btn(
                            "fa fa-pencil",
                            tr!("Write"),
                            MarkdownViewMode::Write,
                        ))
                        .with_button(mode_btn(
                            "fa fa-columns",
                            tr!("Split"),
                            MarkdownViewMode::Split,
                        ))
                        .with_button(mode_btn(
                            "fa fa-eye",
                            tr!("Preview"),
                            MarkdownViewMode::Preview,
                        )),
                )
        });

        let oninput = link.callback(|e: InputEvent| {
            let el: HtmlTextAreaElement = e.target_unchecked_into();
            Msg::Input(el.value())
        });

        let min_rows = props.rows.unwrap_or(4);
        let style =
            format!("overflow-y: hidden; flex: 1 1 0; min-width: 0; min-height: {min_rows}lh;");

        let textarea = html! {
            <textarea
                ref={self.input_ref.clone()}
                class={classes!("pwt-textarea", (!valid).then_some("is-invalid"))}
                style={style}
                rows={min_rows.to_string()}
                value={value.clone()}
                placeholder={props.input_props.placeholder.clone()}
                disabled={disabled}
                {oninput}
            />
        };

        // Render the debounced text, not the live one: handing Markdown an unchanged prop keeps
        // it from re-parsing and re-sanitizing the document on every keystroke.
        let preview: Html = {
            let body: Html = if self.preview_text.trim().is_empty() {
                html! { <span class="pwt-opacity-50">{ tr!("Nothing to preview") }</span> }
            } else {
                // render through the Markdown viewer so the preview goes through the same
                // sanitizer as the finally displayed content
                Markdown::new().text(self.preview_text.clone()).into()
            };
            Container::new()
                .class("pwt-border pwt-shape-small pwt-p-2 pwt-overflow-auto")
                .style("flex", "1 1 0")
                .style("min-width", "0")
                .with_child(body)
                .into()
        };

        let body = match self.mode {
            MarkdownViewMode::Write => Row::new().with_child(textarea),
            MarkdownViewMode::Preview => Row::new().with_child(preview),
            MarkdownViewMode::Split => Row::new()
                .class("pwt-align-items-start")
                .gap(2)
                .with_child(textarea)
                .with_child(preview),
        };

        Column::new()
            .gap(1)
            .with_optional_child(toolbar)
            .with_child(body)
            .into()
    }

    fn rendered(&mut self, ctx: &ManagedFieldContext<Self>, first_render: bool) {
        if let Some((start, end)) = self.pending_selection.take() {
            if let Some(el) = self.input_ref.cast::<HtmlTextAreaElement>() {
                let _ = el.focus();
                let _ = el.set_selection_range(start, end);
            }
        }
        if first_render && ctx.props().input_props.autofocus {
            if let Some(el) = self.input_ref.cast::<HtmlTextAreaElement>() {
                let _ = el.focus();
            }
        }
        // Keep the textarea height matched to its content on every render
        // (covers typing, toolbar edits, form loads and mode switches).
        self.autosize();
    }
}
