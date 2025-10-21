use std::rc::Rc;

use gloo_timers::callback::Timeout;
use pwt::state::{Selection, Store};
use serde_json::Value;

use yew::html::IntoEventCallback;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::{ExtractPrimaryKey, IntoSubmitCallback, SubmitCallback};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader, DataTableKeyboardEvent, DataTableMouseEvent,
};
use pwt::widget::{Button, Column, Toolbar};
use pwt::AsyncAbortGuard;

use crate::{ApiLoadCallback, IntoApiLoadCallback, PropertyList};

use pwt_macros::builder;

use crate::{EditableProperty, PropertyEditDialog};

/// Render object properties as [List]
#[derive(Properties, Clone, PartialEq)]
#[builder]
pub struct PropertyGrid {
    /// CSS class
    #[prop_or_default]
    pub class: Classes,

    /// List of property definitions
    pub properties: Rc<Vec<EditableProperty>>,

    /// Data loader.
    #[builder_cb(IntoApiLoadCallback, into_api_load_callback, Value)]
    #[prop_or_default]
    pub loader: Option<ApiLoadCallback<Value>>,

    /// Submit callback.
    #[builder_cb(IntoSubmitCallback, into_submit_callback, Value)]
    #[prop_or_default]
    pub on_submit: Option<SubmitCallback<Value>>,

    /// Select callback.
    #[prop_or_default]
    #[builder_cb(IntoEventCallback, into_event_callback, Option<Key>)]
    pub on_select: Option<Callback<Option<Key>>>,
}

impl PropertyGrid {
    pub fn new(properties: Rc<Vec<EditableProperty>>) -> Self {
        yew::props!(Self { properties })
    }

    pwt::impl_class_prop_builder!();

    fn lookup_property(&self, key: &Key) -> Option<&EditableProperty> {
        let property_name: AttrValue = key.to_string().into();
        self.properties
            .iter()
            .find(|p| p.get_name() == Some(&property_name))
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct PropertyGridRecord {
    pub key: Key,
    pub header: Html,
    pub content: Html,
    pub has_changes: bool,
}

impl ExtractPrimaryKey for PropertyGridRecord {
    fn extract_key(&self) -> Key {
        Key::from(self.key.clone())
    }
}

pub enum Msg {
    Load,
    LoadResult(Result<Value, String>),
    ShowDialog(Option<Html>),
    EditProperty(Key),
    Select(Option<Key>),
}

pub struct PvePropertyGrid {
    data: Option<Value>,
    error: Option<String>,
    reload_timeout: Option<Timeout>,
    load_guard: Option<AsyncAbortGuard>,
    edit_dialog: Option<Html>,
    store: Store<PropertyGridRecord>,
    columns: Rc<Vec<DataTableHeader<PropertyGridRecord>>>,
    selection: Selection,
}

impl PvePropertyGrid {
    fn toolbar(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let selected_key = self.selection.selected_key();

        let toolbar = Toolbar::new()
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(selected_key.is_none())
                    .onclick({
                        let key = selected_key.clone();
                        let link = link.clone();
                        move |_| {
                            if let Some(key) = &key {
                                link.send_message(Msg::EditProperty(key.clone()));
                            }
                        }
                    }),
            );

        toolbar.into()
    }

    fn update_store(&mut self, ctx: &Context<Self>) {
        let props = ctx.props();

        let record = match &self.data {
            Some(data) => data.clone(),
            _ => Value::Null,
        };

        let mut rows: Vec<PropertyGridRecord> = Vec::new();

        for item in props.properties.iter() {
            let name = match item.get_name() {
                Some(name) => name.to_string(),
                None::<_> => {
                    log::error!("property list: skiping property without name");
                    continue;
                }
            };
            let property_exists = match record.as_object() {
                Some(map) => map.contains_key(&name),
                None => false,
            };

            if item.required || property_exists {
                let header = html! { &item.title };
                let content = PropertyList::render_property_value(&record, item);

                rows.push(PropertyGridRecord {
                    key: Key::from(name.clone()),
                    header,
                    content,
                    has_changes: false,
                });
            }
        }
        self.store.set_data(rows);
    }
}

impl Component for PvePropertyGrid {
    type Message = Msg;
    type Properties = PropertyGrid;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        ctx.link().send_message(Msg::Load);

