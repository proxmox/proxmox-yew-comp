use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use html::IntoPropValue;
use proxmox_client::ApiResponseData;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader, DataTableMouseEvent};
use pwt::widget::form::{delete_empty_values, DisplayField, Field, FormContext, Number, TextArea};
use pwt::widget::{Button, InputPanel, Toolbar};

use pwt_macros::builder;

use crate::percent_encoding::percent_encode_component;
use crate::{
    http_get, ConfirmButton, EditWindow, LoadableComponent, LoadableComponentContext,
    LoadableComponentLink, LoadableComponentMaster,
};

use super::{AcmeChallengeSchemaItem, AcmeChallengeSelector};

pub(crate) async fn load_acme_plugin_list(url: AttrValue) -> Result<Vec<PluginConfig>, Error> {
    let data: Vec<PluginConfig> = crate::http_get(&*url, None).await?;
    let data = data
        .into_iter()
        .filter(|item| item.ty == "dns" && item.api.is_some())
        .collect();
    Ok(data)
}

// An ACME Plugin list entry.
#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct PluginConfig {
    /// Plugin ID.
    pub plugin: String,

    /// Plugin type.
    #[serde(rename = "type")]
    pub ty: String,

    /// DNS Api name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
}

#[derive(PartialEq, Clone, Properties)]
#[builder]
pub struct AcmePluginsPanel {
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(AttrValue::Static("/config/acme/plugins"))]
    url: AttrValue,

    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(AttrValue::Static("/config/acme/challenge-schema"))]
    challenge_shema_url: AttrValue,
}

