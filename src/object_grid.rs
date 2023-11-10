use std::rc::Rc;

use derivative::Derivative;
use indexmap::IndexMap;
use serde_json::Value;

use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};
use yew::html::IntoPropValue;

use pwt::prelude::*;
use pwt::props::{IntoLoadCallback, LoadCallback};
use pwt::state::Loader;
use pwt::widget::data_table::{DataTableKeyboardEvent, DataTableMouseEvent};
use pwt::widget::form::{FormContext, IntoSubmitCallback, SubmitCallback};
use pwt::widget::{Button, Toolbar, Column};

use crate::{EditWindow, KVGrid, KVGridRow};

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
    DataChange,
    Select(Option<Key>),
    Edit,
    Close,
}

#[derive(Properties, PartialEq, Clone)]
pub struct ObjectGrid {
    /// Yew key property.
    #[prop_or_default]
    pub key: Option<Key>,

    /// CSS class of the container.
    #[prop_or_default]
    pub class: Classes,

    #[prop_or_default]
    pub editable: bool,

    rows: Rc<Vec<ObjectGridRow>>,

    #[prop_or_default]
    loader: Option<LoadCallback<Value>>,

    #[prop_or_default]
    data: Option<Value>,

    #[prop_or_default]
    on_submit: Option<SubmitCallback>,
}

impl Into<VNode> for ObjectGrid {
    fn into(self) -> VNode {
        let key = self.key.clone();
        let comp = VComp::new::<PwtObjectGrid>(Rc::new(self), key);
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

    pub fn loader(mut self, callback: impl IntoLoadCallback<Value>) -> Self {
        self.loader = callback.into_load_callback();
        self
    }

    pub fn on_submit(mut self, callback: impl IntoSubmitCallback) -> Self {
        self.on_submit = callback.into_submit_callback();
        self
    }

    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    pub fn rows(mut self, rows: Rc<Vec<ObjectGridRow>>) -> Self {
        self.set_rows(rows);
        self
    }

    pub fn set_rows(&mut self, rows: Rc<Vec<ObjectGridRow>>) {
        self.rows = rows;
    }
}

#[doc(hidden)]
pub struct PwtObjectGrid {
    loader: Loader<Value>,
    selection: Option<Key>,
    show_dialog: bool,

    rows: Rc<Vec<KVGridRow>>,
    editors: IndexMap<String, RenderObjectGridItemFn>,
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

    fn data(&self) -> Value {
        match &self.loader.read().data {
            Some(Ok(data)) => data.as_ref().clone(),
            _ => Value::Null,
        }
    }

    fn edit_dialog(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let name = self.selection.as_ref().unwrap().to_string();

        let row = self.rows.iter().find(|row| row.name == name).unwrap();

        let title = &row.header;

        let data = self.data();
        let value = data[&name].clone();

        let editor = self.editors.get(&name).unwrap().clone();

        EditWindow::new(format!("Edit: {}", title))
            .loader(props.loader.clone())
            .on_done(Some(ctx.link().callback(|_| Msg::Close)))
            .renderer(move |form_state| (editor.0)(&form_state, &name, &value, &data))
            .on_submit(props.on_submit.clone())
            .into()
    }
}

impl Component for PwtObjectGrid {
    type Message = Msg;
    type Properties = ObjectGrid;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let loader = Loader::new()
            .loader(props.loader.clone())
            .on_change(ctx.link().callback(|_| Msg::DataChange));

        loader.load();

        let mut me = Self {
            rows: Rc::new(Vec::new()),
            editors: IndexMap::new(),
            loader,
            selection: None,
            show_dialog: false,
        };
        me.update_rows(props);
        me
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::DataChange => true,
            Msg::Edit => {
                self.show_dialog = true;
                true
            }
            Msg::Close => {
                self.show_dialog = false;
                self.loader.load();
                true
            }
            Msg::Select(opt_key) => {
                self.selection = opt_key;
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if !Rc::ptr_eq(&props.rows, &old_props.rows) {
            self.update_rows(props);
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let main_view = self.loader.render(|data| self.main_view(ctx, data));

        let disable_edit = if let Some(key) = &self.selection {
            let name: &str = &*key;
            !self.editors.contains_key(name)
        } else {
            true
        };

        let toolbar = props.editable.then(|| self.toolbar(ctx, disable_edit));
        let dialog = self.show_dialog.then(|| self.edit_dialog(ctx));

        Column::new()
            .with_optional_child(toolbar)
            .with_child(main_view)
            .with_optional_child(dialog)
            .into()
    }
}

impl PwtObjectGrid {
    fn toolbar(&self, ctx: &Context<Self>, disable_edit: bool) -> Html {
        Toolbar::new()
            .with_child(
                Button::new("Edit")
                    .disabled(disable_edit)
                    .onclick(ctx.link().callback(|_| Msg::Edit)),
            )
            .with_flex_spacer()
            .with_child(self.loader.reload_button())
            .into()
    }

    fn main_view(&self, ctx: &Context<Self>, data: Rc<Value>) -> Html {
        let props = ctx.props();

        KVGrid::new()
            .class(props.class.clone())
            .rows(Rc::clone(&self.rows))
            .data(data)
            .on_select(ctx.link().callback(|key| Msg::Select(key)))
            .on_row_dblclick({
                let link = ctx.link().clone();
                move |event: &mut DataTableMouseEvent| {
                    link.send_message(Msg::Select(Some(event.record_key.clone())));
                    link.send_message(Msg::Edit);
                }
            })
            .on_row_keydown({
                let link = ctx.link().clone();
                move |event: &mut DataTableKeyboardEvent| {
                    if event.key() == " " {
                        link.send_message(Msg::Select(Some(event.record_key.clone())));
                        link.send_message(Msg::Edit);
                    }
                }
            })
            .into()
    }
}
