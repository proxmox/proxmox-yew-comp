use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use derivative::Derivative;
use indexmap::IndexMap;
use serde_json::Value;

use yew::html::IntoPropValue;
use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoLoadCallback, IntoSubmitCallback, LoadCallback, SubmitCallback};
use pwt::state::{SharedState, SharedStateObserver};
use pwt::widget::data_table::{DataTableKeyboardEvent, DataTableMouseEvent};
use pwt::widget::form::FormContext;
use pwt::widget::{Button, Toolbar};

use crate::{EditWindow, KVGrid, KVGridRow, LoadableComponentLink};
use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster};

use pwt_macros::builder;

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectGridCommand {
    Reload,
    StartTask(String, Option<Value>, bool),
}

#[derive(Clone, PartialEq)]
pub struct ObjectGridController {
    state: SharedState<Vec<ObjectGridCommand>>,
}

impl ObjectGridController {
    pub fn new() -> Self {
        Self {
            state: SharedState::new(Vec::new()),
        }
    }

    /// Reload the object grid
    pub fn reload(&self) {
        self.state.write().push(ObjectGridCommand::Reload);
    }
    pub fn start_task(&self, command_path: impl Into<String>, data: Option<Value>, short: bool) {
        self.state.write().push(ObjectGridCommand::StartTask(
            command_path.into(),
            data,
            short,
        ));
    }
}

pub trait IntoObjectGridController {
    fn into_object_grid_controller(self) -> Option<ObjectGridController>;
}

impl IntoObjectGridController for ObjectGridController {
    fn into_object_grid_controller(self) -> Option<ObjectGridController> {
        Some(self)
    }
}

impl IntoObjectGridController for Option<ObjectGridController> {
    fn into_object_grid_controller(self) -> Option<ObjectGridController> {
        self
    }
}

#[derive(Derivative)]
#[derivative(Clone, PartialEq)]
pub struct RenderObjectGridItemFn(
    #[derivative(PartialEq(compare_with = "Rc::ptr_eq"))]
    Rc<dyn Fn(&FormContext, &str, &Value, &Value) -> Html>,
);

impl RenderObjectGridItemFn {
    /// Creates a new [`RenderObjectGridItemFn`]
    pub fn new(renderer: impl 'static + Fn(&FormContext, &str, &Value, &Value) -> Html) -> Self {
        Self(Rc::new(renderer))
    }
}

#[derive(Clone, PartialEq)]
pub struct ObjectGridRow {
    row: KVGridRow,
    editor: Option<RenderObjectGridItemFn>,
}

impl ObjectGridRow {
    pub fn new(name: impl Into<String>, header: impl Into<String>) -> Self {
        Self {
            row: KVGridRow::new(name, header),
            editor: None,
        }
    }

    pub fn required(mut self, required: bool) -> Self {
        self.set_required(required);
        self
    }

    pub fn set_required(&mut self, required: bool) {
        self.row.set_required(required);
    }

    pub fn placeholder(mut self, placeholder: impl IntoPropValue<Option<String>>) -> Self {
        self.set_placeholder(placeholder);
        self
    }

    pub fn set_placeholder(&mut self, placeholder: impl IntoPropValue<Option<String>>) {
        self.row.set_placeholder(placeholder);
    }

    pub fn renderer(mut self, renderer: impl 'static + Fn(&str, &Value, &Value) -> Html) -> Self {
        self.set_renderer(renderer);
        self
    }

    pub fn set_renderer(&mut self, renderer: impl 'static + Fn(&str, &Value, &Value) -> Html) {
        self.row.set_renderer(renderer);
    }

    pub fn editor(
        mut self,
        editor: impl 'static + Fn(&FormContext, &str, &Value, &Value) -> Html,
    ) -> Self {
        self.editor = Some(RenderObjectGridItemFn::new(editor));
        self
    }
}

pub enum Msg {
    DataChange(Value),
    Select(Option<Key>),
    ControllerCommand(ObjectGridCommand),
}

