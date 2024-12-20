use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use html::IntoPropValue;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader, DataTableMouseEvent};
use pwt::widget::form::{delete_empty_values, DisplayField, Field, FormContext, Number, TextArea};
use pwt::widget::{Button, InputPanel, Toolbar};
use pwt::{prelude::*, AsyncPool};

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

impl Default for AcmePluginsPanel {
    fn default() -> Self {
        Self::new()
    }
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
    schema_info: ChallengeSchemaInfo,
    async_pool: AsyncPool,
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
    Delete(Option<Key>),
    ChallengeSchema(Option<AcmeChallengeSchemaItem>),
    ApiDataChange(FormContext, String, String),
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
            schema_info: ChallengeSchemaInfo {
                schema_name_map,
                store: Store::new(),
            },
            async_pool: AsyncPool::new(),
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
                ctx.link().change_view(Some(ViewState::Add));
                false
            }
            Msg::Edit(key) => {
                self.challenge_schema = None;
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
            Msg::ApiDataChange(form_ctx, name, value) => {
                Self::update_api_data(
                    &form_ctx,
                    self.challenge_schema.as_ref(),
                    Some((name, value)),
                );
                true
            }
            Msg::CloseDialog => {
                self.challenge_schema = None;
                ctx.link().change_view(None);
                ctx.link().send_reload();
                true
            }
            Msg::LoadChallengeSchemaList => {
                let url = ctx.props().challenge_shema_url.clone();
                let link = ctx.link();
                self.async_pool.spawn(async move {
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
            Msg::Delete(selected_key) => {
                if let Some(selected_key) = &selected_key {
                    let command_path = format!(
                        "{}/{}",
                        ctx.props().url,
                        percent_encode_component(&*selected_key)
                    );
                    let command_future = crate::http_delete(command_path, None);
                    let link = ctx.link().clone();
                    self.async_pool.spawn(async move {
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

                false
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
                    .on_activate(ctx.link().callback({
                        let selected_key = selected_key.clone();
                        move |_| Msg::Delete(selected_key.clone())
                    })),
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

impl From<AcmePluginsPanel> for VNode {
    fn from(val: AcmePluginsPanel) -> Self {
        let comp =
            VComp::new::<LoadableComponentMaster<ProxmoxAcmePluginsPanel>>(Rc::new(val), None);
        VNode::from(comp)
    }
}

impl ProxmoxAcmePluginsPanel {
    fn update_api_data(
        form_ctx: &FormContext,
        challenge_schema: Option<&AcmeChallengeSchemaItem>,
        insert: Option<(String, String)>,
    ) {
        let api_data = form_ctx.read().get_field_text("data");
        let mut parsed_data = Self::parse_plugin_data(&api_data);

        if let Some((name, value)) = insert {
            // just add the new data
            parsed_data.insert(name, value);
            let mut api_data = Vec::new();
            for (field_name, value) in parsed_data {
                api_data.push(format!("{field_name}={value}"));
            }
            let api_data = api_data.join("\n");
            form_ctx
                .write()
                .set_field_value("data", api_data.clone().into());
        } else {
            let field_list = challenge_schema
                .and_then(|challenge_schema| challenge_schema.schema["fields"].as_object());

            // only add data from known fields
            if let Some(field_list) = field_list {
                let mut api_data = Vec::new();
                for (field_name, _field_schema) in field_list {
                    if let Some(value) = parsed_data.get(field_name) {
                        let value = value.trim();
                        if !value.is_empty() {
                            api_data.push(format!("{field_name}={value}"))
                        }
                    }
                }
                let api_data = api_data.join("\n");
                form_ctx
                    .write()
                    .set_field_value("data", api_data.clone().into());
            }
        }
    }

    fn dns_plugin_input_panel(
        link: &LoadableComponentLink<Self>,
        form_ctx: &FormContext,
        id: Option<&str>,
        challenge_schema: Option<&AcmeChallengeSchemaItem>,
        challenge_store: Store<AcmeChallengeSchemaItem>,
    ) -> InputPanel {
        let api_data = form_ctx.read().get_field_text("data");

        let mut panel = InputPanel::new()
            .width(600)
            .class("pwt-flex-fit")
            .padding(4);

        if let Some(id) = id {
            panel.add_field(tr!("Plugin ID"), DisplayField::new(id.to_string()));
        } else {
            panel.add_field(
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
                tr!("Hint"),
                DisplayField::new(description.to_string()).key("__hint__"),
            );
        }

        let field_list = challenge_schema
            .and_then(|challenge_schema| challenge_schema.schema["fields"].as_object());

        panel.add_field_with_options(
            pwt::widget::FieldPosition::Left,
            false,
            field_list.is_some(),
            tr!("API Data"),
            TextArea::new()
                .name("data")
                .class("pwt-w-100")
                .submit_empty(true)
                .submit(false)
                .attribute("rows", "4"),
        );

        if let Some(field_list) = field_list {
            for (field_name, field_schema) in field_list {
                let parsed_data = Self::parse_plugin_data(&api_data);
                let value = parsed_data
                    .get(field_name)
                    .map(|s| s.to_owned())
                    .unwrap_or(String::new());
                let description: Option<String> =
                    field_schema["description"].as_str().map(|s| s.to_owned());
                let placeholder = field_schema["default"].as_str().map(|s| s.to_owned());

                panel.add_field(
                    format!("{}=", field_name),
                    Field::new()
                        .key(format!("data_{}", field_name))
                        .tip(description)
                        .submit(false)
                        .placeholder(placeholder)
                        .value(value)
                        .on_change({
                            let field_name = field_name.clone();
                            let form_ctx = form_ctx.clone();
                            link.callback(move |v| {
                                Msg::ApiDataChange(form_ctx.clone(), field_name.clone(), v)
                            })
                        }),
                )
            }
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

    fn create_edit_dns_plugin_dialog(
        &self,
        ctx: &crate::LoadableComponentContext<Self>,
        id: &str,
    ) -> Html {
        let url = format!("{}/{}", ctx.props().url, percent_encode_component(id));
        EditWindow::new(tr!("Edit") + ": " + &tr!("ACME DNS Plugin"))
            .loader((
                move |url: AttrValue| crate::http_get_full(url.to_string(), None),
                url.clone(),
            ))
            .on_done(ctx.link().callback(|_| Msg::CloseDialog))
            .renderer({
                let id = id.to_owned();
                let link = ctx.link();
                let challenge_schema = self.challenge_schema.clone();
                let challenge_store = self.schema_info.store.clone();
                move |form_ctx: &FormContext| {
                    Self::dns_plugin_input_panel(
                        &link,
                        form_ctx,
                        Some(&id),
                        challenge_schema.as_ref(),
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

                    Self::update_api_data(&form_ctx, challenge_schema.as_ref(), None);
                    data["data"] = base64::encode(form_ctx.read().get_field_text("data")).into();

                    let data = delete_empty_values(&data, &["validation-delay"], true);

                    async move { crate::http_put(&url, Some(data)).await }
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
                let challenge_store = self.schema_info.store.clone();
                move |form_ctx: &FormContext| {
                    Self::dns_plugin_input_panel(
                        &link,
                        form_ctx,
                        None,
                        challenge_schema.as_ref(),
                        challenge_store.clone(),
                    )
                    .into()
                }
            })
            .on_submit({
                let challenge_schema = self.challenge_schema.clone();
                move |form_ctx: FormContext| {
                    Self::update_api_data(&form_ctx, challenge_schema.as_ref(), None);

                    let mut data = form_ctx.get_submit_data();
                    data["type"] = "dns".into();
                    data["id"] = data
                        .as_object_mut()
                        .unwrap()
                        .remove("plugin")
                        .unwrap_or(Value::Null);

                    data["data"] = base64::encode(form_ctx.read().get_field_text("data")).into();

                    async move { crate::http_post("/config/acme/plugins", Some(data)).await }
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
