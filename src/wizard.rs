use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use anyhow::Error;
use derivative::Derivative;
use html::Scope;
use indexmap::IndexMap;
use serde_json::{json, Value};

use pwt::css::{Flex, Overflow};
use pwt::props::RenderFn;
use pwt::{prelude::*, AsyncPool};

use pwt::css::ColorScheme;
use pwt::props::{AsCssStylesMut, ContainerBuilder, CssStyles};
use pwt::state::Selection;
use pwt::widget::form::{Form, FormContext};
use pwt::widget::{
    AlertDialog, Button, Container, Dialog, Input, Mask, MiniScrollMode, Row, TabBarItem,
    TabBarStyle, TabPanel,
};

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{Key, VComp, VNode};
use yew::{Callback, Component, Html, Properties};

use super::{IntoSubmitValueCallback, SubmitValueCallback};

use pwt_macros::builder;

/// Infos passed to the [pwt::widget::SelectionView] render function.
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

    /// Sets a callback that will be called when a later page wants to be selected
    /// (e.g. by the next button)
    ///
    /// The callback should return true when that's allowed, false otherwise.
    /// When the callback returns false, the page should handle navigating
    /// to the next page by itself.
    ///
    /// This is useful for panels in a wizard that act like a form that
    /// has to be submitted before navigating to the next page.
    pub fn on_next(&self, callback: impl Into<Callback<(), bool>>) {
        let mut controller = self.controller.write();
        controller
            .submit_callbacks
            .insert(self.key.clone(), callback.into());
    }

    /// Navigates the wizard to the next page (if possible)
    ///
    /// Note that callbacks setup with `on_next` will not be called,
    /// otherwise it could lead to an infinite loop easily.
    pub fn go_to_next_page(&self) {
        let controller = self.controller.write();
        let Some(current_idx) = controller.get_current_index() else {
            return;
        };
        let Some(next_page) = controller.page_list.get(current_idx + 1) else {
            return;
        };
        controller
            .link
            .send_message(Msg::SelectPage(next_page.clone(), false));
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

    /// Use [pwt::widget::MiniScroll] for [pwt::widget::TabBar] to allow
    /// scrolling.
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
    pages_valid: HashSet<Key>,
    pages_lock: HashSet<Key>,
    submit_callbacks: HashMap<Key, Callback<(), bool>>,
}

impl WizardState {
    /// Returns the index of the page from the given [`Key`] if that exists
    pub fn get_index(&self, key: &Key) -> Option<usize> {
        self.page_list.iter().position(|page_key| *page_key == *key)
    }

    /// Returns the index of the current page if any.
    pub fn get_current_index(&self) -> Option<usize> {
        self.page.as_ref().and_then(|key| self.get_index(key))
    }

    /// Returns the callback of the page from the given [`Key`] if that exists
    pub fn get_callback(&self, key: Option<&Key>) -> Option<Callback<(), bool>> {
        key.and_then(|key| self.submit_callbacks.get(key).cloned())
    }

    /// Returns if the page for the given [`Key`] is valid
    pub fn page_valid(&self, key: &Key) -> bool {
        self.pages_valid.contains(key)
    }

    /// Returns if the page for the given [`Key`] is locked
    pub fn page_locked(&self, key: &Key) -> bool {
        self.pages_lock.contains(key)
    }

    fn can_progress(&self) -> bool {
        let mut next_enabled = true;
        let last_page = self.page_list.last().cloned();
        for i in 0..=self.get_current_index().unwrap_or(0) {
            match self.page_list.get(i) {
                None => {
                    next_enabled = false;
                    break;
                }
                Some(key) => {
                    if !self.page_valid(key) && Some(key) != last_page.as_ref() {
                        next_enabled = false;
                        break;
                    }
                    if self.page_locked(key) {
                        next_enabled = false;
                        break;
                    }
                }
            }
        }
        next_enabled
    }
}