        let selection = Selection::new().on_select({
            let on_select = props.on_select.clone();
            let link = ctx.link().clone();
            move |selection: Selection| {
                let selected_key = selection.selected_key();
                link.send_message(Msg::Select(selected_key.clone()));
                if let Some(on_select) = &on_select {
                    on_select.emit(selected_key);
                }
            }
        });

        let mut me = Self {
            data: None,
            error: None,
            reload_timeout: None,
            load_guard: None,
            edit_dialog: None,
            store: Store::new(),
            columns: columns(),
            selection,
        };
        me.update_store(ctx);
        me
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::Select(_key) => { /* just redraw */ }
            Msg::EditProperty(key) => {
                let property = match props.lookup_property(&key) {
                    Some(property) => property,
                    None::<_> => return false,
                };

                let dialog = PropertyEditDialog::from(property.clone())
                    .on_done(ctx.link().callback(|_| Msg::ShowDialog(None)))
                    .loader(props.loader.clone())
                    .on_submit(props.on_submit.clone())
                    .into();
                self.edit_dialog = Some(dialog);
            }
            Msg::Load => {
                self.reload_timeout = None;
                let link = ctx.link().clone();
                if let Some(loader) = props.loader.clone() {
                    self.load_guard = Some(AsyncAbortGuard::spawn(async move {
                        let result = loader.apply().await;
                        let data = match result {
                            Ok(result) => Ok(result.data),
                            Err(err) => Err(err.to_string()),
                        };
                        link.send_message(Msg::LoadResult(data));
                    }));
                }
            }
            Msg::LoadResult(result) => {
                match result {
                    Ok(data) => {
                        self.data = Some(data);
                        self.error = None;
                    }
                    Err(err) => self.error = Some(err),
                }
                self.update_store(ctx);
                let link = ctx.link().clone();
                self.reload_timeout = Some(Timeout::new(3000, move || {
                    link.send_message(Msg::Load);
                }));
            }
            Msg::ShowDialog(dialog) => {
                if dialog.is_none() && self.reload_timeout.is_some() {
                    ctx.link().send_message(Msg::Load);
                }
                self.edit_dialog = dialog;
            }
        }
        true
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if props.properties != old_props.properties {
            self.update_store(ctx);
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let table = DataTable::new(self.columns.clone(), self.store.clone())
            .class(pwt::css::FlexFit)
            .show_header(false)
            .virtual_scroll(false)
            .selection(self.selection.clone())
            .on_row_dblclick({
                let link = ctx.link().clone();
                move |event: &mut DataTableMouseEvent| {
                    link.send_message(Msg::EditProperty(event.record_key.clone()));
                }
            })
            .on_row_keydown({
                let link = ctx.link().clone();
                move |event: &mut DataTableKeyboardEvent| {
                    if event.key() == " " {
                        link.send_message(Msg::EditProperty(event.record_key.clone()));
                    }
                }
            });

        let loading = self.data.is_none() && self.error.is_none();

        Column::new()
            .class(props.class.clone())
            .with_optional_child(
                loading.then(|| pwt::widget::Progress::new().class("pwt-delay-visibility")),
            )
            .with_child(self.toolbar(ctx))
            .with_child(table)
            .with_optional_child(
                self.error
                    .as_deref()
                    .map(|err| pwt::widget::error_message(&err.to_string()).padding(2)),
            )
            .with_optional_child(self.edit_dialog.clone())
            .into()
    }
}

impl From<PropertyGrid> for VNode {
    fn from(props: PropertyGrid) -> Self {
        let comp = VComp::new::<PvePropertyGrid>(Rc::new(props), None);
        VNode::from(comp)
    }
}

pub(crate) fn columns() -> Rc<Vec<DataTableHeader<PropertyGridRecord>>> {
    Rc::new(vec![
        DataTableColumn::new(tr!("Key"))
            .width("15em")
            .show_menu(false)
            .render(|record: &PropertyGridRecord| record.header.clone())
            .into(),
        DataTableColumn::new(tr!("Value"))
            //.flex(1)
            .width("1fr")
            .show_menu(false)
            .render(|record: &PropertyGridRecord| record.content.clone())
            .into(),
    ])
}
