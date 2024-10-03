use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use anyhow::Error;
use derivative::Derivative;
use html::Scope;
use indexmap::IndexMap;
use serde_json::{json, Value};

use pwt::css::{Flex, Overflow};
use pwt::prelude::*;
use pwt::props::RenderFn;

use pwt::css::ColorScheme;
use pwt::props::{AsCssStylesMut, ContainerBuilder, CssStyles};
use pwt::state::Selection;
use pwt::widget::form::{Form, FormContext};
use pwt::widget::{
    AlertDialog, Button, Container, Dialog, MiniScrollMode, Row, TabBarItem, TabBarStyle, TabPanel,
};

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{Key, VComp, VNode};
use yew::{Callback, Component, Html, Properties};

use super::{IntoSubmitValueCallback, SubmitValueCallback};

use pwt_macros::builder;

/// Infos passed to the [SelectionView] render function.
#[derive(Clone, PartialEq)]
pub struct WizardPageRenderInfo {
    /// The key of the item to render
    pub key: Key,

    /// Set if this item is visible/active.
    ///
    /// So that the item can react on visibility changes.
    pub visible: bool,

    /// The [FormContext] of the current page
    pub form_ctx: FormContext,

    /// Submit data from all forms.
    ///
    /// Note: Merged into a single json object.
    pub valid_data: Rc<Value>,

    controller: WizardController,
}

impl WizardPageRenderInfo {
    /// Allow access to the [FormContext] of other pages.
    pub fn lookup_form_context(&self, key: &Key) -> Option<FormContext> {
        self.controller.read().page_data.get(key).cloned()
    }

    /// Disable/Enable the next button.
    pub fn page_lock(&self, lock: bool) {
        self.controller
            .read()
            .link
            .send_message(Msg::PageLock(self.key.clone(), lock));
    }

    /// Resets the valid pages state for all pages after the current one.
    ///
    /// Useful for pages that want to reset the state of the remaining wizard.
    pub fn reset_remaining_valid_pages(&self) {
        let mut reset = false;
        let controller = self.controller.write();
        for page in controller.page_list.iter() {
            if reset {
                controller
                    .link
                    .send_message(Msg::ChangeValid(page.clone(), false));
            }
            if *page == self.key {
                reset = true;
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct PageConfig {
    tab_bar_item: TabBarItem,
    renderer: RenderFn<WizardPageRenderInfo>,
}

#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct Wizard {
    /// The yew component key.
    #[prop_or_default]
    pub key: Option<Key>,

    /// Dialog Title (also used as 'arial-label')
    pub title: AttrValue,

    /// Title as Html
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub html_title: Option<Html>,

    /// Use [MiniScroll] for [TabBar] to allow scrolling.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub scroll_mode: Option<MiniScrollMode>,

    /// The [TabBarStyle]
    #[prop_or_default]
    #[builder]
    pub tab_bar_style: TabBarStyle,

    /// CSS style for the dialog window
    #[prop_or_default]
    pub styles: CssStyles,

    /// Dialog close/abort callback.
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    on_close: Option<Callback<()>>,

    /// Done callback, called after Close, Abort or Submit.
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_done: Option<Callback<()>>,

    /// Submit callback.
    #[prop_or_default]
    pub on_submit: Option<SubmitValueCallback>,

    /// Wizard page render functions.
    #[prop_or_default]
    pages: IndexMap<Key, PageConfig>,

    /// Submit button text.
    ///
    /// Default is 'Finish'.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub submit_text: Option<AttrValue>,

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
}

impl AsCssStylesMut for Wizard {
    fn as_css_styles_mut(&mut self) -> &mut CssStyles {
        &mut self.styles
    }
}

impl WidgetStyleBuilder for Wizard {}

impl Wizard {
    /// Create a new instance.
    pub fn new(title: impl Into<AttrValue>) -> Self {
        yew::props!(Self {
            title: title.into(),
        })
    }

    /// Builder style method to set the yew `key` property
    pub fn key(mut self, key: impl IntoOptionalKey) -> Self {
        self.key = key.into_optional_key();
        self
    }

    /// Method to add a wizard page.
    pub fn with_page(
        mut self,
        item: impl Into<TabBarItem>,
        renderer: impl 'static + Fn(&WizardPageRenderInfo) -> Html,
    ) -> Self {
        let mut item = item.into();

        if item.key.is_none() {
            item.key = Some(Key::from(format!("__wizard_page{}", self.pages.len())));
        }

        let key = item.key.clone().unwrap();

        let page = PageConfig {
            renderer: RenderFn::new(renderer),
            tab_bar_item: item,
        };

        self.pages.insert(key, page);
        self
    }

    pub fn on_submit(mut self, callback: impl IntoSubmitValueCallback) -> Self {
        self.on_submit = callback.into_submit_value_callback();
        self
    }
}

#[derive(Derivative)]
#[derivative(Clone(bound = ""), PartialEq(bound = ""))]
pub struct WizardController {
    #[derivative(PartialEq(compare_with = "Rc::ptr_eq"))]
    state: Rc<RefCell<WizardState>>,
}

struct WizardState {
    link: Scope<PwtWizard>,
    page: Option<Key>,
    page_data: HashMap<Key, FormContext>,
    page_list: Vec<Key>,
}

impl WizardController {
    fn new(link: Scope<PwtWizard>) -> Self {
        let state = WizardState {
            link,
            page: None,
            page_data: HashMap::new(),
            page_list: Vec::new(),
        };
        Self {
            state: Rc::new(RefCell::new(state)),
        }
    }

    fn read(&self) -> Ref<'_, WizardState> {
        self.state.borrow()
    }

    fn write(&self) -> RefMut<'_, WizardState> {
        self.state.borrow_mut()
    }

    fn insert_page(&self, key: &Key) {
        let mut state = self.write();
        state.page_list.push(key.clone());
        let form_ctx = FormContext::new().on_change(state.link.callback({
            let key = key.clone();
            move |form_ctx: FormContext| Msg::ChangeValid(key.clone(), form_ctx.read().is_valid())
        }));
        state.page_data.insert(key.clone(), form_ctx);
        if state.page.is_none() {
            state.page = Some(key.clone());
        }
    }
}
pub struct PwtWizard {
    selection: Selection,
    loading: bool, // set during submit
    submit_error: Option<String>,
    pages_valid: HashSet<Key>,
    pages_lock: HashSet<Key>,
    valid_data: Rc<Value>,

