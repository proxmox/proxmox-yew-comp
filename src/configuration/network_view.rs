use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;

use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::menu::{Menu, MenuButton, MenuItem};
use pwt::widget::{Button, Column, SplitPane, Toolbar};

use proxmox_client::ApiResponseData;
use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster, TaskProgress};

use crate::percent_encoding::percent_encode_component;
use proxmox_system_config_api::network::{BondXmitHashPolicy, Interface, LinuxBondMode, NetworkInterfaceType};

use super::NetworkEdit;

async fn load_interfaces() -> Result<(Vec<Interface>, String), Error> {
    let resp: ApiResponseData<Vec<Interface>> =
        crate::http_get_full("/nodes/localhost/network", None).await?;
    let data = resp.data;
    let changes = resp
        .attribs
        .get("changes")
        .map(|c| c.as_str())
        .flatten()
        .unwrap_or("");
    Ok((data, changes.to_string()))
}

async fn delete_interface(key: Key) -> Result<(), Error> {
    let url = format!(
        "/nodes/localhost/network/{}",
        percent_encode_component(&*key)
    );
    crate::http_delete(&url, None).await?;
    Ok(())
}

async fn revert_changes() -> Result<(), Error> {
    crate::http_delete("/nodes/localhost/network", None).await
}

async fn apply_changes() -> Result<String, Error> {
    crate::http_put("/nodes/localhost/network", None).await
}

#[derive(PartialEq, Properties)]
pub struct NetworkView {}

impl NetworkView {
    pub fn new() -> Self {
        Self {}
    }
}

#[doc(hidden)]
pub struct ProxmoxNetworkView {
    store: Store<Interface>,
    changes: String,
    selection: Selection,
}

#[derive(PartialEq)]
pub enum ViewState {
    AddBridge,
    AddBond,
    Edit,
    ApplyChanges(String),
}

pub enum Msg {
    SelectionChange,
    RemoveItem,
    Changes(String),
    RevertChanges,
    ApplyChanges,
}

impl ProxmoxNetworkView {
    fn get_selected_record(&self) -> Option<Interface> {
        let selected_key = self.selection.selected_key();
        let mut selected_record = None;
        if let Some(key) = &selected_key {
            selected_record = self.store.read().lookup_record(key).map(|r| r.clone());
        }
        selected_record
    }
}

fn find_next_free_interface_id(prefix: &str, list: &[Interface]) -> Option<String> {
    for next in 0..9999 {
        let id = format!("{prefix}{next}");
        if list.iter().find(|item| item.name == id).is_none() {
            return Some(id);
        }
    }
    None
}