#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct ObjectGrid {
    /// Yew key property.
    #[prop_or_default]
    pub key: Option<Key>,

    /// CSS class of the container.
    #[prop_or_default]
    pub class: Classes,

    /// Show edit button.
    #[builder]
    #[prop_or_default]
    pub editable: bool,

    #[builder]
    rows: Rc<Vec<ObjectGridRow>>,

    #[builder_cb(IntoLoadCallback, into_load_callback, Value)]
    #[prop_or_default]
    loader: Option<LoadCallback<Value>>,

    #[prop_or_default]
    data: Option<Value>,

    #[prop_or_default]
    on_submit: Option<SubmitCallback<FormContext>>,

    /// Tools (buttons) added to the toolbar.
    #[prop_or_default]
    pub tools: Vec<VNode>,

    #[prop_or_default]
    pub controller: Option<ObjectGridController>,
}

impl Into<VNode> for ObjectGrid {
    fn into(self) -> VNode {
        let key = self.key.clone();
        let comp = VComp::new::<LoadableComponentMaster<PwtObjectGrid>>(Rc::new(self), key);
        VNode::from(comp)
    }
}

impl ObjectGrid {
    pub fn new() -> Self {
        yew::props!(Self {
            rows: Rc::new(Vec::new()),
        })
    }

    /// Builder style method to set the yew `key` property
    pub fn key(mut self, key: impl IntoOptionalKey) -> Self {
        self.key = key.into_optional_key();
        self
    }

    /// Builder style method to add a html class.
    pub fn class(mut self, class: impl Into<Classes>) -> Self {
        self.add_class(class);
        self
    }

    /// Method to add a html class.
    pub fn add_class(&mut self, class: impl Into<Classes>) {
        self.class.push(class);
    }

    /// Builder style method to add a tool.
    pub fn with_tool(mut self, tool: impl Into<VNode>) -> Self {
        self.add_tool(tool);
        self
    }

    /// Method to add a tool.
    pub fn add_tool(&mut self, tool: impl Into<VNode>) {
        self.tools.push(tool.into());
    }

    pub fn on_submit(mut self, callback: impl IntoSubmitCallback<FormContext>) -> Self {
        self.on_submit = callback.into_submit_callback();
        self
    }

    pub fn controller(mut self, controller: impl IntoObjectGridController) -> Self {
        self.set_controller(controller);
        self
    }