impl WizardController {
    fn new(link: Scope<PwtWizard>) -> Self {
        let state = WizardState {
            link,
            page: None,
            page_data: HashMap::new(),
            page_list: Vec::new(),
            pages_valid: HashSet::new(),
            pages_lock: HashSet::new(),
            submit_callbacks: HashMap::new(),
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
    valid_data: Rc<Value>,

    controller: WizardController,
    async_pool: AsyncPool,
}

pub enum Msg {
    PageLock(Key, bool),   // disable/enable next button
    SelectPage(Key, bool), // call optional callback
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
            selection,
            valid_data: Rc::new(json!({})),
            controller,
            async_pool: AsyncPool::new(),
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::SelectPage(page, use_callback) => {
                let mut state = self.controller.write();

                if use_callback {
                    let cur_idx = state.get_current_index();
                    let target_idx = state.get_index(&page);

                    match (cur_idx, target_idx) {
                        (Some(cur), Some(target)) if target > cur => {
                            // we selected a later page
                            if let Some(callback) = state.get_callback(state.page.as_ref()) {
                                if !callback.emit(()) {
                                    self.selection.select(state.page.clone().unwrap());
                                    return true;
                                }
                            }
                        }
                        _ => {}
                    }
                }

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
                    return <Self as yew::Component>::update(
                        self,
                        ctx,
                        Msg::SelectPage(selected_key, true),
                    );
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
                    self.async_pool.spawn(async move {
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
            .tab_bar_style(props.tab_bar_style)
            .selection(self.selection.clone());

        let state = self.controller.read();

        let mut disabled = false;
        for (page_num, (key, page)) in props.pages.iter().enumerate() {
            let active = Some(key) == state.page.as_ref();
            let form_ctx = state.page_data.get(key).unwrap();

            let page_content = page.renderer.apply(&WizardPageRenderInfo {
                key: key.clone(),
                visible: active,
                form_ctx: form_ctx.clone(),
                valid_data: Rc::clone(&self.valid_data),
                controller: self.controller.clone(),
            });

            let next_page = props
                .pages
                .get_index(page_num + 1)
                .map(|(key, _)| key.clone());

            let page_content = Form::new()
                .class(Overflow::Auto)
                .class(Flex::Fill)
                .form_context(form_ctx.clone())
                .onsubmit(ctx.link().batch_callback({
                    let state = self.controller.clone();
                    move |_| {
                        if !state.read().can_progress() {
                            return None;
                        }
                        if let Some(page) = next_page.clone() {
                            Some(Msg::SelectPage(page, true))
                        } else {
                            Some(Msg::Submit)
                        }
                    }
                }))
                .with_child(page_content)
                .with_child(Input::new().attribute("type", "submit").class("pwt-d-none"));

            let tab_bar_item = page.tab_bar_item.clone().disabled(disabled);

            if !disabled {
                if !self.controller.read().page_valid(key) {
                    disabled = true;
                }
                if self.controller.read().page_locked(key) {
                    disabled = true;
                }
            }

            tab_panel.add_item(tab_bar_item, page_content);
        }

        let tab_panel = Mask::new(tab_panel).visible(self.loading);

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
        let mut state = self.controller.write();
        if valid {
            state.pages_valid.insert(page.clone());
        } else {
            state.pages_valid.remove(page);
        }
    }

    fn change_page_lock(&mut self, page: &Key, lock: bool) {
        let mut state = self.controller.write();
        if lock {
            state.pages_lock.insert(page.clone());
        } else {
            state.pages_lock.remove(page);
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

        let next_is_enabled = !self.loading && state.can_progress();

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
                Button::new(tr!("Back")).disabled(self.loading).onclick({
                    let link = ctx.link().clone();
                    let prev_page = prev_page.clone();
                    move |_| {
                        if let Some(prev_page) = &prev_page {
                            link.send_message(Msg::SelectPage(prev_page.clone(), false));
                        }
                    }
                })
            }))
            .with_child(
                Button::new(next_button_text)
                    .class(ColorScheme::Primary)
                    .disabled(!next_is_enabled)
                    .onclick({
                        let link = ctx.link().clone();
                        let next_page = next_page.clone();
                        move |_| {
                            if let Some(next_page) = &next_page {
                                link.send_message(Msg::SelectPage(next_page.clone(), true));
                            } else {
                                link.send_message(Msg::Submit);
                            }
                        }
                    }),
            )
    }
}

impl From<Wizard> for VNode {
    fn from(val: Wizard) -> Self {
        let key = val.key.clone();
        let comp = VComp::new::<PwtWizard>(Rc::new(val), key);
        VNode::from(comp)
    }
}