impl LoadableComponent for ProxmoxNetworkView {
    type Message = Msg;
    type Properties = NetworkView;
    type ViewState = ViewState;

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let store = self.store.clone();
        let link = ctx.link();
        Box::pin(async move {
            let (data, changes) = load_interfaces().await?;
            store.write().set_data(data);
            link.send_message(Msg::Changes(changes));
            Ok(())
        })
    }

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let store = Store::with_extract_key(|record: &Interface| Key::from(record.name.as_str()));
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::SelectionChange));
        Self {
            store,
            selection,
            changes: String::new(),
        }
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SelectionChange => true,
            Msg::RemoveItem => {
                if let Some(key) = self.selection.selected_key() {
                    let link = ctx.link().clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Err(err) = delete_interface(key).await {
                            link.show_error(tr!("Unable to delete item"), err, true);
                        }
                        link.send_reload();
                    })
                }
                false
            }
            Msg::Changes(changes) => {
                self.changes = changes;
                true
            }
            Msg::RevertChanges => {
                let link = ctx.link().clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(err) = revert_changes().await {
                        link.show_error(tr!("Unable to revert changes"), err, true);
                    }
                    link.send_reload();
                });
                false
            }
            Msg::ApplyChanges => {
                let link = ctx.link().clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match apply_changes().await  {
                        Err(err) => {
                            link.show_error(tr!("Unable to apply changes"), err, true);
                            link.send_reload();

                        }
                        Ok(upid) => {
                            link.change_view(Some(ViewState::ApplyChanges(upid)));
                        }
                    }
                });
                false
            }
        }
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let link = ctx.link();

        let disabled = self.selection.is_empty();

        let no_changes = self.changes.is_empty();


        let add_menu = Menu::new()
            .with_item(
                MenuItem::new(tr!("Linux Bridge")).on_select(
                    ctx.link()
                        .change_view_callback(|_| Some(ViewState::AddBridge)),
                ),
            )
            .with_item(
                MenuItem::new(tr!("Linux Bond")).on_select(
                    ctx.link()
                        .change_view_callback(|_| Some(ViewState::AddBond)),
                ),
            );

        let toolbar = Toolbar::new()
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                MenuButton::new(tr!("Create"))
                    .show_arrow(true)
                    .menu(add_menu),
            )
            .with_spacer()
            .with_child(
                Button::new(tr!("Revert"))
                    .disabled(no_changes)
                    .onclick(link.callback(|_| Msg::RevertChanges)),
            )
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(disabled)
                    .onclick(link.change_view_callback(|_| Some(ViewState::Edit))),
            )
            .with_child(
                Button::new(tr!("Remove"))
                    .disabled(disabled)
                    .onclick(link.callback(|_| Msg::RemoveItem)),
            )
            .with_spacer()
            .with_child(
                Button::new(tr!("Apply Configuration"))
                    .disabled(no_changes)
                    .onclick(link.callback(|_| Msg::ApplyChanges)),
            )
            .with_flex_spacer()
            .with_child({
                let loading = ctx.loading();
                let link = ctx.link();
                Button::refresh(loading).onclick(move |_| link.send_reload())
            });

        Some(toolbar.into())
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let link = ctx.link();

        let table = DataTable::new(columns(), self.store.clone())
            .class("pwt-flex-fit")
            .selection(self.selection.clone())
            .striped(true)
            .on_row_dblclick(move |_: &mut _| {
                link.change_view(Some(ViewState::Edit));
            });

        let changes = (!self.changes.is_empty()).then(|| {
            Column::new()
                .class("pwt-flex-fit")
                .with_child(html!{
                    <div class="pwt-p-2 pwt-border-bottom pwt-font-size-body-medium">{
                    tr!("Pending changes (Either reboot or use 'Apply Configuration' (needs ifupdown2) to activate)")
                    }</div>
                })
                .with_child(html!{
                    <pre class="pwt-flex-fit pwt-p-2 pwt-font-monospace pwt-font-size-body-medium pwt-line-height-body-medium">{&self.changes}</pre>
                })
        });

        let mut split = SplitPane::new()
            .class("pwt-flex-fit")
            .vertical(true)
            .with_child(table);

        if let Some(changes) = changes {
            split.add_child(changes);
        }

        split.into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        let guard = self.store.read();
        let list = guard.data();
        match view_state {
            ViewState::AddBridge => Some(
                NetworkEdit::new(NetworkInterfaceType::Bridge)
                    .default_name(find_next_free_interface_id("vmbr", list))
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::AddBond => Some(
                NetworkEdit::new(NetworkInterfaceType::Bond)
                    .default_name(find_next_free_interface_id("bond", list))
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::Edit => match self.get_selected_record() {
                None => None,
                Some(record) => Some(
                    NetworkEdit::new(record.interface_type)
                        .name(AttrValue::from(record.name.clone()))
                        .on_close(ctx.link().change_view_callback(|_| None))
                        .into(),
                ),
            },
            ViewState::ApplyChanges(task_id) => Some(
                TaskProgress::new(task_id)
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into()
            )
        }
    }
}

impl Into<VNode> for NetworkView {
    fn into(self) -> VNode {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxNetworkView>>(Rc::new(self), None);
        VNode::from(comp)
    }
}

fn format_ports_slaves(interface: &Interface) -> String {
    match interface.interface_type {
        NetworkInterfaceType::Bridge => interface
            .bridge_ports
            .as_ref()
            .map(|ports| ports.join(" "))
            .unwrap_or(String::new()),
        NetworkInterfaceType::Bond => interface
            .slaves
            .as_ref()
            .map(|ports| ports.join(" "))
            .unwrap_or(String::new()),
        NetworkInterfaceType::Alias
        | NetworkInterfaceType::Vlan
        | NetworkInterfaceType::Eth
        | NetworkInterfaceType::Loopback
        | NetworkInterfaceType::Unknown => String::new(),
    }
}

fn format_bond_mode(mode: Option<LinuxBondMode>) -> String {
    let mode = match mode {
        Some(mode) => mode,
        None => return String::new(),
    };

    match mode {
        LinuxBondMode::ActiveBackup
        | LinuxBondMode::BalanceAlb
        | LinuxBondMode::BalanceRr
        | LinuxBondMode::BalanceTlb
        | LinuxBondMode::BalanceXor
        | LinuxBondMode::Broadcast => serde_plain::to_string(&mode).unwrap(),
        LinuxBondMode::Ieee802_3ad => "LACP (802.3ad)".into(),
    }
}