impl AcmePluginsPanel {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

struct ChallengeSchemaInfo {
    schema_name_map: Rc<HashMap<String, String>>,
    store: Store<AcmeChallengeSchemaItem>,
}

#[doc(hidden)]
pub struct ProxmoxAcmePluginsPanel {
    selection: Selection,
    store: Store<PluginConfig>,
    columns: Rc<Vec<DataTableHeader<PluginConfig>>>,
    challenge_schema: Option<AcmeChallengeSchemaItem>,
    api_data: String,
    schema_info: ChallengeSchemaInfo,
}

#[derive(PartialEq)]
pub enum ViewState {
    Add,
    Edit(Key),
}

pub enum Msg {
    Redraw,
    CloseDialog,
    Add,
    Edit(Key),
    ChallengeSchema(Option<AcmeChallengeSchemaItem>),
    ApiData(String),
    LoadChallengeSchemaList,
    UpdateChallengeSchemaList(Result<Vec<AcmeChallengeSchemaItem>, Error>),
}

impl ProxmoxAcmePluginsPanel {
    fn update_challenge_info(&mut self, list: Vec<AcmeChallengeSchemaItem>) {
        let mut map = HashMap::new();
        for item in list.iter() {
            if let Value::String(ref name) = item.schema["name"] {
                map.insert(item.id.clone(), name.clone());
            }
        }
        self.schema_info.schema_name_map = Rc::new(map);
        self.schema_info.store.set_data(list);
    }
}

impl LoadableComponent for ProxmoxAcmePluginsPanel {
    type Properties = AcmePluginsPanel;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::Redraw));
        let store =
            Store::with_extract_key(|record: &PluginConfig| Key::from(record.plugin.clone()));

        let schema_name_map = Rc::new(HashMap::new());
        let columns = columns(schema_name_map.clone());

        ctx.link().send_message(Msg::LoadChallengeSchemaList);

        Self {
            selection,
            store,
            columns,
            challenge_schema: None,
            api_data: String::new(),
            schema_info: ChallengeSchemaInfo {
                schema_name_map,
                store: Store::new(),
            },
        }
    }

    fn load(
        &self,
        ctx: &crate::LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let props = ctx.props();
        let store = self.store.clone();
        let url = props.url.clone();
        Box::pin(async move {
            let data = load_acme_plugin_list(url).await?;
            store.write().set_data(data);
            Ok(())
        })
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Redraw => true,
            Msg::Add => {
                self.challenge_schema = None;
                self.api_data = String::new();
                ctx.link().change_view(Some(ViewState::Add));
                false
            }
            Msg::Edit(key) => {
                self.challenge_schema = None;
                self.api_data = String::new();

                // avoid flickering by setting the correct challenge_schema
                // before opening the edit dialog
                if let Some(plugin) = self
                    .store
                    .read()
                    .iter()
                    .find(|item| item.plugin == &*key && item.ty == "dns")
                    .cloned()
                {
                    if let Some(api) = &plugin.api {
                        if let Some(schema) = self
                            .schema_info
                            .store
                            .read()
                            .iter()
                            .find(|item| item.id == *api)
                            .cloned()
                        {
                            self.challenge_schema = Some(schema);
                        }
                    }
                }
                ctx.link().change_view(Some(ViewState::Edit(key)));
                false
            }
            Msg::ChallengeSchema(schema) => {
                self.challenge_schema = schema;
                true
            }
            Msg::ApiData(api_data) => {
                self.api_data = api_data;
                true
            }
            Msg::CloseDialog => {
                self.challenge_schema = None;
                self.api_data = String::new();
                ctx.link().change_view(None);
                ctx.link().send_reload();
                true
            }
            Msg::LoadChallengeSchemaList => {
                let url = ctx.props().challenge_shema_url.clone();
                let link = ctx.link();
                wasm_bindgen_futures::spawn_local(async move {
                    let result = http_get(&*url, None).await;
                    link.send_message(Msg::UpdateChallengeSchemaList(result));
                });
                false
            }
            Msg::UpdateChallengeSchemaList(result) => {
                // fixme: handle errors
                if let Ok(list) = result {
                    self.update_challenge_info(list);
                    self.columns = columns(self.schema_info.schema_name_map.clone());
                }
                true
            }
        }
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let selected_key = self.selection.selected_key();

        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(Button::new(tr!("Add")).onclick(ctx.link().callback(|_| Msg::Add)))
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(selected_key.is_none())
                    .onclick({
                        let link = ctx.link();
                        let selected_key = selected_key.clone();
                        move |_| {
                            if let Some(selected_key) = &selected_key {
                                link.send_message(Msg::Edit(selected_key.clone()));
                            }
                        }
                    }),
            )
            .with_child(
                ConfirmButton::remove_entry(selected_key.as_deref().unwrap_or("").to_string())
                    .disabled(selected_key.is_none())
                    .on_activate({
                        let link = ctx.link();
                        move |_| {
                            let link = link.clone();
                            if let Some(selected_key) = &selected_key {
                                let command_path = format!(
                                    "/config/acme/plugins/{}",
                                    percent_encode_component(&*selected_key)
                                );
                                let command_future = crate::http_delete(command_path, None);
                                wasm_bindgen_futures::spawn_local(async move {
                                    match command_future.await {
                                        Ok(()) => {
                                            link.send_reload();
                                        }
                                        Err(err) => {
                                            link.show_error(tr!("Error"), err, true);
                                        }
                                    }
                                });
                            }
                        }
                    }),
            );

        Some(toolbar.into())
    }

    fn main_view(&self, ctx: &crate::LoadableComponentContext<Self>) -> Html {
        DataTable::new(self.columns.clone(), self.store.clone())
            .class("pwt-flex-fit")
            .selection(self.selection.clone())
            .on_row_dblclick({
                let store = self.store.clone();
                let link = ctx.link();
                move |event: &mut DataTableMouseEvent| {
                    let key = &event.record_key;
                    if store.read().lookup_record(key).is_some() {
                        link.send_message(Msg::Edit(key.clone()));
                    };
                }
            })
            .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        match view_state {
            ViewState::Add => Some(self.create_add_dns_plugin_dialog(ctx)),
            ViewState::Edit(id) => Some(self.create_edit_dns_plugin_dialog(ctx, &*id)),
        }
    }
}

