mod pending_property_grid;
pub use pending_property_grid::PendingPropertyGrid;

mod pending_property_list;
pub use pending_property_list::PendingPropertyList;

use std::collections::HashSet;
use std::ops::DerefMut;

use anyhow::Error;
use gloo_timers::callback::Timeout;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use proxmox_client::ApiResponseData;
use yew::virtual_dom::Key;

use pwt::props::SubmitCallback;
use pwt::touch::{SnackBar, SnackBarContextExt};
use pwt::widget::AlertDialog;
use pwt::AsyncAbortGuard;
use pwt::{prelude::*, AsyncPool};

use crate::{http_get_full, ApiLoadCallback, EditableProperty, PropertyEditDialog};
use pve_api_types::PendingConfigValue;

/// Pending configuration data
///
/// The PVE interface often returns `<Vec<QemuPendingConfigValue>>`, which
/// can be converted into this struct with: [PvePendingConfiguration::from_config_array] or
/// [pve_pending_config_array_to_objects_typed].
pub struct PvePendingConfiguration {
    /// Current, active configuration
    pub current: Value,
    pub pending: Value,
    pub keys: HashSet<String>,
}

impl PvePendingConfiguration {
    pub fn new() -> Self {
        Self {
            current: Value::Null,
            pending: Value::Null,
            keys: HashSet::new(),
        }
    }

    pub fn from_config_array(data: Vec<PendingConfigValue>) -> Self {
        let (current, pending, keys) = pve_pending_config_array_to_objects(data);
        Self {
            current,
            pending,
            keys,
        }
    }
}

/// Load data using PVE pending api
///
/// The generic type T is used to  to convert between perl and rust types.
///
pub fn pending_typed_load<T: DeserializeOwned + Serialize>(
    url: impl Into<String>,
) -> ApiLoadCallback<PvePendingConfiguration> {
    let url = url.into();
    let url_cloned = url.clone();
    ApiLoadCallback::new(move || {
        let url = url.clone();
        async move {
            let ApiResponseData { data, attribs } = http_get_full(&url, None).await?;
            let data = pve_pending_config_array_to_objects_typed::<T>(data)?;
            Ok(ApiResponseData { attribs, data })
        }
    })
    .url(url_cloned)
}

/// Note: PVE API sometime return numbers as string, and bool as 1/0
pub fn pve_pending_config_array_to_objects_typed<T: DeserializeOwned + Serialize>(
    data: Vec<PendingConfigValue>,
) -> Result<PvePendingConfiguration, Error> {
    let (current, pending, keys) = pve_pending_config_array_to_objects(data);

    let current: T = serde_json::from_value(current)?;
    let current = serde_json::to_value(current)?;

    let pending: T = serde_json::from_value(pending)?;
    let pending = serde_json::to_value(pending)?;

    Ok(PvePendingConfiguration {
        current,
        pending,
        keys,
    })
}

pub enum PendingPropertyViewMsg<M> {
    Load,
    LoadResult(Result<PvePendingConfiguration, String>),
    ShowDialog(Option<Html>),
    EditProperty(EditableProperty, Option<SubmitCallback<Value>>),
    AddProperty(EditableProperty, Option<SubmitCallback<Value>>),
    RevertProperty(EditableProperty),
    CommandResult(Result<(), Error>, String),
    Delete(String, Option<SubmitCallback<Value>>),
    Select(Option<Key>),
    Custom(M),
}

#[derive(Default)]
pub struct PendingPropertyViewState {
    pub data: Option<PvePendingConfiguration>,
    pub error: Option<String>,
    pub reload_timeout: Option<Timeout>,
    pub load_guard: Option<AsyncAbortGuard>,
    pub revert_guard: Option<AsyncAbortGuard>,
    pub async_pool: AsyncPool,
    pub dialog: Option<Html>,
}

pub trait PendingPropertyView: DerefMut<Target = PendingPropertyViewState> {
    type Properties: Properties;
    type Message;

    const MOBILE: bool;

    fn editor_loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>>;

    fn pending_loader(props: &Self::Properties)
        -> Option<ApiLoadCallback<PvePendingConfiguration>>;

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>>;

    fn create(ctx: &Context<PvePendingPropertyView<Self>>) -> Self
    where
        Self: 'static + Sized;

    #[allow(unused_variables)]
    fn update(&mut self, ctx: &Context<PvePendingPropertyView<Self>>, msg: Self::Message) -> bool
    where
        Self: 'static + Sized,
    {
        true
    }

    #[allow(unused_variables)]
    fn changed(
        &mut self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        old_props: &Self::Properties,
    ) -> bool
    where
        Self: 'static + Sized,
    {
        true
    }

