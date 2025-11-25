use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use proxmox_schema::property_string::PropertyString;
use pve_api_types::{LxcConfig, LxcConfigNet};

use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use serde_json::Value;
use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::ExtractPrimaryKey;
use pwt::state::{Selection, Store};

use pwt_macros::builder;

use crate::LoadableComponentMaster;
use crate::{
    configuration::guest_config_url, form::pve::PveGuestType, LoadableComponent,
    LoadableComponentContext,
};

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct LxcNetworkPanel {
    vmid: u32,
    node: AttrValue,

    /// Use Proxmox Datacenter Manager API endpoints
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub remote: Option<AttrValue>,

    /// Layout for mobile devices.
    #[prop_or_default]
    #[builder]
    pub mobile: bool,

    /// Read-only view - hide toolbar and all buttons/menus to edit content.
    #[prop_or_default]
    #[builder]
    pub readonly: bool,
}

impl LxcNetworkPanel {
    pub fn new(node: impl Into<AttrValue>, vmid: u32) -> Self {
        yew::props!(Self {
            node: node.into(),
            vmid,
        })
    }
}

#[derive(Clone, PartialEq)]
struct NetworkEntry {
    key: Key,
    config: LxcConfigNet,
}

impl ExtractPrimaryKey for NetworkEntry {
    fn extract_key(&self) -> Key {
        Key::from(self.key.clone())
    }
}

#[derive(PartialEq)]
pub enum ViewState {
    // Add,
    Edit,
}

pub enum Msg {
    Redraw,
    SelectionChange,
}
pub struct LxcNetworkComp {
    columns: Rc<Vec<DataTableHeader<NetworkEntry>>>,
    store: Store<NetworkEntry>,
    selection: Selection,
}

impl LxcNetworkComp {
    fn get_selected_record(&self) -> Option<NetworkEntry> {
        let selected_key = self.selection.selected_key();
        let mut selected_record = None;
        if let Some(key) = &selected_key {
            selected_record = self.store.read().lookup_record(key).cloned();
        }
        selected_record
    }
}

impl LoadableComponent for LxcNetworkComp {
    type Message = Msg;
    type Properties = LxcNetworkPanel;
    type ViewState = ViewState;

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let props = ctx.props();
        let store = self.store.clone();
        let url = guest_config_url(props.vmid, &props.node, &props.remote, PveGuestType::Lxc);
        Box::pin(async move {
            let data: LxcConfig = crate::http_get(url, None).await?;

            let mut list = Vec::new();
            for (index, net) in data.net.iter() {
                let net: PropertyString<LxcConfigNet> =
                    serde_json::from_value(Value::String(net.clone()))?;

                list.push(NetworkEntry {
                    key: Key::from(format!("net{}", index)),
                    config: net.into_inner(),
                });
            }
            store.write().set_data(list);
            Ok(())
        })
    }

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::SelectionChange));
        let store = Store::new().on_change(ctx.link().callback(|_| Msg::Redraw));

        Self {
            store,
            selection,
            columns: columns(),
        }
    }

    fn update(&mut self, _ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SelectionChange => true,
            Msg::Redraw => true,
        }
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let props = ctx.props();
        let readonly = props.readonly;
        let link = ctx.link();
        DataTable::new(Rc::clone(&self.columns), self.store.clone())
            .class(pwt::css::FlexFit)
            .selection(self.selection.clone())
            .striped(true)
            .virtual_scroll(false)
            .on_row_dblclick(move |_: &mut _| {
                if !readonly {
                    link.change_view(Some(ViewState::Edit));
                }
            })
            .into()
    }

    fn dialog_view(
        &self,
        _ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        match view_state {
            //ViewState::Add => None,
            ViewState::Edit => match self.get_selected_record() {
                None => None,
                Some(_record) => None,
            },
        }
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

fn columns() -> Rc<Vec<DataTableHeader<NetworkEntry>>> {
    Rc::new(vec![
        DataTableColumn::new(tr!("ID"))
            .width("minmax(80px, auto)")
            .get_property(|item: &NetworkEntry| &item.key)
            .into(),
        DataTableColumn::new(tr!("Name"))
            .width("minmax(80px, auto)")
            .get_property(|item: &NetworkEntry| &item.config.name)
            .into(),
        DataTableColumn::new(tr!("Bridge"))
            .width("minmax(80px, auto)")
            .get_property_owned(|item: &NetworkEntry| {
                item.config.bridge.clone().unwrap_or(String::new())
            })
            .into(),
        DataTableColumn::new(tr!("Firewall"))
            .width("minmax(80px, auto)")
            .render({
                let yes_text = tr!("Yes");
                let no_text = tr!("No");

                move |item: &NetworkEntry| {
                    html! {{
                        match item.config.firewall.unwrap_or(false) {
                            true => &yes_text,
                            false => &no_text,
                        }
                    }}
                }
            })
            .into(),
        DataTableColumn::new(tr!("VLAN Tag"))
            .width("minmax(80px, auto)")
            .render(|item: &NetworkEntry| {
                html! { item.config.tag.map(|tag| tag.to_string()).unwrap_or(String::new())}
            })
            .into(),
        DataTableColumn::new(tr!("MAC address"))
            .width("minmax(160px,1fr)")
            .render(|item: &NetworkEntry| {
                html! { item.config.hwaddr.as_deref().unwrap_or("")}
            })
            .into(),
        DataTableColumn::new(tr!("CIDR"))
            .width("minmax(150px,1fr)")
            .render(move |item: &NetworkEntry| {
                render_two_lines(item.config.ip.as_ref(), item.config.ip6.as_ref())
            })
            .into(),
        DataTableColumn::new(tr!("Gateway"))
            .width("minmax(150px,1fr)")
            .render(move |item: &NetworkEntry| {
                render_two_lines(item.config.gw.as_ref(), item.config.gw6.as_ref())
            })
            .into(),
        DataTableColumn::new(tr!("MTU"))
            .width("minmax(80px, auto)")
            .render(|item: &NetworkEntry| {
                html! { item.config.mtu.map(|mtu| mtu.to_string()).unwrap_or(String::new())}
            })
            .into(),
        DataTableColumn::new(tr!("Disconnected"))
            .width("minmax(80px, auto)")
            .render({
                let yes_text = tr!("Yes");
                let no_text = tr!("No");

                move |item: &NetworkEntry| {
                    html! {{
                        match item.config.link_down.unwrap_or(false) {
                            true => &yes_text,
                            false => &no_text,
                        }
                    }}
                }
            })
            .into(),
    ])
}

impl From<LxcNetworkPanel> for VNode {
    fn from(props: LxcNetworkPanel) -> Self {
        let comp = VComp::new::<LoadableComponentMaster<LxcNetworkComp>>(Rc::new(props), None);
        VNode::from(comp)
    }
}
