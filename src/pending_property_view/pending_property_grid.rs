use std::collections::HashSet;
use std::rc::Rc;

use pwt::state::{Selection, Store};
use serde_json::Value;

use yew::html::IntoEventCallback;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoSubmitCallback, SubmitCallback};
use pwt::widget::data_table::{
    DataTable, DataTableHeader, DataTableKeyboardEvent, DataTableMouseEvent,
};
use pwt::widget::{Button, Column, Container, Toolbar};

use crate::{ApiLoadCallback, IntoApiLoadCallback};

use pwt_macros::builder;

use crate::property_view::{property_grid_columns, PropertyGridRecord};
use crate::pve_api_types::QemuPendingConfigValue;
use crate::EditableProperty;

use super::{PendingPropertyView, PendingPropertyViewMsg, PvePendingPropertyView};

/// Render a list of pending changes ([`Vec<QemuPendingConfigValue>`])
#[derive(Properties, Clone, PartialEq)]
#[builder]
pub struct PendingPropertyGrid {
    /// CSS class
    #[prop_or_default]
    pub class: Classes,

    /// List of property definitions
    pub properties: Rc<Vec<EditableProperty>>,

    /// Load property list with pending changes information.
    #[builder_cb(IntoApiLoadCallback, into_api_load_callback, Vec<QemuPendingConfigValue>)]
    #[prop_or_default]
    pub pending_loader: Option<ApiLoadCallback<Vec<QemuPendingConfigValue>>>,

    /// Loader passed to the EditDialog
    #[builder_cb(IntoApiLoadCallback, into_api_load_callback, Value)]
    #[prop_or_default]
    pub editor_loader: Option<ApiLoadCallback<Value>>,

    /// Submit callback.
    #[builder_cb(IntoSubmitCallback, into_submit_callback, Value)]
    #[prop_or_default]
    pub on_submit: Option<SubmitCallback<Value>>,

    /// Select callback.
    #[prop_or_default]
    #[builder_cb(IntoEventCallback, into_event_callback, Option<Key>)]
    pub on_select: Option<Callback<Option<Key>>>,
}

impl PendingPropertyGrid {
    pub fn new(properties: Rc<Vec<EditableProperty>>) -> Self {
        yew::props!(Self { properties })
    }
    pwt::impl_class_prop_builder!();
}

pub struct PvePendingPropertyGrid {
    store: Store<PropertyGridRecord>,
    columns: Rc<Vec<DataTableHeader<PropertyGridRecord>>>,
    selection: Selection,
}

impl PendingPropertyView for PvePendingPropertyGrid {
    type Properties = PendingPropertyGrid;
    type Message = ();

    const MOBILE: bool = false;

    fn class(props: &Self::Properties) -> &Classes {
        &props.class
    }

    fn properties(props: &Self::Properties) -> &Rc<Vec<EditableProperty>> {
        &props.properties
    }

    fn editor_loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>> {
        props.editor_loader.clone()
    }

    fn pending_loader(
        props: &Self::Properties,
    ) -> Option<ApiLoadCallback<Vec<QemuPendingConfigValue>>> {
        props.pending_loader.clone()
    }

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>> {
        props.on_submit.clone()
    }

    fn create(ctx: &Context<PvePendingPropertyView<Self>>) -> Self {
        let props = ctx.props();
        let selection = Selection::new().on_select({
            let on_select = props.on_select.clone();
            let link = ctx.link().clone();
            move |selection: Selection| {
                let selected_key = selection.selected_key();
                link.send_message(PendingPropertyViewMsg::Select(selected_key.clone()));
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
        ctx: &Context<super::PvePendingPropertyView<Self>>,
        data: Option<&(Value, Value, HashSet<String>)>,
        _error: Option<&str>,
    ) {
        let props = ctx.props();

        let (current, pending, keys): (Value, Value, HashSet<String>) = match data {
            Some(data) => data.clone(),
            _ => (Value::Null, Value::Null, HashSet::new()),
        };

        let mut rows: Vec<PropertyGridRecord> = Vec::new();

        for item in props.properties.iter() {
            let name = match item.get_name() {
                Some(name) => name.to_string(),
                None::<_> => {
                    log::error!("pending property list: skiping property without name");
                    continue;
                }
            };

            if item.required || keys.contains(&name) {
                let header = html! { &item.title };
                let (value, new_value) =
                    super::render_pending_property_value(&current, &pending, item);

                let mut content = Column::new()
                    //.gap(0.5)
                    .with_child(Container::new().with_child(value.clone()));

                let mut has_changes = false;

                if let Some(new_value) = new_value {
                    has_changes = true;
                    content.add_child(
                        Container::new()
                            .class("pwt-color-warning")
                            .with_child(new_value),
                    );
                }

                rows.push(PropertyGridRecord {
                    key: Key::from(name.clone()),
                    header,
                    content: content.into(),
                    has_changes,
                });
            }
        }
        self.store.set_data(rows);
    }

    fn toolbar(
        &self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        _data: Option<&(Value, Value, HashSet<String>)>,
        _error: Option<&str>,
    ) -> Option<Html> {
        let link = ctx.link();

        let selected_key = self.selection.selected_key();
        let has_changes = selected_key
            .as_ref()
            .map(|key| self.store.read().lookup_record(&key).cloned())
            .flatten()
            .map(|record| record.has_changes)
            .unwrap_or(false);

        let disable_revert = !(has_changes && selected_key.is_some());

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
                                link.send_message(PendingPropertyViewMsg::Edit(key.clone()));
                            }
                        }
                    }),
            )
            .with_child(
                Button::new(tr!("Revert"))
                    .disabled(disable_revert)
                    .onclick({
                        let key = selected_key.clone();
                        let link = link.clone();
                        move |_| {
                            if let Some(key) = &key {
                                link.send_message(PendingPropertyViewMsg::Revert(key.clone()));
                            }
                        }
                    }),
            );

        Some(toolbar.into())
    }

    fn view(
        &self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        _data: Option<&(Value, Value, HashSet<String>)>,
        _error: Option<&str>,
    ) -> Html {
        DataTable::new(self.columns.clone(), self.store.clone())
            .class(pwt::css::FlexFit)
            .show_header(false)
            .virtual_scroll(false)
            .selection(self.selection.clone())
            .on_row_dblclick({
                let link = ctx.link().clone();
                move |event: &mut DataTableMouseEvent| {
                    link.send_message(PendingPropertyViewMsg::Edit(event.record_key.clone()));
                }
            })
            .on_row_keydown({
                let link = ctx.link().clone();
                move |event: &mut DataTableKeyboardEvent| {
                    if event.key() == " " {
                        link.send_message(PendingPropertyViewMsg::Edit(event.record_key.clone()));
                    }
                }
            })
            .into()
    }
}

impl From<PendingPropertyGrid> for VNode {
    fn from(props: PendingPropertyGrid) -> Self {
        let comp =
            VComp::new::<PvePendingPropertyView<PvePendingPropertyGrid>>(Rc::new(props), None);
        VNode::from(comp)
    }
}