impl Into<VNode> for AcmePluginsPanel {
    fn into(self) -> VNode {
        let comp =
            VComp::new::<LoadableComponentMaster<ProxmoxAcmePluginsPanel>>(Rc::new(self), None);
        VNode::from(comp)
    }
}

impl ProxmoxAcmePluginsPanel {
    fn dns_plugin_input_panel(
        link: &LoadableComponentLink<Self>,
        _form_ctx: &FormContext,
        id: Option<&str>,
        challenge_schema: Option<&AcmeChallengeSchemaItem>,
        api_data: &str,
        challenge_store: Store<AcmeChallengeSchemaItem>,
    ) -> InputPanel {
        let mut panel = InputPanel::new()
            .width(600)
            .class("pwt-flex-fit")
            .padding(4);

        if let Some(id) = id {
            panel.add_field(false, tr!("Plugin ID"), DisplayField::new(id.to_string()));
        } else {
            panel.add_field(
                false,
                tr!("Plugin ID"),
                Field::new()
                    .name("plugin")
                    .disabled(id.is_some())
                    .submit(id.is_none())
                    .required(true),
            );
        }

        let mut panel = panel
            .with_field(
                tr!("Validation Delay"),
                Number::<u8>::new()
                    .name("validation-delay")
                    .max(48)
                    .placeholder("30"),
            )
            .with_field(
                tr!("DNS API"),
                AcmeChallengeSelector::with_store(challenge_store)
                    .name("api")
                    .required(true)
                    .on_change(link.callback(move |schema| Msg::ChallengeSchema(schema))),
            );

        if let Some(description) =
            challenge_schema.and_then(|schema| schema.schema["description"].as_str())
        {
            panel.add_field(
                false,
                tr!("Hint"),
                DisplayField::new(description.to_string()).key("__hint__"),
            );
        }

        let field_list = challenge_schema
            .and_then(|challenge_schema| challenge_schema.schema["fields"].as_object());

        if let Some(field_list) = field_list {
            for (field_name, field_schema) in field_list {
                let parsed_data = Self::parse_plugin_data(api_data);
                let default = parsed_data.get(field_name).map(|s| s.to_owned());
                let description: Option<String> =
                    field_schema["description"].as_str().map(|s| s.to_owned());
                let placeholder = field_schema["default"].as_str().map(|s| s.to_owned());

                panel.add_field(
                    false,
                    format!("{}=", field_name),
                    Field::new()
                        .name(format!("data_{}", field_name))
                        .tip(description)
                        .submit(false)
                        .placeholder(placeholder)
                        .default(default),
                )
            }
        } else {
            panel.add_field(
                false,
                tr!("API Data"),
                TextArea::new()
                    .name("data")
                    .default(api_data.to_owned())
                    .class("pwt-w-100")
                    .submit_empty(true)
                    .submit(false)
                    .attribute("rows", "4"),
            );
        }
        panel
    }

    fn parse_plugin_data(data: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for line in data.lines() {
            if let Some((key, value)) = line.split_once('=') {
                map.insert(key.to_owned(), value.to_owned());
            }
        }
        map
    }

    fn assemble_api_data(
        form_ctx: &FormContext,
        challenge_schema: Option<&AcmeChallengeSchemaItem>,
    ) -> String {
        let field_list = challenge_schema
            .and_then(|challenge_schema| challenge_schema.schema["fields"].as_object());

        let form_ctx = form_ctx.read();
        if let Some(field_list) = field_list {
            let mut api_data = Vec::new();
            for (field_name, _field_schema) in field_list {
                let value = form_ctx.get_field_text(format!("data_{field_name}"));
                let value = value.trim();
                if !value.is_empty() {
                    api_data.push(format!("{field_name}={value}"))
                }
            }
            base64::encode(api_data.join("\n"))
        } else {
            base64::encode(form_ctx.get_field_text("data"))
        }
    }