    controller: WizardController,
}

pub enum Msg {
    PageLock(Key, bool), // disable/enable next button
    SelectPage(Key),
    ChangeValid(Key, bool),
    SelectionChange(Selection),
    CloseDialog,
    Submit,
    SubmitResult(Result<(), Error>),
    ClearError,
}

impl Component for PwtWizard {
    type Message = Msg;

    type Properties = Wizard;

    fn create(ctx: &yew::Context<Self>) -> Self {
        let props = ctx.props();

        let selection = Selection::new().on_select(ctx.link().callback(Msg::SelectionChange));

        let controller = WizardController::new(ctx.link().clone());

        for (key, _) in props.pages.iter() {
            controller.insert_page(key);
        }

        Self {
            loading: false,
            submit_error: None,
            pages_valid: HashSet::new(),
            pages_lock: HashSet::new(),
            selection,
            valid_data: Rc::new(json!({})),
            controller,
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::SelectPage(page) => {
                let mut state = self.controller.write();
                self.selection.select(page.clone());
                state.page = Some(page.clone());

                if let Some(form_ctx) = state.page_data.get(&page) {
                    let valid = form_ctx.read().is_valid();
                    drop(state);
                    self.change_page_valid(&page, valid);
                } else {
                    drop(state);
                }
                self.update_valid_data(ctx);
            }
            Msg::ChangeValid(page, valid) => {
                self.change_page_valid(&page, valid);
                self.update_valid_data(ctx);
            }
            Msg::SelectionChange(selection) => {
                if let Some(selected_key) = selection.selected_key() {
                    let mut state = self.controller.write();
                    state.page = Some(selected_key);
                }
            }
            Msg::CloseDialog => {
                if let Some(on_close) = &props.on_close {
                    on_close.emit(());
                }
                if let Some(on_done) = &props.on_done {
                    on_done.emit(());
                }
            }
            Msg::Submit => {
                if let Some(on_submit) = props.on_submit.clone() {
                    let link = ctx.link().clone();
                    let data = self.valid_data.as_ref().clone();
                    self.loading = true;
                    wasm_bindgen_futures::spawn_local(async move {
                        let result = on_submit.apply(data).await;
                        link.send_message(Msg::SubmitResult(result));
                    });
                }
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
            }
            Msg::ClearError => {
                self.submit_error = None;
            }
            Msg::PageLock(page, lock) => {
                self.change_page_lock(&page, lock);
            }
        }
        true
    }

