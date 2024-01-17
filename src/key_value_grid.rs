use std::rc::Rc;

use derivative::Derivative;
use indexmap::IndexMap;
use serde_json::Value;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::{CallbackMut, ExtractPrimaryKey, IntoEventCallbackMut};
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader, DataTableKeyboardEvent, DataTableMouseEvent,
};

use pwt_macros::builder;

/// For use with KVGrid
#[derive(Derivative)]
#[derivative(Clone, PartialEq)]
pub struct RenderKVGridRecordFn(
    #[derivative(PartialEq(compare_with = "Rc::ptr_eq"))] Rc<dyn Fn(&str, &Value, &Value) -> Html>,
);

impl RenderKVGridRecordFn {
    /// Creates a new [`RenderKVGridRecordFn`]
    pub fn new(renderer: impl 'static + Fn(&str, &Value, &Value) -> Html) -> Self {
        Self(Rc::new(renderer))
    }
}

#[derive(Clone, PartialEq)]
#[builder]
pub struct KVGridRow {
    pub name: String,
    pub header: String,
    #[builder]
    pub required: bool,
    #[builder(IntoPropValue, into_prop_value)]
    pub placeholder: Option<String>,
    pub renderer: Option<RenderKVGridRecordFn>,
}

impl KVGridRow {
    pub fn new(name: impl Into<String>, header: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            header: header.into(),
            required: false,
            placeholder: None,
            renderer: None,
        }
    }

    pub fn renderer(mut self, renderer: impl 'static + Fn(&str, &Value, &Value) -> Html) -> Self {
        self.set_renderer(renderer);
        self
    }

    pub fn set_renderer(&mut self, renderer: impl 'static + Fn(&str, &Value, &Value) -> Html) {
        self.renderer = Some(RenderKVGridRecordFn::new(renderer));
    }
}

#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct KVGrid {
    /// Yew key property.
    #[prop_or_default]
    pub key: Option<Key>,

    /// CSS class of the container.
    #[prop_or_default]
    pub class: Classes,

    /// Set class for table cells (default is "pwt-datatable-cell").
    #[prop_or_default]
    pub cell_class: Classes,

    /// Disable horizontal borders.
    #[prop_or_default]
    #[builder]
    pub borderless: bool,

    /// Use a striped color scheme for rows.
    #[prop_or(true)]
    #[builder]
    pub striped: bool,

    rows: Rc<Vec<KVGridRow>>,
    data: Rc<Value>,
    /// Select callback.
    #[prop_or_default]
    #[builder_cb(IntoEventCallback, into_event_callback, Option<Key>)]
    pub on_select: Option<Callback<Option<Key>>>,

    /// Row click callback.
    #[prop_or_default]
    #[builder_cb(IntoEventCallbackMut, into_event_cb_mut, DataTableMouseEvent)]
    pub on_row_click: Option<CallbackMut<DataTableMouseEvent>>,

    /// Row double click callback.
    #[prop_or_default]
    #[builder_cb(IntoEventCallbackMut, into_event_cb_mut, DataTableMouseEvent)]
    pub on_row_dblclick: Option<CallbackMut<DataTableMouseEvent>>,

    /// Row keydown callback.
    #[prop_or_default]
    #[builder_cb(IntoEventCallbackMut, into_event_cb_mut, DataTableKeyboardEvent)]
    pub on_row_keydown: Option<CallbackMut<DataTableKeyboardEvent>>,
}