    pub fn set_controller(&mut self, controller: impl IntoObjectGridController) {
        self.controller = controller.into_object_grid_controller();
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum ViewState {
    EditObject,
}

#[doc(hidden)]
pub struct PwtObjectGrid {
    selection: Option<Key>,
    data: Rc<Value>,

    rows: Rc<Vec<KVGridRow>>,
    editors: IndexMap<String, RenderObjectGridItemFn>,

    controller_observer: Option<SharedStateObserver<Vec<ObjectGridCommand>>>,
}

impl PwtObjectGrid {
    fn update_rows(&mut self, props: &ObjectGrid) {
        let mut rows = Vec::new();
        self.editors = IndexMap::new();
        for row in props.rows.iter() {
            if let Some(editor) = &row.editor {
                let name = row.row.name.clone();
                self.editors.insert(name, editor.clone());
            }
            rows.push(row.row.clone());
        }
        self.rows = Rc::new(rows);
    }

    fn update_controller(&mut self, props: &ObjectGrid, link: LoadableComponentLink<Self>) {
        match &props.controller {
            None => self.controller_observer = None,
            Some(controller) => {
                self.controller_observer = Some(controller.state.add_listener(
                    move |state: SharedState<Vec<ObjectGridCommand>>| {
                        let commands = {
                            let mut guard = state.write();
                            guard.notify = false;
                            guard.split_off(0)
                        };
                        for command in commands {
                            link.send_message(Msg::ControllerCommand(command));
                        }
                    },
                ));
            }
        }
    }

    fn edit_dialog(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let props = ctx.props();

        let name = match self.selection.as_ref() {
            Some(name) => name.to_string(),
            None => return None,
        };

        let row = match self.rows.iter().find(|row| row.name == name) {
            Some(row) => row,
            None => return None,
        };

        let title = &row.header;

        let data = self.data.clone();
        let value = data[&name].clone();

        let editor = match self.editors.get(&name) {
            Some(editor) => editor.clone(),
            None => return None,
        };

        Some(
            EditWindow::new(format!("Edit: {}", title))
                .loader(props.loader.clone())
                .on_done(ctx.link().change_view_callback(|_| None))
                .renderer(move |form_state| (editor.0)(&form_state, &name, &value, &data))
                .on_submit(props.on_submit.clone())
                .into(),
        )
    }
}

impl LoadableComponent for PwtObjectGrid {
    type Message = Msg;
    type Properties = ObjectGrid;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let props = ctx.props();

        ctx.link().repeated_load(3000);

        let mut me = Self {
            data: Rc::new(Value::Null),
            rows: Rc::new(Vec::new()),
            editors: IndexMap::new(),
            selection: None,
            controller_observer: None,
        };
        me.update_rows(props);
        me.update_controller(props, ctx.link());
        me
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let props = ctx.props();
        let loader = props.loader.clone();
        let link = ctx.link();

        Box::pin(async move {
            if let Some(loader) = &loader {
                let data: Value = loader.apply().await?;
                link.send_message(Msg::DataChange(data));
            }
            Ok(())
        })
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::DataChange(data) => {
                self.data = Rc::new(data);
                true
            }
            Msg::Select(opt_key) => {
                self.selection = opt_key;
                true
            }
            Msg::ControllerCommand(cmd) => match cmd {
                ObjectGridCommand::Reload => {
                    ctx.link().send_reload();
                    false
                }
                ObjectGridCommand::StartTask(command_path, data, short) => {
                    ctx.link().start_task(command_path, data, short);
                    false
                }
            },
        }
    }

    /*
    fn changed(&mut self, ctx: &LoadableComponentContext<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if !Rc::ptr_eq(&props.rows, &old_props.rows) {
            self.update_rows(props);
        }
        if props.controller != old_props.controller {
            self.update_controller(props, ctx.link());
        }
        true
    }
    */

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let props = ctx.props();

        let disable_edit = if let Some(key) = &self.selection {
            let name: &str = &*key;
            !self.editors.contains_key(name)
        } else {
            true
        };

        let show_toolbar = props.editable || !props.tools.is_empty();

        if !show_toolbar {
            return None;
        }

        let mut toolbar = Toolbar::new()
            .border_bottom(true)
            .with_child(Button::new("Edit").disabled(disable_edit).onclick({
                let link = ctx.link();
                move |_| {
                    link.change_view(Some(ViewState::EditObject));
                }
            }))
            .with_flex_spacer();

        for tool in &props.tools {
            toolbar.add_child(tool.clone());
        }

        Some(toolbar.into())
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        KVGrid::new()
            .class("pwt-flex-fit")
            .rows(Rc::clone(&self.rows))
            .data(self.data.clone())
            .on_select(ctx.link().callback(|key| Msg::Select(key)))
            .on_row_dblclick({
                let link = ctx.link().clone();
                move |event: &mut DataTableMouseEvent| {
                    link.send_message(Msg::Select(Some(event.record_key.clone())));
                    link.change_view(Some(ViewState::EditObject));
                }
            })
            .on_row_keydown({
                let link = ctx.link().clone();
                move |event: &mut DataTableKeyboardEvent| {
                    if event.key() == " " {
                        link.send_message(Msg::Select(Some(event.record_key.clone())));
                        link.change_view(Some(ViewState::EditObject));
                    }
                }
            })
            .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        match view_state {
            ViewState::EditObject => self.edit_dialog(ctx),
        }
    }
}