    fn view(&self, ctx: &yew::Context<Self>) -> yew::Html {
        let props = ctx.props();

        let mut tab_panel = TabPanel::new()
            .class(Overflow::Auto)
            .class(Flex::Fill)
            .tab_bar_style(props.tab_bar_style.clone())
            .selection(self.selection.clone());

        let state = self.controller.read();

        let mut disabled = false;
        for (key, page) in props.pages.iter() {
            let active = Some(key) == state.page.as_ref();
            let form_ctx = state.page_data.get(key).unwrap();

            let page_content = page.renderer.apply(&WizardPageRenderInfo {
                key: key.clone(),
                visible: active,
                form_ctx: form_ctx.clone(),
                valid_data: Rc::clone(&self.valid_data),
                controller: self.controller.clone(),
            });

            let page_content = Form::new()
                .class(Overflow::Auto)
                .class(Flex::Fill)
                .form_context(form_ctx.clone())
                .with_child(page_content);

            let tab_bar_item = page.tab_bar_item.clone().disabled(disabled);

            if !disabled {
                if !self.pages_valid.contains(&key) {
                    disabled = true;
                }
                if self.pages_lock.contains(&key) {
                    disabled = true;
                }
            }

            tab_panel.add_item(tab_bar_item, page_content);
        }

        Container::new()
            .with_child(
                Dialog::new(props.title.clone())
                    .html_title(props.html_title.clone())
                    .styles(props.styles.clone())
                    .draggable(props.draggable)
                    .resizable(props.resizable)
                    .auto_center(props.auto_center)
                    .on_close(ctx.link().callback(|_| Msg::CloseDialog))
                    .with_child(tab_panel)
                    .with_child(self.create_bottom_bar(ctx)),
            )
            .with_optional_child(self.submit_error.as_deref().map(|err| {
                AlertDialog::new(err).on_close(ctx.link().callback(|_| Msg::ClearError))
            }))
            .into()
    }
}

impl PwtWizard {
    fn change_page_valid(&mut self, page: &Key, valid: bool) {
        if valid {
            self.pages_valid.insert(page.clone());
        } else {
            self.pages_valid.remove(page);
        }
    }

    fn change_page_lock(&mut self, page: &Key, lock: bool) {
        if lock {
            self.pages_lock.insert(page.clone());
        } else {
            self.pages_lock.remove(page);
        }
    }

    fn update_valid_data(&mut self, ctx: &yew::Context<Self>) {
        let props = ctx.props();

        let state = self.controller.read();

        let mut valid_data = serde_json::Map::new();
        for (key, _) in props.pages.iter() {
            if let Some(form_ctx) = state.page_data.get(key) {
                let mut data = form_ctx.read().get_submit_data();
                valid_data.append(data.as_object_mut().unwrap());
            }
            if Some(key) == state.page.as_ref() {
                break;
            }
        }

        self.valid_data = Rc::new(Value::Object(valid_data));
    }

    fn create_bottom_bar(&self, ctx: &yew::Context<Self>) -> Row {
        let props = ctx.props();

        let state = self.controller.read();

        let first_page = props.pages.first().map(|(key, _value)| key.clone());

        let is_first = match &state.page {
            None => true,
            Some(key) => Some(key) == first_page.as_ref(),
        };

        let last_page = props.pages.last().map(|(key, _value)| key.clone());

        let is_last = match &state.page {
            None => false,
            Some(key) => Some(key) == last_page.as_ref(),
        };

        let page_num = match &state.page {
            None => 0,
            Some(key) => props.pages.get_index_of(key).unwrap_or(0),
        };

        let mut next_is_disabled = false;
        for i in 0..=page_num {
            match props.pages.get_index(i) {
                None => {
                    next_is_disabled = true;
                    break;
                }
                Some((key, _)) => {
                    if !self.pages_valid.contains(key) && Some(key) != last_page.as_ref() {
                        next_is_disabled = true;
                        break;
                    }
                    if self.pages_lock.contains(key) {
                        next_is_disabled = true;
                        break;
                    }
                }
            }
        }

        let next_page = props
            .pages
            .get_index(page_num + 1)
            .map(|(key, _)| key.clone());

        let prev_page = props
            .pages
            .get_index(page_num.saturating_sub(1))
            .map(|(key, _)| key.clone());

        let next_button_text = if is_last {
            props
                .submit_text
                .as_ref()
                .map(|text| text.to_string())
                .unwrap_or_else(|| tr!("Finish"))
        } else {
            tr!("Next")
        };

        Row::new()
            .padding(2)
            .gap(2)
            .with_flex_spacer()
            .class(ColorScheme::Surface)
            .class("pwt-panel-header")
            .with_optional_child((!is_first).then(|| {
                Button::new(tr!("Back")).onclick({
                    let link = ctx.link().clone();
                    let prev_page = prev_page.clone();
                    move |_| {
                        if let Some(prev_page) = &prev_page {
                            link.send_message(Msg::SelectPage(prev_page.clone()));
                        }
                    }
                })
            }))
            .with_child(
                Button::new(next_button_text)
                    .class(ColorScheme::Primary)
                    .disabled(next_is_disabled)
                    .onclick({
                        let link = ctx.link().clone();
                        let next_page = next_page.clone();
                        move |_| {
                            if let Some(next_page) = &next_page {
                                link.send_message(Msg::SelectPage(next_page.clone()));
                            } else {
                                link.send_message(Msg::Submit);
                            }
                        }
                    }),
            )
    }
}

impl Into<VNode> for Wizard {
    fn into(self) -> VNode {
        let key = self.key.clone();
        let comp = VComp::new::<PwtWizard>(Rc::new(self), key);
        VNode::from(comp)
    }
}
