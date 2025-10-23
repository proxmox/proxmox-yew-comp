mod pending_property_grid;
pub use pending_property_grid::PendingPropertyGrid;

mod pending_property_list;
pub use pending_property_list::PendingPropertyList;

use std::collections::HashSet;

use anyhow::Error;
use gloo_timers::callback::Timeout;
use serde_json::{json, Value};

use yew::virtual_dom::Key;

use pwt::props::SubmitCallback;
use pwt::touch::{SnackBar, SnackBarContextExt};
use pwt::widget::AlertDialog;
use pwt::AsyncAbortGuard;
use pwt::{prelude::*, AsyncPool};

use crate::pve_api_types::QemuPendingConfigValue;
use crate::{ApiLoadCallback, EditableProperty, PropertyEditDialog};

pub enum PendingPropertyViewMsg<M> {
    Load,
    LoadResult(Result<Vec<QemuPendingConfigValue>, String>),
    ShowDialog(Option<Html>),
    EditProperty(EditableProperty),
    RevertProperty(EditableProperty),
    RevertResult(Result<(), Error>),
    Select(Option<Key>),
    Custom(M),
}

pub trait PendingPropertyView {
    type Properties: Properties;
    type Message;

    const MOBILE: bool;

    fn editor_loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>>;

    fn pending_loader(
        props: &Self::Properties,
    ) -> Option<ApiLoadCallback<Vec<QemuPendingConfigValue>>>;

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>>;

    fn create(ctx: &Context<PvePendingPropertyView<Self>>) -> Self
    where
        Self: 'static + Sized;

    #[allow(unused_variables)]
    fn update(
        &mut self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        view_state: &mut PendingPropertyViewState,
        msg: Self::Message,
    ) -> bool
    where
        Self: 'static + Sized,
    {
        true
    }

    #[allow(unused_variables)]
    fn changed(
        &mut self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        view_state: &mut PendingPropertyViewState,
        old_props: &Self::Properties,
    ) -> bool
    where
        Self: 'static + Sized,
    {
        true
    }

    #[allow(unused_variables)]
    fn update_data(
        &mut self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        view_state: &mut PendingPropertyViewState,
    ) where
        Self: 'static + Sized,
    {
    }

    fn view(
        &self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        view_state: &PendingPropertyViewState,
    ) -> Html
    where
        Self: 'static + Sized;
}

pub struct PendingPropertyViewState {
    pub data: Option<(Value, Value, HashSet<String>)>,
    pub error: Option<String>,
    pub reload_timeout: Option<Timeout>,
    pub load_guard: Option<AsyncAbortGuard>,
    pub revert_guard: Option<AsyncAbortGuard>,
    pub async_pool: AsyncPool,
    pub dialog: Option<Html>,
}

impl PendingPropertyViewState {
    pub fn update(&mut self, result: Result<(Value, Value, HashSet<String>), String>) {
        match result {
            Ok(data) => {
                self.error = None;
                self.data = Some(data);
            }
            Err(err) => {
                self.error = Some(err);
            }
        }
    }

    pub fn loading(&self) -> bool {
        self.data.is_none() && self.error.is_none()
    }
}

pub struct PvePendingPropertyView<T> {
    view_state: PendingPropertyViewState,
    child_state: T,
}

impl<T: 'static + PendingPropertyView> Component for PvePendingPropertyView<T> {
    type Message = PendingPropertyViewMsg<T::Message>;
    type Properties = T::Properties;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(PendingPropertyViewMsg::Load);

        let mut me = Self {
            view_state: PendingPropertyViewState {
                data: None,
                error: None,
                reload_timeout: None,
                load_guard: None,
                revert_guard: None,
                async_pool: AsyncPool::new(),
                dialog: None,
            },
            child_state: T::create(ctx),
        };
        me.child_state.update_data(ctx, &mut me.view_state);
        me
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            PendingPropertyViewMsg::Custom(custom) => {
                return self.child_state.update(ctx, &mut self.view_state, custom);
            }
            PendingPropertyViewMsg::Select(_key) => { /* just redraw */ }
            PendingPropertyViewMsg::RevertProperty(property) => {
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
                    self.view_state.revert_guard = Some(AsyncAbortGuard::spawn(async move {
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
                        self.view_state.dialog = Some(
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
                if self.view_state.reload_timeout.is_some() {
                    ctx.link().send_message(PendingPropertyViewMsg::Load);
                }
            }
            PendingPropertyViewMsg::EditProperty(property) => {
                let dialog = PropertyEditDialog::from(property.clone())
                    .mobile(T::MOBILE)
                    .on_done(
                        ctx.link()
                            .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                    )
                    .loader(T::editor_loader(props))
                    .on_submit(T::on_submit(props))
                    .into();
                self.view_state.dialog = Some(dialog);
            }
            PendingPropertyViewMsg::Load => {
                self.view_state.reload_timeout = None;
                let link = ctx.link().clone();
                if let Some(loader) = T::pending_loader(props) {
                    self.view_state.load_guard = Some(AsyncAbortGuard::spawn(async move {
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
                self.view_state.update(result);
                self.child_state.update_data(ctx, &mut self.view_state);
                let link = ctx.link().clone();
                self.view_state.reload_timeout = Some(Timeout::new(3000, move || {
                    link.send_message(PendingPropertyViewMsg::Load);
                }));
            }
            PendingPropertyViewMsg::ShowDialog(dialog) => {
                if dialog.is_none() && self.view_state.reload_timeout.is_some() {
                    ctx.link().send_message(PendingPropertyViewMsg::Load);
                }
                self.view_state.dialog = dialog;
            }
        }
        true
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        self.child_state
            .changed(ctx, &mut self.view_state, old_props)
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        self.child_state.view(ctx, &self.view_state)
    }
}

pub fn lookup_property<'a>(
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
