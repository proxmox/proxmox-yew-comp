mod pending_property_grid;
pub use pending_property_grid::PendingPropertyGrid;

mod pending_property_list;
pub use pending_property_list::PendingPropertyList;

use std::collections::HashSet;
use std::rc::Rc;

use anyhow::Error;
use gloo_timers::callback::Timeout;
use serde_json::{json, Value};

use yew::virtual_dom::Key;

use pwt::prelude::*;
use pwt::props::SubmitCallback;
use pwt::touch::{SnackBar, SnackBarContextExt};
use pwt::widget::{AlertDialog, Column};
use pwt::AsyncAbortGuard;

use crate::pve_api_types::QemuPendingConfigValue;
use crate::{ApiLoadCallback, EditableProperty, PropertyEditDialog};

pub enum PendingPropertyViewMsg {
    Load,
    LoadResult(Result<Vec<QemuPendingConfigValue>, String>),
    ShowDialog(Option<Html>),
    EditProperty(Key),
    Revert(Key),
    RevertResult(Result<(), Error>),
    Select(Option<Key>),
}

pub trait PendingPropertyView {
    type Properties: Properties;
    const MOBILE: bool;

    fn class(props: &Self::Properties) -> &Classes;

    fn properties(props: &Self::Properties) -> &Rc<Vec<EditableProperty>>;

    fn editor_loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>>;

    fn pending_loader(
        props: &Self::Properties,
    ) -> Option<ApiLoadCallback<Vec<QemuPendingConfigValue>>>;

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>>;

    fn create(ctx: &Context<PvePendingPropertyView<Self>>) -> Self
    where
        Self: 'static + Sized;

    fn update_data(
        &mut self,
        _ctx: &Context<PvePendingPropertyView<Self>>,
        _data: Option<&(Value, Value, HashSet<String>)>,
        _error: Option<&str>,
    ) where
        Self: 'static + Sized,
    {
    }

    fn toolbar(
        &self,
        _ctx: &Context<PvePendingPropertyView<Self>>,
        _data: Option<&(Value, Value, HashSet<String>)>,
        _error: Option<&str>,
    ) -> Option<Html>
    where
        Self: 'static + Sized,
    {
        None
    }

    fn view(
        &self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        data: Option<&(Value, Value, HashSet<String>)>,
        error: Option<&str>,
    ) -> Html
    where
        Self: 'static + Sized;
}

pub struct PvePendingPropertyView<T> {
    data: Option<(Value, Value, HashSet<String>)>,
    error: Option<String>,
    reload_timeout: Option<Timeout>,
    load_guard: Option<AsyncAbortGuard>,
    revert_guard: Option<AsyncAbortGuard>,
    dialog: Option<Html>,
    view_state: T,
}

