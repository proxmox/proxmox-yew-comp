use std::rc::Rc;

use pwt::state::{Selection, Store};
use pwt::widget::{Button, Toolbar};
use serde_json::Value;

use yew::html::IntoEventCallback;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoSubmitCallback, SubmitCallback};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader, DataTableKeyboardEvent, DataTableMouseEvent,
};

use crate::{ApiLoadCallback, IntoApiLoadCallback};

use pwt_macros::builder;

use crate::EditableProperty;

use super::{
    PropertyGridRecord, PropertyView, PropertyViewMsg, PropertyViewState, PvePropertyView,
};

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
}

struct PvePropertyGrid {
    store: Store<PropertyGridRecord>,
    columns: Rc<Vec<DataTableHeader<PropertyGridRecord>>>,
    selection: Selection,
}

impl PvePropertyGrid {
    fn toolbar(&self, ctx: &Context<PvePropertyView<Self>>) -> Html {
        let link = ctx.link();

        let selected_key = self.selection.selected_key();
        let selected_record = selected_key
            .as_ref()
            .map(|key| self.store.read().lookup_record(&key).cloned())
            .flatten();
        let property = selected_record.as_ref().map(|r| r.property.clone());

        let toolbar = Toolbar::new()
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(selected_key.is_none())
                    .onclick({
                        let property = property.clone();
                        let link = link.clone();
                        move |_| {
                            if let Some(property) = &property {
                                link.send_message(PropertyViewMsg::EditProperty(property.clone()));
                            }
                        }
                    }),
            );

        toolbar.into()
    }
}

impl PropertyView for PvePropertyGrid {
    type Properties = PropertyGrid;
    type Message = ();
    const MOBILE: bool = false;

    fn loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>> {
        props.loader.clone()
    }

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>> {
        props.on_submit.clone()
    }

    fn create(ctx: &Context<PvePropertyView<Self>>) -> Self {
        let props = ctx.props();
        let selection = Selection::new().on_select({
            let on_select = props.on_select.clone();
            let link = ctx.link().clone();
            move |selection: Selection| {
                let selected_key = selection.selected_key();
                link.send_message(PropertyViewMsg::Select(selected_key.clone()));
                if let Some(on_select) = &on_select {
                    on_select.emit(selected_key);
                }
            }
        });

        Self {
            store: Store::new(),
            columns: property_grid_columns(),
            selection,
        }
    }

    fn update_data(
        &mut self,
        ctx: &Context<PvePropertyView<Self>>,
        view_state: &mut PropertyViewState,
    ) {
        let props = ctx.props();

        let record = match &view_state.data {
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
                None::<_> => false,
            };

            if item.required || property_exists {
                let header = html! { &item.title };
                let content = super::render_property_value(&record, item);

                rows.push(PropertyGridRecord {
                    key: Key::from(name.clone()),
                    property: item.clone(),
                    header,
                    content,
                    has_changes: false,
                });
            }
        }
        self.store.set_data(rows);
    }

    fn view(&self, ctx: &Context<PvePropertyView<Self>>, view_state: &PropertyViewState) -> Html {
        let props = ctx.props();

        let table = DataTable::new(self.columns.clone(), self.store.clone())
            .class(pwt::css::FlexFit)
            .show_header(false)
            .virtual_scroll(false)
            .selection(self.selection.clone())
            .on_row_dblclick({
                let link = ctx.link().clone();
                let store = self.store.clone();
                move |event: &mut DataTableMouseEvent| {
                    let property = store
                        .read()
                        .lookup_record(&event.record_key)
                        .map(|r| r.property.clone());
                    if let Some(property) = property {
                        link.send_message(PropertyViewMsg::EditProperty(property));
                    }
                }
            })
            .on_row_keydown({
                let link = ctx.link().clone();
                let store = self.store.clone();
                move |event: &mut DataTableKeyboardEvent| {
                    if event.key() == " " {
                        let property = store
                            .read()
                            .lookup_record(&event.record_key)
                            .map(|r| r.property.clone());
                        if let Some(property) = property {
                            link.send_message(PropertyViewMsg::EditProperty(property));
                        }
                    }
                }
            })
            .into();

        let loading = view_state.loading();
        let toolbar = self.toolbar(ctx);
        let class = props.class.clone();
        let dialog = view_state.dialog.clone();
        let error = view_state.error.clone();

        super::render_loadable_panel(class, table, Some(toolbar), dialog, loading, error)
    }
}

impl From<PropertyGrid> for VNode {
    fn from(props: PropertyGrid) -> Self {
        let comp = VComp::new::<PvePropertyView<PvePropertyGrid>>(Rc::new(props), None);
        VNode::from(comp)
    }
}

pub fn property_grid_columns() -> Rc<Vec<DataTableHeader<PropertyGridRecord>>> {
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
