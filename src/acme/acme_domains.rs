use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::{bail, Error};
use serde_json::{json, Value};
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader, DataTableMouseEvent};
use pwt::widget::form::{Field, FormContext};
use pwt::widget::{ActionIcon, Button, InputPanel, Toolbar, Tooltip};

use crate::common_api_types::{
    create_acme_config_string, parse_acme_config_string, AcmeConfig,
};
use crate::common_api_types::{
    create_acme_domain_string, parse_acme_domain_string, AcmeDomain,
};
use crate::percent_encoding::percent_encode_component;
use crate::{ConfirmButton, EditWindow};
use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster};

use super::{AcmeAccountSelector, AcmeChallengeTypeSelector, AcmePluginSelector};

#[derive(Clone, PartialEq)]
struct AcmeDomainEntry {
    config_key: String,
    config_type: &'static str,
    config: AcmeDomain,
}

#[derive(PartialEq, Properties)]
pub struct AcmeDomainsPanel {}

impl AcmeDomainsPanel {
    pub fn new() -> Self {
        Self {}
    }
}

#[doc(hidden)]
pub struct ProxmoxAcmeDomainsPanel {
    selection: Selection,
    store: Store<AcmeDomainEntry>,
    columns: Rc<Vec<DataTableHeader<AcmeDomainEntry>>>,
    acme_account: Option<AcmeConfig>,
}

pub enum Msg {
    Redraw,
    AcmeAccount(Option<AcmeConfig>),
}

#[derive(PartialEq)]
pub enum ViewState {
    Add,
    Edit(Key),
    EditAccount,
}

