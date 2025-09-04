use std::rc::Rc;

use anyhow::Error;
use pwt::state::PersistentState;
use serde_json::Value;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

use proxmox_client::ApiResponseData;

use pwt::props::{
    AsCssStylesMut, CssStyles, IntoSubmitCallback, RenderFn, SubmitCallback, WidgetStyleBuilder,
};
use pwt::widget::form::{Checkbox, Form, FormContext, Hidden, ResetButton, SubmitButton};
use pwt::widget::{AlertDialog, Column, Dialog, Mask, Row};
use pwt::{prelude::*, AsyncPool};

use pwt_macros::builder;

use crate::{ApiLoadCallback, IntoApiLoadCallback};

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct EditWindow {
    /// Yew component key
    #[prop_or_default]
    pub key: Option<Key>,

    /// Window title
    #[prop_or_default]
    pub title: AttrValue,

    /// Show advanced checkbox
    #[prop_or_default]
    #[builder]
    pub advanced_checkbox: bool,

    // Form renderer.
    #[prop_or_default]
    pub renderer: Option<RenderFn<FormContext>>,

    /// Form data loader.
    #[builder_cb(IntoApiLoadCallback, into_api_load_callback, Value)]
    #[prop_or_default]
    pub loader: Option<ApiLoadCallback<Value>>,

    /// Submit button text.
    ///
    /// Default is Add, or Update if there is a loader.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub submit_text: Option<AttrValue>,

    /// Submit the digest if the loader returned one.
    #[prop_or(true)]
    #[builder]
    pub submit_digest: bool,

    /// Determines if the dialog can be moved
    #[prop_or(true)]
    #[builder]
    pub draggable: bool,

    /// Determines if the dialog can be resized
    #[prop_or_default]
    #[builder]
    pub resizable: bool,

    /// Determines if the dialog should be auto centered
    #[prop_or(true)]
    #[builder]
    pub auto_center: bool,

    /// CSS style for the dialog window.
    #[prop_or_default]
    pub styles: CssStyles,

    /// Close/Abort callback.
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    /// Done callback, called after Close, Abort or Submit.
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_done: Option<Callback<()>>,

    /// Submit callback.
    #[prop_or_default]
    pub on_submit: Option<SubmitCallback<FormContext>>,

    /// Reset button press callback.
    #[prop_or_default]
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    pub on_reset: Option<Callback<()>>,

    /// Data change callback.
    #[builder_cb(IntoEventCallback, into_event_callback, FormContext)]
    #[prop_or_default]
    pub on_change: Option<Callback<FormContext>>,

    /// Determines if the window is in edit mode (enabled reset button + dirty tracking)
    ///
    /// Set automatically if a loader is present, can be turned off or on manually with this option.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub edit: Option<bool>,
}

impl AsCssStylesMut for EditWindow {
    fn as_css_styles_mut(&mut self) -> &mut CssStyles {
        &mut self.styles
    }
}

impl WidgetStyleBuilder for EditWindow {}

impl EditWindow {
    pub fn new(title: impl Into<AttrValue>) -> Self {
        yew::props!(Self {
            title: title.into(),
        })
    }

    pwt::impl_yew_std_props_builder!();

    pub fn renderer(mut self, renderer: impl 'static + Fn(&FormContext) -> Html) -> Self {
        self.renderer = Some(RenderFn::new(renderer));
        self
    }

    pub fn on_submit(mut self, callback: impl IntoSubmitCallback<FormContext>) -> Self {
        self.on_submit = callback.into_submit_callback();
        self
    }

    pub fn is_edit(&self) -> bool {
        if let Some(is_edit) = self.edit {
            is_edit
        } else {
            self.loader.is_some()
        }
    }
}

pub enum Msg {
    FormDataChange,
    Submit,
    SubmitResult(Result<(), Error>),
    Load,
    LoadResult(Result<ApiResponseData<Value>, Error>),
    ClearError,
    ShowAdvanced(bool),
}

#[doc(hidden)]
pub struct PwtEditWindow {
    loading: bool,
    form_ctx: FormContext,
    submit_error: Option<String>,
    load_error: Option<String>,
    show_advanced: PersistentState<bool>,
    async_pool: AsyncPool,
}