impl<T: 'static + PendingPropertyView> Component for PvePendingPropertyView<T> {
    type Message = PendingPropertyViewMsg;
    type Properties = T::Properties;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(PendingPropertyViewMsg::Load);

        let mut me = Self {
            data: None,
            error: None,
            reload_timeout: None,
            load_guard: None,
            revert_guard: None,
            dialog: None,
            view_state: T::create(ctx),
        };
        me.view_state.update_data(ctx, None, None);
        me
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            PendingPropertyViewMsg::Select(_key) => { /* just redraw */ }
            PendingPropertyViewMsg::Revert(key) => {
                let property = match lookup_property(T::properties(props), &key) {
                    Some(property) => property,
                    None::<_> => return false,
                };
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
                if let Some(on_submit) = T::on_submit(props) {
                    let param = json!({ "revert": keys });
                    self.revert_guard = Some(AsyncAbortGuard::spawn(async move {
                        let result = on_submit.apply(param).await;
                        link.send_message(PendingPropertyViewMsg::RevertResult(result));
                    }));
                }
            }
            PendingPropertyViewMsg::RevertResult(result) => {
                if let Err(err) = result {
                    if T::MOBILE {
                        ctx.link().show_snackbar(
                            SnackBar::new()
                                .message(tr!("Revert property failed") + " - " + &err.to_string()),
                        );
                    } else {
                        self.dialog = Some(
                            AlertDialog::new(
                                tr!("Revert property failed") + " - " + &err.to_string(),
                            )
                            .on_close(
                                ctx.link()
                                    .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                            )
                            .into(),
                        );
                    }
                }
                if self.reload_timeout.is_some() {
                    ctx.link().send_message(PendingPropertyViewMsg::Load);
                }
            }
            PendingPropertyViewMsg::EditProperty(key) => {
                let property = match lookup_property(T::properties(props), &key) {
                    Some(property) => property,
                    None::<_> => return false,
                };
                let dialog = PropertyEditDialog::from(property.clone())
                    .mobile(T::MOBILE)
                    .on_done(
                        ctx.link()
                            .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                    )
                    .loader(T::editor_loader(props))
                    .on_submit(T::on_submit(props))
                    .into();
                self.dialog = Some(dialog);
            }
            PendingPropertyViewMsg::Load => {
                self.reload_timeout = None;
                let link = ctx.link().clone();
                if let Some(loader) = T::pending_loader(props) {
                    self.load_guard = Some(AsyncAbortGuard::spawn(async move {
                        let result = loader.apply().await;
                        let data = match result {
                            Ok(result) => Ok(result.data),
                            Err(err) => Err(err.to_string()),
                        };
                        link.send_message(PendingPropertyViewMsg::LoadResult(data));
                    }));
                }
            }
            PendingPropertyViewMsg::LoadResult(result) => {
                let result = result.and_then(|data| {
                    pve_pending_config_array_to_objects(data).map_err(|err| err.to_string())
                });
                match result {
                    Ok(data) => {
                        self.data = Some(data);
                        self.error = None;
                    }
                    Err(err) => self.error = Some(err),
                }
                self.view_state
                    .update_data(ctx, self.data.as_ref(), self.error.as_deref());
                let link = ctx.link().clone();
                self.reload_timeout = Some(Timeout::new(3000, move || {
                    link.send_message(PendingPropertyViewMsg::Load);
                }));
            }
            PendingPropertyViewMsg::ShowDialog(dialog) => {
                if dialog.is_none() && self.reload_timeout.is_some() {
                    ctx.link().send_message(PendingPropertyViewMsg::Load);
                }
                self.dialog = dialog;
            }
        }
        true
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if T::properties(props) != T::properties(old_props) {
            self.view_state
                .update_data(ctx, self.data.as_ref(), self.error.as_deref());
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let panel = self
            .view_state
            .view(ctx, self.data.as_ref(), self.error.as_deref());

        let loading = self.data.is_none() && self.error.is_none();

        Column::new()
            .class(T::class(props).clone())
            .with_optional_child(self.view_state.toolbar(
                ctx,
                self.data.as_ref(),
                self.error.as_deref(),
            ))
            .with_optional_child(
                loading.then(|| pwt::widget::Progress::new().class("pwt-delay-visibility")),
            )
            .with_child(panel)
            .with_optional_child(
                self.error
                    .as_deref()
                    .map(|err| pwt::widget::error_message(&err.to_string()).padding(2)),
            )
            .with_optional_child(self.dialog.clone())
            .into()
    }
}

fn lookup_property<'a>(
    properties: &'a [EditableProperty],
    key: &Key,
) -> Option<&'a EditableProperty> {
    let property_name: AttrValue = key.to_string().into();
    properties
        .iter()
        .find(|p| p.get_name() == Some(&property_name))
}

pub fn render_pending_property_value(
    current: &Value,
    pending: &Value,
    property: &EditableProperty,
) -> (Html, Option<Html>) {
    let value = crate::property_view::render_property_value(current, property);
    let new_value = crate::property_view::render_property_value(pending, property);

    if value != new_value {
        (value, Some(new_value))
    } else {
        (value, None)
    }
}

/// Parse PVE pending configuration array
///
/// Returns 2 Objects, containing current and pending configuration,
/// and the set of contained configuration  keys.
pub fn pve_pending_config_array_to_objects(
    data: Vec<QemuPendingConfigValue>,
) -> Result<(Value, Value, HashSet<String>), Error> {
    let mut current = serde_json::Map::new();
    let mut pending = serde_json::Map::new();
    let mut keys = HashSet::new();

    for item in data.iter() {
        keys.insert(item.key.clone());

        if let Some(value) = item.value.clone() {
            current.insert(item.key.clone(), value);
        }
        if matches!(item.delete, Some(1) | Some(2)) {
            continue;
        }
        if let Some(value) = item.pending.clone() {
            pending.insert(item.key.clone(), value);
        } else if let Some(value) = item.value.clone() {
            pending.insert(item.key.clone(), value);
        }
    }

    Ok((Value::Object(current), Value::Object(pending), keys))
}