    #[allow(unused_variables)]
    fn update_data(&mut self, ctx: &Context<PvePendingPropertyView<Self>>)
    where
        Self: 'static + Sized,
    {
    }

    fn view(&self, ctx: &Context<PvePendingPropertyView<Self>>) -> Html
    where
        Self: 'static + Sized;
}

impl PendingPropertyViewState {
    pub fn set_load_result(&mut self, result: Result<PvePendingConfiguration, String>) {
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
    state: T,
}

impl<T: 'static + PendingPropertyView> Component for PvePendingPropertyView<T> {
    type Message = PendingPropertyViewMsg<T::Message>;
    type Properties = T::Properties;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(PendingPropertyViewMsg::Load);
        let mut state = T::create(ctx);
        state.update_data(ctx);
        Self { state }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            PendingPropertyViewMsg::Custom(custom) => {
                return self.state.update(ctx, custom);
            }
            PendingPropertyViewMsg::Select(_key) => { /* just redraw */ }
            PendingPropertyViewMsg::Delete(name, on_submit) => {
                let link = ctx.link().clone();
                if let Some(on_submit) = on_submit.or_else(|| T::on_submit(props)) {
                    let param = json!({ "delete": [ name ] });
                    self.state.async_pool.spawn(async move {
                        let result = on_submit.apply(param).await;
                        link.send_message(PendingPropertyViewMsg::CommandResult(
                            result,
                            tr!("Delete property failed"),
                        ));
                    });
                }
            }
            PendingPropertyViewMsg::RevertProperty(property) => {
                let link = ctx.link().clone();
                let keys: Vec<String> = match property.revert_keys.as_deref() {
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
                    self.state.revert_guard = Some(AsyncAbortGuard::spawn(async move {
                        let result = on_submit.apply(param).await;
                        link.send_message(PendingPropertyViewMsg::CommandResult(result, tr!("")));
                    }));
                }
            }
            PendingPropertyViewMsg::CommandResult(result, message) => {
                if let Err(err) = result {
                    if T::MOBILE {
                        ctx.link().show_snackbar(
                            SnackBar::new().message(message + " - " + &err.to_string()),
                        );
                    } else {
                        self.state.dialog = Some(
                            AlertDialog::new(message + " - " + &err.to_string())
                                .on_close(
                                    ctx.link()
                                        .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                                )
                                .into(),
                        );
                    }
                }
                if self.state.reload_timeout.is_some() {
                    ctx.link().send_message(PendingPropertyViewMsg::Load);
                }
            }
            PendingPropertyViewMsg::EditProperty(property, on_submit) => {
                if property.render_input_panel.is_none() {
                    return false;
                }
                let dialog = PropertyEditDialog::from(property.clone())
                    .mobile(T::MOBILE)
                    .on_done(
                        ctx.link()
                            .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                    )
                    .loader(T::editor_loader(props))
                    .on_submit(on_submit.or_else(|| T::on_submit(props)))
                    .into();
                self.state.dialog = Some(dialog);
            }
            PendingPropertyViewMsg::AddProperty(property, on_submit) => {
                let dialog = PropertyEditDialog::from(property.clone())
                    .mobile(T::MOBILE)
                    .edit(false)
                    .on_done(
                        ctx.link()
                            .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                    )
                    .loader(T::editor_loader(props))
                    .on_submit(on_submit.or_else(|| T::on_submit(props)))
                    .into();
                self.state.dialog = Some(dialog);
            }
            PendingPropertyViewMsg::Load => {
                self.state.reload_timeout = None;
                let link = ctx.link().clone();
                if let Some(loader) = T::pending_loader(props) {
                    self.state.load_guard = Some(AsyncAbortGuard::spawn(async move {
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
                self.state.set_load_result(result);
                self.state.update_data(ctx);
                let link = ctx.link().clone();
                self.state.reload_timeout = Some(Timeout::new(3000, move || {
                    link.send_message(PendingPropertyViewMsg::Load);
                }));
            }
            PendingPropertyViewMsg::ShowDialog(dialog) => {
                if dialog.is_none() && self.state.reload_timeout.is_some() {
                    ctx.link().send_message(PendingPropertyViewMsg::Load);
                }
                self.state.dialog = dialog;
            }
        }
        true
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        self.state.changed(ctx, old_props)
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        self.state.view(ctx)
    }
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
/// and the set of contained configuration keys.
pub fn pve_pending_config_array_to_objects(
    data: Vec<PendingConfigValue>,
) -> (Value, Value, HashSet<String>) {
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

    (Value::Object(current), Value::Object(pending), keys)
}