impl LoadableComponent for ProxmoxAcmeDomainsPanel {
    type Properties = AcmeDomainsPanel;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::Redraw));
        let store = Store::with_extract_key(|record: &AcmeDomainEntry| {
            Key::from(record.config_key.clone())
        });

        let columns = Rc::new(vec![
            DataTableColumn::new(tr!("Name"))
                .flex(1)
                .render(|record: &AcmeDomainEntry| html! { &record.config.domain })
                .sorter(|a: &AcmeDomainEntry, b: &AcmeDomainEntry| a.config.domain.cmp(&b.config.domain))
                .sort_order(true)
                .into(),
            DataTableColumn::new(tr!("Type"))
                .width("150px")
                .render(|record: &AcmeDomainEntry| html! { record.config_type })
                .sorter(|a: &AcmeDomainEntry, b: &AcmeDomainEntry| a.config_type.cmp(&b.config_type))
                .into(),
            DataTableColumn::new(tr!("Plugin"))
                .width("150px")
                .render(|record: &AcmeDomainEntry| html! { record.config.plugin.as_deref().unwrap_or("")})
                .sorter(|a: &AcmeDomainEntry, b: &AcmeDomainEntry| {
                    let a = a.config.plugin.as_deref().unwrap_or("");
                    let b = b.config.plugin.as_deref().unwrap_or("");
                    a.cmp(b)
                })
                .into(),
        ]);

        ctx.link().repeated_load(3000);

        Self {
            selection,
            store,
            columns,
            acme_account: None,
        }
    }

    fn load(
        &self,
        ctx: &crate::LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let store = self.store.clone();
        let link = ctx.link();
        Box::pin(async move {
            let data: Value = crate::http_get("/nodes/localhost/config", None).await?;

            let mut domain_list: Vec<AcmeDomainEntry> = Vec::new();
            for i in 0..5 {
                let key = format!("acmedomain{i}");
                if let Some(acme_domain_string) = data[&key].as_str() {
                    let config = parse_acme_domain_string(acme_domain_string)?;
                    domain_list.push(AcmeDomainEntry {
                        config_key: key,
                        config_type: if config.plugin.is_some() {
                            "dns"
                        } else {
                            "standalone"
                        },
                        config,
                    });
                }
            }

            store.write().set_data(domain_list);

            if let Some(Value::String(acme_account)) = data.get("acme") {
                let acme_account = parse_acme_config_string(acme_account)?;
                link.send_message(Msg::AcmeAccount(Some(acme_account)));
            } else {
                link.send_message(Msg::AcmeAccount(None));
            }
            Ok(())
        })
    }

    fn update(&mut self, _ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Redraw => true,
            Msg::AcmeAccount(acme_account) => {
                self.acme_account = acme_account;
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
            .with_child(
                Button::new(tr!("Add"))
                    .onclick(ctx.link().change_view_callback(|_| Some(ViewState::Add))),
            )
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(selected_key.is_none())
                    .onclick({
                        let link = ctx.link();
                        let selected_key = selected_key.clone();
                        move |_| {
                            if let Some(selected_key) = &selected_key {
                                link.change_view(Some(ViewState::Edit(selected_key.clone())));
                            }
                        }
                    }),
            )
            .with_child(
                ConfirmButton::remove_entry(selected_key.as_deref().unwrap_or("").to_string())
                    .disabled(selected_key.is_none())
                    .on_activate({
                        let link = ctx.link();
                        let selected_key = selected_key.clone();
                        move |_| {
                            let link = link.clone();
                            if let Some(selected_key) = &selected_key {
                                let url = "/nodes/localhost/config";
                                let data = json!({ "delete": &[ selected_key.to_string()] });
                                let command_future = crate::http_put(url, Some(data));
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
            )
            .with_flex_spacer()
            .with_child({
                let acme_account = self
                    .acme_account
                    .as_ref()
                    .map(|config| config.account.as_str())
                    .unwrap_or("default");

                let icon = ActionIcon::new("fa fa-pencil").on_activate(
                    ctx.link()
                        .change_view_callback(|_| Some(ViewState::EditAccount)),
                );

                let icon = Tooltip::new(icon)
                    .class("pwt-d-inline")
                    .tip(tr!("Edit account settings"));

                html! {<div>
                    <span>{tr!("Using Account") + ": " + acme_account}</span>
                    <span class="pwt-ps-2">{icon}</span>
                </div>}
            })
            .with_child(Button::new(tr!("Order Certificate Now")).onclick({
                let link = ctx.link();
                move |_| {
                    let command_path = "/nodes/localhost/certificates/acme/certificate";
                    link.start_task(command_path, None, false);
                }
            }));

        Some(toolbar.into())
    }

    fn main_view(&self, ctx: &crate::LoadableComponentContext<Self>) -> Html {
        DataTable::new(self.columns.clone(), self.store.clone())
            .class("pwt-flex-fit")
            .selection(self.selection.clone())
            .on_row_dblclick({
                let link = ctx.link();
                move |event: &mut DataTableMouseEvent| {
                    let key = &event.record_key;
                    link.change_view(Some(ViewState::Edit(key.clone())));
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
            ViewState::Add => Some(self.create_add_acme_domain_dialog(ctx)),
            ViewState::Edit(domain_name) => {
                Some(self.create_edit_acme_domain_dialog(ctx, &*domain_name))
            }
            ViewState::EditAccount => Some(self.create_edit_acme_account_dialog(ctx)),
        }
    }
}

impl Into<VNode> for AcmeDomainsPanel {
    fn into(self) -> VNode {
        let comp =
            VComp::new::<LoadableComponentMaster<ProxmoxAcmeDomainsPanel>>(Rc::new(self), None);
        VNode::from(comp)
    }
}

impl ProxmoxAcmeDomainsPanel {
    fn acme_domain_input_panel(form_ctx: &FormContext) -> InputPanel {
        let challenge_type = form_ctx.read().get_field_text("type");

        let mut panel = InputPanel::new().class("pwt-flex-fit pwt-p-4").with_field(
            tr!("Challenge Type"),
            AcmeChallengeTypeSelector::new()
                .name("type")
                .required(true)
                .default(AttrValue::Static("HTTP")),
        );

        if challenge_type == "DNS" {
            panel.add_field(
                false,
                tr!("Plugin"),
                AcmePluginSelector::new().name("plugin").required(true),
            );
        }

        panel.add_field(
            false,
            tr!("Domain"),
            Field::new().name("domain").required(true),
        );

        panel
    }

    async fn update_acme_domain(
        form_ctx: FormContext,
        config_key: Option<String>,
    ) -> Result<(), Error> {
        let config_key = match config_key {
            Some(key) => key,
            None => bail!(tr!(
                "It is not possible to configure more that 5 ACME domain."
            )),
        };

        let acme_domain = form_ctx.get_submit_data();
        let acme_domain: AcmeDomain = serde_json::from_value(acme_domain)?;
        let mut data = json!({});
        data[config_key] = create_acme_domain_string(&acme_domain).into();
        crate::http_put("/nodes/localhost/config", Some(data)).await?;
        Ok(())
    }

    fn create_add_acme_domain_dialog(
        &self,
        ctx: &crate::LoadableComponentContext<Self>,
    ) -> Html {
        let mut next_key = None;
        {
            let store = self.store.read();
            for i in 0..5 {
                let key = format!("acmedomain{i}");
                if store.iter().find(|item| item.config_key == key).is_some() {
                    continue;
                }
                next_key = Some(key);
                break;
            }
        }

        EditWindow::new(tr!("Add") + ": " + &tr!("ACME Domain"))
            .on_done(ctx.link().change_view_callback(|_| None))
            .renderer(move |form_ctx: &FormContext| Self::acme_domain_input_panel(form_ctx).into())
            .on_submit(move |form_ctx: FormContext| {
                Self::update_acme_domain(form_ctx, next_key.clone())
            })
            .into()
    }

    fn create_edit_acme_domain_dialog(
        &self,
        ctx: &crate::LoadableComponentContext<Self>,
        config_key: &str,
    ) -> Html {
        // this url does not exists - it is used only for change tracking
        let fake_url = format!(
            "/nodes/localhost/config/acme_domains/{}",
            percent_encode_component(config_key)
        );

        EditWindow::new(tr!("Edit") + ": " + &tr!("ACME Domain"))
            .on_done(ctx.link().change_view_callback(|_| None))
            .renderer({
                move |form_ctx: &FormContext| Self::acme_domain_input_panel(form_ctx).into()
            })
            .loader((
                {
                    let config_key = config_key.to_owned();
                    move |_fake_url: AttrValue| {
                        let config_key = config_key.clone();
                        async move {
                            let data: Value =
                                crate::http_get("/nodes/localhost/config", None).await?;
                            if let Some(acme_config_string) = data[&config_key].as_str() {
                                let config = parse_acme_domain_string(acme_config_string)?;
                                let config = serde_json::to_value(config)?;
                                Ok(config)
                            } else {
                                bail!("unable to load ACME domain config '{}'", config_key);
                            }
                        }
                    }
                },
                fake_url,
            ))
            .on_submit({
                let config_key = config_key.to_owned();
                move |form_ctx: FormContext| {
                    Self::update_acme_domain(form_ctx, Some(config_key.clone()))
                }
            })
            .into()
    }

    fn create_edit_acme_account_dialog(
        &self,
        ctx: &crate::LoadableComponentContext<Self>,
    ) -> Html {
        EditWindow::new(tr!("Edit account settings"))
            .on_done(ctx.link().change_view_callback(|_| None))
            .loader((
                |url: AttrValue| async move {
                    let data: Value = crate::http_get(&*url, None).await?;
                    if let Some(Value::String(acme_account)) = data.get("acme") {
                        let acme_account = parse_acme_config_string(acme_account)?;
                        let acme_account = serde_json::to_value(acme_account)?;
                        Ok(acme_account)
                    } else {
                        Ok(Value::Null)
                    }
                },
                "/nodes/localhost/config",
            ))
            .renderer(|_form_ctx: &FormContext| {
                let panel = InputPanel::new().class("pwt-flex-fit pwt-p-4").with_field(
                    tr!("Account Name"),
                    AcmeAccountSelector::new()
                        .name("account")
                        .placeholder("default")
                        .autofocus(true),
                );
                panel.into()
            })
            .on_submit(|form_ctx: FormContext| async move {
                let account = form_ctx.read().get_field_text("account");
                let data = if account.is_empty() {
                    json!({ "delete": ["acme"] })
                } else {
                    let acme = form_ctx.get_submit_data();
                    let acme: AcmeConfig = serde_json::from_value(acme)?;
                    let acme = create_acme_config_string(&acme);
                    json!({ "acme": acme })
                };
                crate::http_put("/nodes/localhost/config", Some(data)).await?;
                Ok(())
            })
            .into()
    }
}
