use std::collections::HashSet;
use std::rc::Rc;

use anyhow::Error;
use gloo_timers::callback::Timeout;
use pwt::state::{Selection, Store};
use pwt::touch::SnackBar;
use serde_json::{json, Value};

use yew::html::IntoEventCallback;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::{CssLength, ExtractPrimaryKey, IntoSubmitCallback, SubmitCallback};
use pwt::touch::SnackBarContextExt;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader, DataTableMouseEvent};
use pwt::widget::{Column, Container};
use pwt::AsyncAbortGuard;

use crate::{ApiLoadCallback, IntoApiLoadCallback, PendingPropertyList, PropertyList};

use pwt_macros::builder;

use crate::pve_api_types::QemuPendingConfigValue;
use crate::{EditDialog, EditableProperty};

/// Render a list of pending changes ([`Vec<QemuPendingConfigValue>`])
#[derive(Properties, Clone, PartialEq)]
#[builder]
pub struct PendingPropertyGrid {
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

pub enum Msg {
    Load,
    LoadResult(Result<Vec<QemuPendingConfigValue>, String>),
    ShowDialog(Option<Html>),
    EditProperty(EditableProperty),
    Revert(EditableProperty),
    RevertResult(Result<(), Error>),
}

#[derive(Clone, PartialEq)]
struct PropertyGridRecord {
    key: Key,
    header: Html,
    content: Html,
}

impl ExtractPrimaryKey for PropertyGridRecord {
    fn extract_key(&self) -> Key {
        Key::from(self.key.clone())
    }
}

pub struct PvePendingPropertyGrid {
    data: Option<Result<(Value, Value, HashSet<String>), String>>,
    reload_timeout: Option<Timeout>,
    load_guard: Option<AsyncAbortGuard>,
    revert_guard: Option<AsyncAbortGuard>,
    edit_dialog: Option<Html>,
    store: Store<PropertyGridRecord>,
    columns: Rc<Vec<DataTableHeader<PropertyGridRecord>>>,
    selection: Selection,
}

impl PvePendingPropertyGrid {
    fn update_store(&mut self, ctx: &Context<Self>) {
        let props = ctx.props();

        let (current, pending, keys): (Value, Value, HashSet<String>) = match &self.data {
            Some(Ok(data)) => data.clone(),
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
                    PendingPropertyList::render_property_value(&current, &pending, item);

                let mut content = Column::new()
                    //.gap(0.5)
                    .with_child(Container::new().with_child(value.clone()));

                if let Some(new_value) = new_value {
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
                });
            }
        }
        self.store.set_data(rows);
    }

    fn view_properties(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let table = DataTable::new(self.columns.clone(), self.store.clone())
            .class(pwt::css::FlexFit)
            //.show_header(false)
            .virtual_scroll(false)
            .selection(self.selection.clone())
            .on_row_dblclick({
                let link = ctx.link().clone();
                let properties = Rc::clone(&props.properties);
                move |event: &mut DataTableMouseEvent| {
                    let key = AttrValue::from(event.record_key.to_string());
                    if let Some(property) = properties.iter().find(|p| p.get_name() == Some(&key)) {
                        link.send_message(Msg::EditProperty(property.clone()));
                    }
                }
            });

        Column::new()
            .class(props.class.clone())
            .with_child(table)
            .with_optional_child(self.edit_dialog.clone())
            .into()
    }
}

impl Component for PvePendingPropertyGrid {
    type Message = Msg;
    type Properties = PendingPropertyGrid;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        ctx.link().send_message(Msg::Load);

        let selection = Selection::new().on_select({
            let on_select = props.on_select.clone();
            move |selection: Selection| {
                if let Some(on_select) = &on_select {
                    on_select.emit(selection.selected_key());
                }
            }
        });

        let mut me = Self {
            data: None,
            reload_timeout: None,
            load_guard: None,
            revert_guard: None,
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
            Msg::Revert(property) => {
                let link = ctx.link().clone();
                let keys = match property.revert_keys.as_deref() {
                    Some(keys) => keys.iter().map(|a| a.to_string()).collect(),
                    None::<_> => {
                        if let Some(name) = property.get_name() {
                            vec![name.to_string()]
                        } else {
                            log::error!(
                                "pending property list: cannot revert property without name",
                            );
                            return false;
                        }
                    }
                };
                if let Some(on_submit) = props.on_submit.clone() {
                    let param = json!({ "revert": keys });
                    self.revert_guard = Some(AsyncAbortGuard::spawn(async move {
                        let result = on_submit.apply(param).await;
                        link.send_message(Msg::RevertResult(result));
                    }));
                }
            }
            Msg::RevertResult(result) => {
                if let Err(err) = result {
                    ctx.link().show_snackbar(
                        SnackBar::new()
                            .message(tr!("Revert property failed") + " - " + &err.to_string()),
                    );
                }
                if self.reload_timeout.is_some() {
                    ctx.link().send_message(Msg::Load);
                }
            }
            Msg::EditProperty(property) => {
                let dialog = EditDialog::from(property.clone())
                    .on_done(ctx.link().callback(|_| Msg::ShowDialog(None)))
                    .loader(props.editor_loader.clone())
                    .on_submit(props.on_submit.clone())
                    .into();
                self.edit_dialog = Some(dialog);
            }
            Msg::Load => {
                self.reload_timeout = None;
                let link = ctx.link().clone();
                if let Some(loader) = props.pending_loader.clone() {
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
                self.data = match result {
                    Ok(data) => Some(
                        PendingPropertyList::pve_pending_config_array_to_objects(data)
                            .map_err(|err| err.to_string()),
                    ),
                    Err(err) => Some(Err(err.to_string())),
                };
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
        // fixme: ??
        crate::layout::render_loaded_data(&self.data, |_| self.view_properties(ctx))
    }
}

impl From<PendingPropertyGrid> for VNode {
    fn from(props: PendingPropertyGrid) -> Self {
        let comp = VComp::new::<PvePendingPropertyGrid>(Rc::new(props), None);
        VNode::from(comp)
    }
}

fn columns() -> Rc<Vec<DataTableHeader<PropertyGridRecord>>> {
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
