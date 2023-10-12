use std::rc::Rc;

use anyhow::Error;
use pwt::state::PersistentState;
use serde_json::Value;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoLoadCallback, LoadCallback, RenderFn};
use pwt::widget::form::{
    Checkbox, Form, FormContext, IntoSubmitCallback, ResetButton, SubmitButton, SubmitCallback,
};
use pwt::widget::{AlertDialog, Column, Dialog, Mask, Row};

use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct EditWindow {
    /// Yew node ref
    #[prop_or_default]
    node_ref: NodeRef,

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
    #[builder_cb(IntoLoadCallback, into_load_callback, Value)]
    #[prop_or_default]
    pub loader: Option<LoadCallback<Value>>,

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
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub style: Option<AttrValue>,

    /// Close/Abort callback.
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_done: Option<Callback<()>>,

    /// Submit callback.
    #[prop_or_default]
    pub on_submit: Option<SubmitCallback>,

    /// Data change callback.
    #[builder_cb(IntoEventCallback, into_event_callback, FormContext)]
    #[prop_or_default]
    pub on_change: Option<Callback<FormContext>>,
}

impl EditWindow {
    pub fn new(title: impl Into<AttrValue>) -> Self {
        yew::props!(Self {
            title: title.into(),
        })
    }

    /// Builder style method to set the yew `node_ref`
    pub fn node_ref(mut self, node_ref: ::yew::html::NodeRef) -> Self {
        self.node_ref = node_ref;
        self
    }

    /// Builder style method to set the yew `key` property
    pub fn key(mut self, key: impl IntoOptionalKey) -> Self {
        self.key = key.into_optional_key();
        self
    }

    pub fn renderer(mut self, renderer: impl 'static + Fn(&FormContext) -> Html) -> Self {
        self.renderer = Some(RenderFn::new(renderer));
        self
    }

    pub fn on_submit(mut self, callback: impl IntoSubmitCallback) -> Self {
        self.on_submit = callback.into_submit_callback();
        self
    }

    pub fn is_edit(&self) -> bool {
        self.loader.is_some()
    }
}

pub enum Msg {
    FormDataChange,
    Submit,
    SubmitResult(Result<(), Error>),
    Load,
    LoadResult(Result<Value, Error>),
    ClearError,
    ShowAdvanced(bool),
}

#[doc(hidden)]
pub struct PwtEditWindow {
    loading: bool,
    form_ctx: FormContext,
    submit_error: Option<String>,
    show_advanced: PersistentState<bool>,
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
            show_advanced,
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
                true
            }
            Msg::Load => {
                if let Some(loader) = props.loader.clone() {
                    self.loading = true;
                    let link = ctx.link().clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let res = loader.apply().await;
                        link.send_message(Msg::LoadResult(res));
                    });
                }
                true
            }
            Msg::LoadResult(result) => {
                self.loading = false;
                match result {
                    Err(err) => log::error!("Load error: {}", err),
                    Ok(value) => {
                        self.form_ctx.load_form(value);
                    }
                }
                true
            }
            Msg::FormDataChange => {
                if self.submit_error != None {
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
                    wasm_bindgen_futures::spawn_local(async move {
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
                .class("pwt-ms-1")
                .label_id(advanced_label_id.clone())
                .checked(*self.show_advanced)
                .on_change(
                    ctx.link()
                        .callback(|value| Msg::ShowAdvanced(value == "on")),
                );

            let advanced = Row::new()
                .class("pwt-align-items-center")
                .with_child(html! {<label id={advanced_label_id}>{tr!("Advanced")}</label>})
                .with_child(advanced_field);

            toolbar.add_child(advanced);
        }

        if edit_mode {
            toolbar.add_child(ResetButton::new());
        }

        toolbar.add_child(
            SubmitButton::new()
                .class("pwt-scheme-primary")
                .text(if edit_mode { tr!("Update") } else { tr!("Add") })
                .check_dirty(edit_mode)
                .on_submit(submit),
        );

        let renderer = props.renderer.clone();
        let loading = self.loading;

        let form = match &renderer {
            Some(renderer) => renderer.apply(&self.form_ctx),
            None => html! {},
        };

        let input_panel = Mask::new(Column::new().with_child(form).with_child(toolbar.clone()))
            .class("pwt-flex-fit")
            .visible(loading);

        let alert = match self.submit_error.as_ref() {
            None => None,
            Some(msg) => {
                Some(AlertDialog::new(msg).on_close(ctx.link().callback(|_| Msg::ClearError)))
            }
        };

        Dialog::new(props.title.clone())
            .node_ref(props.node_ref.clone())
            .on_close(props.on_done.clone())
            .draggable(props.draggable)
            .resizable(props.resizable)
            .auto_center(props.auto_center)
            .style(props.style.clone())
            .with_child(
                Form::new()
                    .class("pwt-flex-fit")
                    .form_context(self.form_ctx.clone())
                    .with_child(input_panel),
            )
            .with_optional_child(alert)
            .into()
    }
}

impl Into<VNode> for EditWindow {
    fn into(self) -> VNode {
        let key = self.key.clone();
        let comp = VComp::new::<PwtEditWindow>(Rc::new(self), key);
        VNode::from(comp)
    }
}
