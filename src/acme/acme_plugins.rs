use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader, DataTableMouseEvent};
use pwt::widget::form::{delete_empty_values, Field, FormContext, Number, TextArea};
use pwt::widget::{Button, InputPanel, Toolbar};

use crate::percent_encoding::percent_encode_component;
use crate::{
    ConfirmButton, EditWindow, LoadableComponent, LoadableComponentContext, LoadableComponentLink,
    LoadableComponentMaster,
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

#[derive(PartialEq, Properties)]
pub struct AcmePluginsPanel {
    #[prop_or(AttrValue::Static("/config/acme/plugins"))]
    url: AttrValue,
}

impl AcmePluginsPanel {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[doc(hidden)]
pub struct ProxmoxAcmePluginsPanel {
    selection: Selection,
    store: Store<PluginConfig>,
    columns: Rc<Vec<DataTableHeader<PluginConfig>>>,
    challenge_schema: Option<AcmeChallengeSchemaItem>,
    api_data: String,
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
}

impl LoadableComponent for ProxmoxAcmePluginsPanel {
    type Properties = AcmePluginsPanel;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::Redraw));
        let store =
            Store::with_extract_key(|record: &PluginConfig| Key::from(record.plugin.clone()));

        let columns = Rc::new(vec![
            DataTableColumn::new(tr!("Plugin"))
                .flex(1)
                .render(|record: &PluginConfig| html! { &record.plugin })
                .sorter(|a: &PluginConfig, b: &PluginConfig| a.plugin.cmp(&b.plugin))
                .sort_order(true)
                .into(),
            DataTableColumn::new(tr!("API"))
                .flex(1)
                .render(|record: &PluginConfig| {
                    let text = match &record.api {
                        Some(api) => api,
                        None => "",
                    };
                    html! {text}
                })
                .into(),
        ]);

        Self {
            selection,
            store,
            columns,
            challenge_schema: None,
            api_data: String::new(),
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
                Button::new(tr!("View"))
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
    ) -> InputPanel {
        let mut panel = InputPanel::new()
            .width(600)
            .class("pwt-flex-fit")
            .padding(4)
            .with_field(
                tr!("Plugin ID"),
                Field::new()
                    .name("plugin")
                    .disabled(id.is_some())
                    .submit(id.is_none())
                    .required(true),
            )
            .with_field(
                tr!("Validation Delay"),
                Number::<u8>::new()
                    .name("validation-delay")
                    .max(48)
                    .placeholder("30"),
            )
            .with_field(
                tr!("DNS API"),
                AcmeChallengeSelector::new()
                    .name("api")
                    .required(true)
                    .on_change(link.callback(move |schema| Msg::ChallengeSchema(schema))),
            );

        let field_list = challenge_schema
            .and_then(|challenge_schema| challenge_schema.schema["fields"].as_object());

        if let Some(field_list) = field_list {
            for (field_name, field_schema) in field_list {
                let parsed_data = Self::parse_plugin_data(api_data);
                let default = parsed_data.get(field_name).map(|s| s.to_owned());
                let description: Option<String> =
                    field_schema["description"].as_str().map(|s| s.to_owned());

                panel.add_field(
                    false,
                    format!("{}=", field_name),
                    Field::new()
                        .name(format!("data_{}", field_name))
                        .tip(description)
                        .submit(false)
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
        let url = format!("/config/acme/plugins/{}", percent_encode_component(id));
        EditWindow::new(tr!("Edit") + ": " + &tr!("ACME DNS Plugin"))
            .loader((
                {
                    let link = ctx.link();
                    move |url: AttrValue| {
                        let url = url.clone();
                        let link = link.clone();
                        async move {
                            let data: Value = crate::http_get(&*url, None).await?;
                            let api_data = data["data"].as_str().unwrap_or("");
                            link.send_message(Msg::ApiData(api_data.to_owned()));
                            Ok(data)
                        }
                    }
                },
                url,
            ))
            .on_done(ctx.link().callback(|_| Msg::CloseDialog))
            .renderer({
                let id = id.to_owned();
                let link = ctx.link();
                let challenge_schema = self.challenge_schema.clone();
                let api_data = self.api_data.clone();
                move |form_ctx: &FormContext| {
                    Self::dns_plugin_input_panel(
                        &link,
                        form_ctx,
                        Some(&id),
                        challenge_schema.as_ref(),
                        &api_data,
                    )
                    .into()
                }
            })
            .on_submit({
                let challenge_schema = self.challenge_schema.clone();
                move |form_ctx: FormContext| {
                    let mut data = form_ctx.get_submit_data();

                    data["data"] =
                        Self::assemble_api_data(&form_ctx, challenge_schema.as_ref()).into();
                    data["type"] = "dns".into();

                    let data = delete_empty_values(&data, &["validation-delay"], true);

                    let plugin = form_ctx.read().get_field_text("plugin");

                    async move {
                        crate::http_put(format!("/config/acme/plugins/{plugin}"), Some(data))
                            .await?;
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
                move |form_ctx: &FormContext| {
                    Self::dns_plugin_input_panel(
                        &link,
                        form_ctx,
                        None,
                        challenge_schema.as_ref(),
                        &api_data,
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
