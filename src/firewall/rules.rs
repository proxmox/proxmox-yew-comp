use std::rc::Rc;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Loader, LoaderState, SharedStateObserver, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::{Container, Fa};
use pwt_macros::builder;

use super::context::FirewallContext;

/// Properties for displaying firewall rules in a read-only table.
///
/// Displays the list of firewall rules for a given context (cluster, node, or guest).
/// This is read-only currently, so it doesn't include any buttons for editing or adding rules.
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct FirewallRules {
    /// The firewall context specifying which level to display rules for (cluster, node, or guest).
    #[builder(IntoPropValue, into_prop_value)]
    pub context: FirewallContext,

    /// Callback invoked when the component is closed.
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    /// Token to trigger a reload of the data.
    #[prop_or_default]
    pub reload_token: usize,
}

impl FirewallRules {
    /// Creates a new `FirewallRules` for displaying cluster-level firewall rules.
    ///
    /// # Arguments
    ///
    /// * `remote` - The remote identifier for the PVE cluster.
    pub fn cluster(remote: impl Into<AttrValue>) -> Self {
        yew::props!(Self {
            context: FirewallContext::cluster(remote),
        })
    }

    /// Creates a new `FirewallRules` for displaying node-level firewall rules.
    ///
    /// # Arguments
    ///
    /// * `remote` - The remote identifier for the PVE cluster.
    /// * `node` - The node identifier.
    pub fn node(remote: impl Into<AttrValue>, node: impl Into<AttrValue>) -> Self {
        yew::props!(Self {
            context: FirewallContext::node(remote, node),
        })
    }

    /// Creates a new `FirewallRules` for displaying guest-level firewall rules.
    ///
    /// # Arguments
    ///
    /// * `remote` - The remote identifier for the PVE cluster.
    /// * `node` - The node identifier where the guest is located.
    /// * `vmid` - The virtual machine ID.
    /// * `vmtype` - The type of guest ("lxc" or "qemu").
    pub fn guest(
        remote: impl Into<AttrValue>,
        node: impl Into<AttrValue>,
        vmid: u64,
        vmtype: impl Into<AttrValue>,
    ) -> Self {
        yew::props!(Self {
            context: FirewallContext::guest(remote, node, vmid, vmtype),
        })
    }
}

pub enum FirewallMsg {
    DataChange,
}

#[doc(hidden)]
pub struct ProxmoxFirewallRules {
    store: Store<pve_api_types::ListFirewallRules>,
    loader: Loader<Vec<pve_api_types::ListFirewallRules>>,
    _listener: SharedStateObserver<LoaderState<Vec<pve_api_types::ListFirewallRules>>>,
    columns: Rc<Vec<DataTableHeader<pve_api_types::ListFirewallRules>>>,
}

fn pill(text: impl Into<AttrValue>) -> Container {
    Container::from_tag("span")
        .style("display", "inline-block")
        .style("margin", "0 1px")
        .style("background-color", "var(--pwt-color-neutral-container)")
        .style("color", "var(--pwt-color-on-neutral-container)")
        .style("border-radius", "var(--pwt-button-corner-shape)")
        .style("padding-inline", "var(--pwt-spacer-2)")
        .with_child(text.into())
}

