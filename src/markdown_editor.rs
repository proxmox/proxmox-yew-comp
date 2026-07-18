use anyhow::Error;
use gloo_timers::callback::Timeout;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

/// Markdown editor form field: a plain-text textarea with a live, sanitized preview rendered
/// through the [`Markdown`] viewer.
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
    /// Catch the preview up with the current text, after the debounce elapsed.
    SyncPreview,
}

#[doc(hidden)]
pub struct MarkdownEditorField {
    input_ref: NodeRef,
    mode: MarkdownViewMode,
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

impl MarkdownEditorField {
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

        // Icon-only: the view mode is a small, rarely-touched control, so the label lives in the
        // tooltip and the accessible name rather than taking width.
        let mode_btn = |icon: &'static str, tip: String, mode: MarkdownViewMode| {
            let active = self.mode == mode;
            Button::new_icon(icon)
                .pressed(active)
                // a chrome control sitting on top of the text, so keep it smaller than a regular
                // button: the label font drives the glyph and the height, the padding the rest
                .class("pwt-font-label-small")
                .style("padding", "var(--pwt-spacer-1)")
                .class(active.then_some(ColorScheme::Primary))
                .aria_label(tip.clone())
                .attribute("title", tip)
                .on_activate(link.callback(move |_| Msg::SetMode(mode)))
        };
        // Floated over the bottom-right corner of the panes rather than sitting in a row of its
        // own, so the switcher costs no vertical space in a cramped detail view. The wrapper stays
        // invisible: a fill or a border around it reads as a second, competing box on top of the
        // text, and the buttons bring their own shape already.
        let switcher = Container::new()
            .style("position", "absolute")
            .style("background-color", "transparent")
            .style("border", "none !important")
            .style("bottom", "var(--pwt-spacer-1)")
            .style("right", "var(--pwt-spacer-1)")
            .style("z-index", "1")
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
            );

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

        // relative: the positioning context the floating switcher anchors to
        Column::new()
            .style("position", "relative")
            .with_child(body)
            .with_child(switcher)
            .into()
    }

    fn rendered(&mut self, ctx: &ManagedFieldContext<Self>, first_render: bool) {
        if first_render && ctx.props().input_props.autofocus {
            if let Some(el) = self.input_ref.cast::<HtmlTextAreaElement>() {
                let _ = el.focus();
            }
        }
        // Keep the textarea height matched to its content on every render
        // (covers typing, form loads and mode switches).
        self.autosize();
    }
}