    fn create_edit_dns_plugin_dialog(
        &self,
        ctx: &crate::LoadableComponentContext<Self>,
        id: &str,
    ) -> Html {
        let url = format!("{}/{}", ctx.props().url, percent_encode_component(id));
        EditWindow::new(tr!("Edit") + ": " + &tr!("ACME DNS Plugin"))
            .loader((
                {
                    let link = ctx.link();
                    move |url: AttrValue| {
                        let url = url.clone();
                        let link = link.clone();
                        async move {
                            let resp: ApiResponseData<Value> =
                                crate::http_get_full(&*url, None).await?;
                            let api_data = resp.data["data"].as_str().unwrap_or("");
                            link.send_message(Msg::ApiData(api_data.to_owned()));
                            Ok(resp)
                        }
                    }
                },
                url.clone(),
            ))
            .on_done(ctx.link().callback(|_| Msg::CloseDialog))
            .renderer({
                let id = id.to_owned();
                let link = ctx.link();
                let challenge_schema = self.challenge_schema.clone();
                let api_data = self.api_data.clone();
                let challenge_store = self.schema_info.store.clone();
                move |form_ctx: &FormContext| {
                    Self::dns_plugin_input_panel(
                        &link,
                        form_ctx,
                        Some(&id),
                        challenge_schema.as_ref(),
                        &api_data,
                        challenge_store.clone(),
                    )
                    .into()
                }
            })
            .on_submit({
                let challenge_schema = self.challenge_schema.clone();
                move |form_ctx: FormContext| {
                    let mut data = form_ctx.get_submit_data();
                    let url = url.clone();

                    data["data"] =
                        Self::assemble_api_data(&form_ctx, challenge_schema.as_ref()).into();

                    let data = delete_empty_values(&data, &["validation-delay"], true);

                    async move {
                        crate::http_put(&url, Some(data)).await?;
                        Ok(())
                    }
                }
            })
            .into()
    }

    fn create_add_dns_plugin_dialog(&self, ctx: &crate::LoadableComponentContext<Self>) -> Html {
        EditWindow::new(tr!("Add") + ": " + &tr!("ACME DNS Plugin"))
            .on_done(ctx.link().callback(|_| Msg::CloseDialog))
            .renderer({
                let link = ctx.link();
                let challenge_schema = self.challenge_schema.clone();
                let api_data = self.api_data.clone();
                let challenge_store = self.schema_info.store.clone();
                move |form_ctx: &FormContext| {
                    Self::dns_plugin_input_panel(
                        &link,
                        form_ctx,
                        None,
                        challenge_schema.as_ref(),
                        &api_data,
                        challenge_store.clone(),
                    )
                    .into()
                }
            })
            .on_submit({
                let challenge_schema = self.challenge_schema.clone();
                move |form_ctx: FormContext| {
                    let mut data = form_ctx.get_submit_data();
                    data["type"] = "dns".into();
                    data["id"] = data
                        .as_object_mut()
                        .unwrap()
                        .remove("plugin")
                        .unwrap_or(Value::Null);
                    data["data"] =
                        Self::assemble_api_data(&form_ctx, challenge_schema.as_ref()).into();

                    async move {
                        crate::http_post("/config/acme/plugins", Some(data)).await?;
                        Ok(())
                    }
                }
            })
            .into()
    }
}

fn columns(
    schema_name_hash: Rc<HashMap<String, String>>,
) -> Rc<Vec<DataTableHeader<PluginConfig>>> {
    Rc::new(vec![
        DataTableColumn::new(tr!("Plugin"))
            .flex(1)
            .render(|record: &PluginConfig| html! { &record.plugin })
            .sorter(|a: &PluginConfig, b: &PluginConfig| a.plugin.cmp(&b.plugin))
            .sort_order(true)
            .into(),
        DataTableColumn::new(tr!("API"))
            .flex(1)
            .render(move |record: &PluginConfig| {
                let text = match &record.api {
                    Some(api) => match schema_name_hash.get(api) {
                        Some(name) => name.clone(),
                        None => api.clone(),
                    },
                    None => String::new(),
                };
                html! {text}
            })
            .into(),
    ])
}