fn format_bond_xmit_hash_policy(policy: Option<BondXmitHashPolicy>) -> String {
    match policy {
        Some(policy) => serde_plain::to_string(&policy).unwrap(),
        None => String::new(),
    }
}

fn render_two_lines(
    line1: Option<impl std::fmt::Display>,
    line2: Option<impl std::fmt::Display>,
) -> Html {
    let line1 = line1.map(|l| l.to_string()).filter(|l| !l.is_empty());
    let line2 = line2.map(|l| l.to_string()).filter(|l| !l.is_empty());

    match (line1, line2) {
        (Some(line1), Some(line2)) => html! {<><div>{line1}</div><div>{line2}</div></>},
        (Some(line1), None) => html! {line1},
        (None, Some(line2)) => html! {line2},
        _ => html! {},
    }
}

fn columns() -> Rc<Vec<DataTableHeader<Interface>>>
{
    Rc::new(vec![
        DataTableColumn::new(tr!("Name"))
            .width("120px")
            .render(|item: &Interface| html!{
                item.name.clone()
            })
            .sorter(|a: &Interface, b: &Interface| {
                a.name.cmp(&b.name)
            })
            .into(),
        DataTableColumn::new(tr!("Type"))
            .width("120px")
            .render(|item: &Interface| html!{
                crate::utils::format_network_interface_type(item.interface_type)
            })
            .sorter(|a: &Interface, b: &Interface| {
                let a =  crate::utils::format_network_interface_type(a.interface_type);
                let b =  crate::utils::format_network_interface_type(b.interface_type);
                a.cmp(&b)
            })
            .into(),
        DataTableColumn::new(tr!("Active"))
            .width("100px")
            .render({
                let yes_text = tr!("Yes");
                let no_text = tr!("No");

                move |item: &Interface| html!{{
                    match item.active {
                        true => &yes_text,
                        false => &no_text,
                    }
                }}
            })
            .sorter(|a: &Interface, b: &Interface| {
                a.active.cmp(&b.active)
            })
            .into(),
        DataTableColumn::new(tr!("Autostart"))
            .width("100px")
            .render({
                let yes_text = tr!("Yes");
                let no_text = tr!("No");

                move |item: &Interface| html!{{
                    match item.autostart {
                        true => &yes_text,
                        false => &no_text,
                    }
                }}
            })
            .sorter(|a: &Interface, b: &Interface| {
                a.autostart.cmp(&b.autostart)
            })
            .into(),
        DataTableColumn::new(tr!("VLAN aware"))
            .width("100px")
            .hidden(true)
            .render({
                let yes_text = tr!("Yes");
                let no_text = tr!("No");

                move |item: &Interface| html!{{
                    match item.bridge_vlan_aware {
                        Some(true) => &yes_text,
                        _ => &no_text,
                    }
                }}
            })
            .sorter(|a: &Interface, b: &Interface| {
                a.bridge_vlan_aware.cmp(&b.bridge_vlan_aware)
            })
            .into(),

        DataTableColumn::new(tr!("Ports/Slaves"))
            .width("120px")
            .render(move |item: &Interface| html!{format_ports_slaves(item)})
            .into(),
        DataTableColumn::new(tr!("Bond Mode"))
            .width("120px")
            .render(move |item: &Interface| html!{format_bond_mode(item.bond_mode)})
            .into(),
        DataTableColumn::new(tr!("Hash policy"))
            .width("120px")
            .hidden(true)
            .render(move |item: &Interface| html!{format_bond_xmit_hash_policy(item.bond_xmit_hash_policy)})
            .into(),
        DataTableColumn::new(tr!("CIDR"))
            .width("150px")
            .render(move |item: &Interface| {
                render_two_lines(item.cidr.as_ref(), item.cidr6.as_ref())
            })
            .into(),
        DataTableColumn::new(tr!("Gateway"))
            .width("150px")
            .render(move |item: &Interface| {
                render_two_lines(item.gateway.as_ref(), item.gateway6.as_ref())
            })
            .into(),
        DataTableColumn::new(tr!("MTU"))
            .width("100px")
            .hidden(true)
            .render(move |item: &Interface| {
                let text = match item.mtu {
                    Some(mtu) => mtu.to_string(),
                    None => String::new(),
                };
                html!{text}
            })
            .into(),
        DataTableColumn::new("Comment")
            .flex(1)
            .render(|item: &Interface| html!{
                item.comments.clone().unwrap_or(String::new())
            })
            .into()

    ])
}