impl KVGrid {
    pub fn new() -> Self {
        yew::props!(Self {
            rows: Rc::new(Vec::new()),
            data: Rc::new(Value::Null),
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

    /// Builder style method to add a html class for table cells.
    pub fn cell_class(mut self, class: impl Into<Classes>) -> Self {
        self.add_cell_class(class);
        self
    }

    /// Method to add a html class for table cells.
    pub fn add_cell_class(&mut self, class: impl Into<Classes>) {
        self.cell_class.push(class);
    }

    pub fn data(mut self, data: Rc<Value>) -> Self {
        self.set_data(data);
        self
    }

    pub fn set_data(&mut self, data: Rc<Value>) {
        self.data = data;
    }

    pub fn rows(mut self, rows: Rc<Vec<KVGridRow>>) -> Self {
        self.set_rows(rows);
        self
    }

    pub fn set_rows(&mut self, rows: Rc<Vec<KVGridRow>>) {
        self.rows = rows;
    }

    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.get_row(name).map(|row| row.header.as_str())
    }

    pub fn get_row(&self, name: &str) -> Option<&KVGridRow> {
        // fixme: replace with somthing faster
        self.rows.iter().find(|row| row.name == name)
    }
}

#[derive(Clone, PartialEq)]
struct KVGridRecord {
    row: Rc<KVGridRow>,
    value: Value,
    store: Rc<Value>,
}

impl ExtractPrimaryKey for KVGridRecord {
    fn extract_key(&self) -> Key {
        Key::from(self.row.name.as_str())
    }
}

#[doc(hidden)]
pub struct PwtKVGrid {
    rows: Rc<IndexMap<String, Rc<KVGridRow>>>,
    store: Store<KVGridRecord>,
    selection: Selection,
}

thread_local! {
    static COLUMNS: Rc<Vec<DataTableHeader<KVGridRecord>>> = Rc::new(vec![
        DataTableColumn::new("Key")
            .show_menu(false)
            .render(|record: &KVGridRecord| html!{record.row.header.clone()})
            .into(),
        DataTableColumn::new("Value")
            .width("100%")
            .show_menu(false)
            .render(|record: &KVGridRecord|  {
                match &record.row.renderer {
                    Some(renderer) => (renderer.0)(&record.row.name, &record.value, &record.store),
                    None => render_value(&record.value),
                }
            })
            .into(),
    ]);
}

impl PwtKVGrid {
    fn data_update(&mut self, props: &KVGrid) {
        let mut visible_rows: Vec<KVGridRecord> = Vec::new();

        for row in self.rows.values() {
            let name = row.name.as_str();
            let value = props.data.get(name);

            if value.is_some() || row.placeholder.is_some() || row.required {
                let value = match value {
                    None => {
                        if let Some(placeholder) = &row.placeholder {
                            placeholder.to_string().into()
                        } else {
                            Value::Null
                        }
                    }
                    Some(value) => value.clone(),
                };

                visible_rows.push(KVGridRecord {
                    row: Rc::clone(row),
                    value,
                    store: Rc::clone(&props.data),
                });
            }
        }
        self.store.set_data(visible_rows);
    }
}

fn convert_rows(rows: &[KVGridRow]) -> Rc<IndexMap<String, Rc<KVGridRow>>> {
    let rows: IndexMap<String, Rc<KVGridRow>> = rows
        .iter()
        .map(|row| (row.name.clone(), Rc::new(row.clone())))
        .collect();
    Rc::new(rows)
}

impl Component for PwtKVGrid {
    type Message = ();
    type Properties = KVGrid;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let selection = Selection::new().on_select({
            let on_select = props.on_select.clone();
            move |selection: Selection| {
                if let Some(on_select) = &on_select {
                    on_select.emit(selection.selected_key());
                }
            }
        });

        let mut me = Self {
            rows: convert_rows(&props.rows),
            store: Store::new(),
            selection,
        };
        me.data_update(props);
        me
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();

        if props.data != old_props.data || !Rc::ptr_eq(&props.rows, &old_props.rows) {
            if !Rc::ptr_eq(&props.rows, &old_props.rows) {
                self.rows = convert_rows(&props.rows);
            }
            self.data_update(props);
        }

        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        DataTable::new(COLUMNS.with(Rc::clone), self.store.clone())
            .class(props.class.clone())
            .cell_class(props.cell_class.clone())
            .borderless(props.borderless)
            .striped(props.striped)
            .virtual_scroll(false)
            .show_header(false)
            .selection(self.selection.clone())
            .on_row_click(props.on_row_click.clone())
            .on_row_dblclick(props.on_row_dblclick.clone())
            .on_row_keydown(props.on_row_keydown.clone())
            .into()
    }
}

impl Into<VNode> for KVGrid {
    fn into(self) -> VNode {
        let key = self.key.clone();
        let comp = VComp::new::<PwtKVGrid>(Rc::new(self), key);
        VNode::from(comp)
    }
}

fn render_value(value: &Value) -> Html {
    match value {
        Value::Null => html! { {"NULL"} },
        Value::Bool(v) => html! { {v.to_string()} },
        Value::Number(v) => html! { {v.to_string()} },
        Value::String(v) => html! { {v} },
        v => html! { {v.to_string()} },
    }
}