fn format_firewall_rule(rule: &pve_api_types::ListFirewallRules) -> Html {
    let mut parts: Vec<VNode> = Vec::new();

    if let Some(iface) = &rule.iface {
        parts.push(pill(format!("iface: {iface}")).into());
    }

    if let Some(macro_name) = &rule.r#macro {
        parts.push(pill(format!("macro: {macro_name}")).into());
    }

    if let Some(proto) = &rule.proto {
        let mut proto_str = proto.to_uppercase();
        if matches!(proto.as_str(), "icmp" | "icmpv6" | "ipv6-icmp") {
            if let Some(icmp_type) = &rule.icmp_type {
                proto_str = format!("{proto_str}, {icmp_type}");
            }
        }
        parts.push(pill(format!("proto: {proto_str}")).into());
    }

    let mut push_host_port =
        |host: &Option<String>, port: &Option<String>, label: &str| match (host, port) {
            (Some(h), Some(p)) => parts.push(pill(format!("{label}: {h}, port: {p}")).into()),
            (Some(h), None) => parts.push(pill(format!("{label}: {h}")).into()),
            (None, Some(p)) => parts.push(pill(format!("{label}: any, port: {p}")).into()),
            _ => {}
        };

    push_host_port(&rule.source, &rule.sport, "src");
    push_host_port(&rule.dest, &rule.dport, "dest");

    if parts.is_empty() {
        return "-".into();
    }

    parts
        .into_iter()
        .enumerate()
        .flat_map(|(i, part)| {
            if i == 0 {
                vec![part]
            } else {
                vec![" ".into(), part]
            }
        })
        .collect::<Html>()
}

impl ProxmoxFirewallRules {
    fn update_data(&mut self) {
        if let Some(Ok(data)) = &self.loader.read().data {
            self.store.set_data((**data).clone());
        }
    }

    fn build_columns() -> Rc<Vec<DataTableHeader<pve_api_types::ListFirewallRules>>> {
        Rc::new(vec![
            DataTableColumn::new("")
                .width("30px")
                .justify("right")
                .show_menu(false)
                .resizable(false)
                .render(|rule: &pve_api_types::ListFirewallRules| html! {&rule.pos})
                .into(),
            DataTableColumn::new(tr!("On"))
                .width("40px")
                .justify("center")
                .resizable(false)
                .render(
                    |rule: &pve_api_types::ListFirewallRules| match rule.enable {
                        Some(1) => Fa::new("check").into(),
                        Some(0) | None => Fa::new("minus").into(),
                        _ => "-".into(),
                    },
                )
                .into(),
            DataTableColumn::new(tr!("Type"))
                .width("80px")
                .render(|rule: &pve_api_types::ListFirewallRules| html! {&rule.ty})
                .into(),
            DataTableColumn::new(tr!("Action"))
                .width("100px")
                .render(|rule: &pve_api_types::ListFirewallRules| html! {&rule.action})
                .into(),
            DataTableColumn::new(tr!("Rule"))
                .flex(1)
                .render(|rule: &pve_api_types::ListFirewallRules| format_firewall_rule(rule))
                .into(),
            DataTableColumn::new(tr!("Comment"))
                .width("150px")
                .render(|rule: &pve_api_types::ListFirewallRules| {
                    rule.comment.as_deref().unwrap_or("-").into()
                })
                .into(),
        ])
    }
}

impl Component for ProxmoxFirewallRules {
    type Message = FirewallMsg;
    type Properties = FirewallRules;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let url: AttrValue = props.context.rules_url().into();

        let store = Store::with_extract_key(|item: &pve_api_types::ListFirewallRules| {
            Key::from(item.pos.to_string())
        });

        let loader = Loader::new().loader({
            let url = url.clone();
            move || {
                let url = url.clone();
                async move { crate::http_get(url.to_string(), None).await }
            }
        });

        let _listener = loader.add_listener(ctx.link().callback(|_| FirewallMsg::DataChange));

        loader.load();

        let mut me = Self {
            store,
            loader,
            _listener,
            columns: Self::build_columns(),
        };

        me.update_data();
        me
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().reload_token != old_props.reload_token {
            self.loader.load();
        }
        true
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            FirewallMsg::DataChange => {
                self.update_data();
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        self.loader.render(|_data| -> Html {
            if self.store.data_len() == 0 {
                Container::new()
                    .padding(2)
                    .with_child(tr!("No firewall rules configured"))
                    .into()
            } else {
                DataTable::new(self.columns.clone(), self.store.clone())
                    .show_header(true)
                    .striped(true)
                    .into()
            }
        })
    }
}

impl From<FirewallRules> for VNode {
    fn from(val: FirewallRules) -> Self {
        let comp = VComp::new::<ProxmoxFirewallRules>(Rc::new(val), None);
        VNode::from(comp)
    }
}