impl Component for PwtEditWindow {
    type Message = Msg;
    type Properties = EditWindow;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Load);

        let form_ctx = FormContext::new().on_change(ctx.link().callback(|_| Msg::FormDataChange));

        let show_advanced = PersistentState::new("proxmox-form-show-advanced");
        form_ctx.set_show_advanced(*show_advanced);

        Self {
            form_ctx,
            loading: false,
            submit_error: None,
            load_error: None,
            show_advanced,
            async_pool: AsyncPool::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::ShowAdvanced(show_advanced) => {
                self.form_ctx.set_show_advanced(show_advanced);
                self.show_advanced.update(show_advanced);
                true
            }
            Msg::ClearError => {
                self.submit_error = None;
                self.load_error = None;
                true
            }
            Msg::Load => {
                if let Some(loader) = props.loader.clone() {
                    self.loading = true;
                    let link = ctx.link().clone();
                    self.async_pool.spawn(async move {
                        let res = loader.apply().await;
                        link.send_message(Msg::LoadResult(res));
                    });
                }
                true
            }
            Msg::LoadResult(result) => {
                self.loading = false;
                match result {
                    Err(err) => self.load_error = Some(err.to_string()),
                    Ok(api_resp) => {
                        let mut value = api_resp.data;
                        if props.submit_digest {
                            if let Some(digest) = api_resp.attribs.get("digest") {
                                value["digest"] = digest.clone();
                            }
                        }
                        self.form_ctx.load_form(value);
                    }
                }
                true
            }
            Msg::FormDataChange => {
                if self.submit_error.is_some() {
                    self.submit_error = None;
                }
                if let Some(on_change) = &props.on_change {
                    on_change.emit(self.form_ctx.clone());
                }
                // Note: we redraw on any data change
                true
            }
            Msg::Submit => {
                if let Some(on_submit) = props.on_submit.clone() {
                    let link = ctx.link().clone();
                    let form_ctx = self.form_ctx.clone();
                    self.loading = true;
                    self.async_pool.spawn(async move {
                        let result = on_submit.apply(form_ctx).await;
                        link.send_message(Msg::SubmitResult(result));
                    });
                }
                true
            }
            Msg::SubmitResult(result) => {
                self.loading = false;
                match result {
                    Ok(_) => {
                        self.submit_error = None;
                        if let Some(on_done) = &props.on_done {
                            on_done.emit(());
                        }
                    }
                    Err(err) => {
                        self.submit_error = Some(err.to_string());
                    }
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let submit = ctx.link().callback(|_| Msg::Submit);

        let edit_mode = props.is_edit();

        let mut toolbar = Row::new()
            .padding(2)
            .gap(2)
            .class("pwt-bg-color-surface")
            .with_flex_spacer();

        if props.advanced_checkbox {
            let advanced_label_id = pwt::widget::get_unique_element_id();
            let advanced_field = Checkbox::new()
                .margin_start(1)
                .label_id(advanced_label_id.clone())
                .checked(*self.show_advanced)
                .on_change(ctx.link().callback(Msg::ShowAdvanced));

            let advanced = Row::new()
                .class("pwt-align-items-center")
                .with_child(html! {<label id={advanced_label_id}>{tr!("Advanced")}</label>})
                .with_child(advanced_field);

            toolbar.add_child(advanced);
        }

        if edit_mode {
            toolbar.add_child(ResetButton::new().on_reset(props.on_reset.clone()));

            if props.submit_digest {
                toolbar.add_child(Hidden::new().name("digest").submit_empty(false));
            }
        }

        let submit_text = match &props.submit_text {
            Some(submit_text) => submit_text.to_string(),
            None => {
                if edit_mode {
                    tr!("Update")
                } else {
                    tr!("Add")
                }
            }
        };
        toolbar.add_child(
            SubmitButton::new()
                .class("pwt-scheme-primary")
                .text(submit_text)
                .check_dirty(edit_mode)
                .on_submit(submit),
        );

        let renderer = props.renderer.clone();
        let loading = self.loading;

        let form = match &renderer {
            Some(renderer) => renderer.apply(&self.form_ctx),
            None => html! {},
        };

        let input_panel = Mask::new(
            Column::new()
                .class("pwt-flex-fit")
                .with_child(form)
                .with_child(toolbar.clone()),
        )
        .class("pwt-flex-fit")
        .visible(loading);

        let alert = self
            .submit_error
            .as_ref()
            .map(|msg| AlertDialog::new(msg).on_close(ctx.link().callback(|_| Msg::ClearError)));

        let on_close = {
            let on_close = props.on_close.clone();
            let on_done = props.on_done.clone();

            if on_close.is_some() || on_done.is_some() {
                Some(move |()| {
                    if let Some(on_close) = &on_close {
                        on_close.emit(());
                    }
                    if let Some(on_done) = &on_done {
                        on_done.emit(());
                    }
                })
            } else {
                None
            }
        };

        let load_err = self
            .load_error
            .as_ref()
            .map(|msg| AlertDialog::new(msg).on_close(on_close.clone()));

        Dialog::new(props.title.clone())
            .on_close(on_close)
            .draggable(props.draggable)
            .resizable(props.resizable)
            .auto_center(props.auto_center)
            .styles(props.styles.clone())
            .with_child(
                Form::new()
                    .class("pwt-flex-fit")
                    .form_context(self.form_ctx.clone())
                    .with_child(input_panel),
            )
            .with_optional_child(alert)
            .with_optional_child(load_err)
            .into()
    }
}

impl From<EditWindow> for VNode {
    fn from(val: EditWindow) -> Self {
        let key = val.key.clone();
        let comp = VComp::new::<PwtEditWindow>(Rc::new(val), key);
        VNode::from(comp)
    }
}
