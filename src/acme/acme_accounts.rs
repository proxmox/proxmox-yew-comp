use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::{Button, Container, Toolbar};

use crate::common_api_types::AcmeAccountInfo;
use crate::percent_encoding::percent_encode_component;
use crate::utils::render_url;
use crate::{
    ConfirmButton, DataViewWindow, LoadableComponent, LoadableComponentContext,
    LoadableComponentMaster,
};

use super::AcmeRegisterAccount;

// An ACME Account entry.
#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub struct AcmeAccountEntry {
    pub name: String,
}

#[derive(PartialEq, Properties)]
pub struct AcmeAccountsPanel {}

impl Default for AcmeAccountsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl AcmeAccountsPanel {
    pub fn new() -> Self {
        Self {}
    }
}

#[doc(hidden)]
pub struct ProxmoxAcmeAccountsPanel {
    selection: Selection,
    store: Store<AcmeAccountEntry>,
    columns: Rc<Vec<DataTableHeader<AcmeAccountEntry>>>,
}

pub enum Msg {
    Redraw,
}

#[derive(PartialEq)]
pub enum ViewState {
    Add,
    View(Key),
}

impl LoadableComponent for ProxmoxAcmeAccountsPanel {
    type Properties = AcmeAccountsPanel;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::Redraw));
        let store =
            Store::with_extract_key(|record: &AcmeAccountEntry| Key::from(record.name.clone()));

        let columns = Rc::new(vec![DataTableColumn::new(tr!("Name"))
            .flex(1)
            .render(|record: &AcmeAccountEntry| html! { &record.name })
            .sorter(|a: &AcmeAccountEntry, b: &AcmeAccountEntry| a.name.cmp(&b.name))
            .sort_order(true)
            .into()]);

        Self {
            selection,
            store,
            columns,
        }
    }

    fn load(
        &self,
        _ctx: &crate::LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let store = self.store.clone();
        Box::pin(async move {
            let data = crate::http_get("/config/acme/account", None).await?;
            store.write().set_data(data);
            Ok(())
        })
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
                Button::new(tr!("View"))
                    .disabled(selected_key.is_none())
                    .onclick({
                        let link = ctx.link();
                        let selected_key = selected_key.clone();
                        move |_| {
                            if let Some(selected_key) = &selected_key {
                                link.change_view(Some(ViewState::View(selected_key.clone())));
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
                                let command_path = format!(
                                    "/config/acme/account/{}",
                                    percent_encode_component(selected_key)
                                );
                                let command_future =
                                    crate::http_delete_get::<String>(command_path, None);
                                link.clone().spawn(async move {
                                    match command_future.await {
                                        Ok(task_id) => {
                                            link.show_task_progres(task_id);
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
                let selection = self.selection.clone();
                let link = ctx.link();
                move |_: &mut _| {
                    if let Some(selected_key) = selection.selected_key() {
                        link.change_view(Some(ViewState::View(selected_key.clone())));
                    }
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
            ViewState::Add => Some(
                AcmeRegisterAccount::new()
                    .on_done(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::View(account_name) => {
                Some(self.create_account_view_dialog(ctx, account_name))
            }
        }
    }
}

impl From<AcmeAccountsPanel> for VNode {
    fn from(val: AcmeAccountsPanel) -> Self {
        let comp =
            VComp::new::<LoadableComponentMaster<ProxmoxAcmeAccountsPanel>>(Rc::new(val), None);
        VNode::from(comp)
    }
}

impl ProxmoxAcmeAccountsPanel {
    fn create_account_view_dialog(
        &self,
        ctx: &crate::LoadableComponentContext<Self>,
        account_name: &str,
    ) -> Html {
        let url = format!(
            "/config/acme/account/{}",
            percent_encode_component(account_name)
        );
        DataViewWindow::<AcmeAccountInfo>::new(tr!("Account") + ": " + account_name)
            .loader(url)
            .on_done(ctx.link().change_view_callback(|_| None))
            .renderer(|data: &AcmeAccountInfo| {
                let mut grid = Container::new()
                    .padding(4)
                    .class("pwt-flex-fit pwt-d-grid pwt-gap-4")
                    .style("grid-template-columns", "auto 500px");

                for contact in &data.account.contact {
                    grid.add_child(html! {<span>{tr!("Contact")}</span>});
                    grid.add_child(html! {<span>{contact}</span>});
                }

                if let Some(created_at) = &data.account.created_at {
                    grid.add_child(html! {<span>{tr!("Created")}</span>});
                    grid.add_child(html! {<span>{created_at}</span>});
                }

                grid.add_child(html! {<span>{tr!("Status")}</span>});
                grid.add_child(html! {<span>{&data.account.status}</span>});

                grid.add_child(html! {<span>{tr!("Directory")}</span>});
                grid.add_child(render_url(&data.directory));

                if let Some(tos) = &data.tos {
                    grid.add_child(html! {<span>{tr!("Terms of Services")}</span>});
                    grid.add_child(render_url(tos));
                }

                grid.into()
            })
            .into()
    }
}
