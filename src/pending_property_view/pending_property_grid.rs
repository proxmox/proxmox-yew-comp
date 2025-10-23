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

use super::{
    PendingPropertyView, PendingPropertyViewMsg, PendingPropertyViewState, PvePendingPropertyView,
};

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

impl PvePendingPropertyGrid {
    fn toolbar(&self, ctx: &Context<PvePendingPropertyView<Self>>) -> Html {
        let link = ctx.link();

        let selected_key = self.selection.selected_key();
        let selected_record = selected_key
            .as_ref()
            .map(|key| self.store.read().lookup_record(&key).cloned())
            .flatten();
        let has_changes = selected_record
            .as_ref()
            .map(|record| record.has_changes)
            .unwrap_or(false);

        let property = selected_record.as_ref().map(|r| r.property.clone());

        let disable_revert = !(has_changes && selected_key.is_some());

        let toolbar = Toolbar::new()
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(selected_key.is_none())
                    .onclick({
                        let link = link.clone();
                        let property = property.clone();
                        move |_| {
                            if let Some(property) = &property {
                                link.send_message(PendingPropertyViewMsg::EditProperty(
                                    property.clone(),
                                ));
                            }
                        }
                    }),
            )
            .with_child(
                Button::new(tr!("Revert"))
                    .disabled(disable_revert)
                    .onclick({
                        let link = link.clone();
                        let property = property.clone();
                        move |_| {
                            if let Some(property) = &property {
                                link.send_message(PendingPropertyViewMsg::RevertProperty(
                                    property.clone(),
                                ));
                            }
                        }
                    }),
            );

        toolbar.into()
    }
}

impl PendingPropertyView for PvePendingPropertyGrid {
    type Properties = PendingPropertyGrid;
    type Message = ();

    const MOBILE: bool = false;

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
        view_state: &mut PendingPropertyViewState,
    ) {
        let props = ctx.props();

        let (current, pending, keys): (Value, Value, HashSet<String>) = match &view_state.data {
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
                    property: item.clone(),
                    header,
                    content: content.into(),
                    has_changes,
                });
            }
        }
        self.store.set_data(rows);
    }

    fn changed(
        &mut self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        view_state: &mut PendingPropertyViewState,
        old_props: &Self::Properties,
    ) -> bool {
        let props = ctx.props();
        if props.properties != old_props.properties {
            self.update_data(ctx, view_state);
        }
        true
    }

    fn view(
        &self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        view_state: &PendingPropertyViewState,
    ) -> Html {
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
                        link.send_message(PendingPropertyViewMsg::EditProperty(property));
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
                            link.send_message(PendingPropertyViewMsg::EditProperty(property));
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

        crate::property_view::render_loadable_panel(
            class,
            table,
            Some(toolbar),
            dialog,
            loading,
            error,
        )
    }
}

impl From<PendingPropertyGrid> for VNode {
    fn from(props: PendingPropertyGrid) -> Self {
        let comp =
            VComp::new::<PvePendingPropertyView<PvePendingPropertyGrid>>(Rc::new(props), None);
        VNode::from(comp)
    }
}
