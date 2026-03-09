use std::rc::Rc;

use anyhow::Error;

use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::{Column, Container, Fa, Row};

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;

use crate::percent_encoding::percent_encode_component;
use crate::{
    LoadableComponent, LoadableComponentContext, LoadableComponentMaster,
    LoadableComponentScopeExt, LoadableComponentState,
};

use pve_api_types::ListFirewallRules;

use pwt_macros::builder;

use crate::form::pve::PveGuestType;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct FirewallRulesPanel {
    context: FirewallContext,

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

impl FirewallRulesPanel {
    pub fn cluster_firewall() -> Self {
        let context = FirewallContext::Cluster;
        yew::props!(Self { context })
    }
    pub fn node_firewall(node: impl Into<AttrValue>) -> Self {
        let context = FirewallContext::Node { node: node.into() };
        yew::props!(Self { context })
    }

    pub fn guest_firewall(guest_type: PveGuestType, node: impl Into<AttrValue>, vmid: u32) -> Self {
        let context = FirewallContext::Guest {
            node: node.into(),
            vmid,
            guest_type,
        };
        yew::props!(Self { context })
    }

    fn url(&self) -> String {
        let remote = self.remote.as_ref();
        match &self.context {
            FirewallContext::Cluster => {
                if let Some(remote) = remote {
                    format!(
                        "/pve/remotes/{}/firewall/rules",
                        percent_encode_component(remote)
                    )
                } else {
                    "/pve/firewall/rules".into()
                }
            }
            FirewallContext::Node { node } => {
                if let Some(remote) = remote {
                    format!(
                        "/pve/remotes/{}/nodes/{}/firewall/rules",
                        percent_encode_component(remote),
                        percent_encode_component(node)
                    )
                } else {
                    format!("/nodes/{}/firewall/rules", percent_encode_component(node))
                }
            }
            FirewallContext::Guest {
                node,
                vmid,
                guest_type,
            } => {
                let vmtype = match guest_type {
                    PveGuestType::Lxc => "lxc",
                    PveGuestType::Qemu => "qemu",
                };
                if let Some(remote) = remote {
                    format!(
                        "/pve/remotes/{}/{}/{}/firewall/rules?node={}",
                        percent_encode_component(remote),
                        vmtype,
                        vmid,
                        percent_encode_component(node)
                    )
                } else {
                    format!(
                        "/nodes/{}/{}/{}/firewall/rules",
                        percent_encode_component(node),
                        vmtype,
                        vmid
                    )
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
enum FirewallContext {
    Cluster,
    Node {
        node: AttrValue,
    },
    Guest {
        node: AttrValue,
        vmid: u32,
        guest_type: PveGuestType,
    },
}

#[derive(Copy, Clone, PartialEq)]
pub enum ViewState {}

pub struct FirewallRulesGuestComp {
    state: LoadableComponentState<ViewState>,
    columns: Rc<Vec<DataTableHeader<ListFirewallRules>>>,
    store: Store<ListFirewallRules>,
}

pwt::impl_deref_mut_property!(
    FirewallRulesGuestComp,
    state,
    LoadableComponentState<ViewState>
);

impl LoadableComponent for FirewallRulesGuestComp {
    type Properties = FirewallRulesPanel;
    type Message = ();
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let props = ctx.props();
        let state = LoadableComponentState::new();
        let store = Store::with_extract_key(|item: &ListFirewallRules| Key::from(item.pos));

        let columns = if props.mobile {
            columns_mobile()
        } else {
            columns()
        };

        Self {
            state,
            store,
            columns,
        }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> std::pin::Pin<Box<dyn std::prelude::rust_2024::Future<Output = Result<(), Error>>>> {
        let props = ctx.props();
        let url = props.url();
        let store = self.store.clone();
        Box::pin(async move {
            let data: Vec<ListFirewallRules> = crate::http_get(url, None).await?;
            store.set_data(data);
            Ok(())
        })
    }

    fn changed(
        &mut self,
        ctx: &LoadableComponentContext<Self>,
        old_props: &Self::Properties,
    ) -> bool {
        let props = ctx.props();
        if props.url() != old_props.url() {
            ctx.link().send_reload();
        }
        true
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let props = ctx.props();
        DataTable::new(self.columns.clone(), self.store.clone())
            .class(pwt::css::FlexFit)
            .show_header(!props.mobile)
            .into()
    }
}

fn pill(text: impl Into<AttrValue>) -> Container {
    Container::from_tag("div")
        .style("background-color", "var(--pwt-color-neutral-container)")
        .style("color", "var(--pwt-color-on-neutral-container)")
        .style("border-radius", "var(--pwt-button-corner-shape)")
        .style("padding-inline", "var(--pwt-spacer-1)")
        .with_child(text.into())
}

fn render_firewall_rule(rule: &ListFirewallRules) -> Html {
    let mut parts = Vec::new();

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

    Row::new()
        .gap(1)
        .style("flex-wrap", "wrap")
        .children(parts)
        .into()
}

fn render_firewall_rule_mobile(rule: &ListFirewallRules) -> Html {
    let mut tile = Row::new().gap(2);

    let prefix = Column::new()
        .gap(1)
        .with_child(Container::new().with_child(&rule.pos))
        .with_child(Container::new().with_child(&rule.ty));

    tile.add_child(prefix);

    let rule_view = render_firewall_rule(rule);

    tile.add_child(
        Container::new()
            .class(pwt::css::Flex::Fill)
            .with_child(rule_view),
    );

    let suffix = Column::new()
        .gap(1)
        .class(pwt::css::AlignItems::Center)
        .with_child(
            Container::new().with_child(
                Container::new()
                    .style("opacity", (rule.enable != Some(1)).then_some("0.5"))
                    .with_child(&rule.action),
            ),
        )
        .with_child(match rule.enable {
            Some(1) => Fa::new("check").to_html(),
            Some(0) | None => Fa::new("minus").into(),
            _ => "-".into(),
        });
    tile.add_child(suffix);

    tile.into()
}

fn columns_mobile() -> Rc<Vec<DataTableHeader<ListFirewallRules>>> {
    Rc::new(vec![DataTableColumn::new("")
        .flex(1)
        .render(|rule: &ListFirewallRules| render_firewall_rule_mobile(rule))
        .into()])
}

fn columns() -> Rc<Vec<DataTableHeader<ListFirewallRules>>> {
    Rc::new(vec![
        DataTableColumn::new("")
            .width("60px")
            .justify("right")
            .show_menu(false)
            //.resizable(false)
            .render(|rule: &ListFirewallRules| html! {&rule.pos})
            .into(),
        DataTableColumn::new(tr!("Active"))
            //.width("60px")
            .justify("center")
            //.resizable(false)
            .render(|rule: &ListFirewallRules| match rule.enable {
                Some(1) => Fa::new("check").into(),
                Some(0) | None => Fa::new("minus").into(),
                _ => "-".into(),
            })
            .into(),
        DataTableColumn::new(tr!("Type"))
            .width("80px")
            .render(|rule: &ListFirewallRules| html! {&rule.ty})
            .into(),
        DataTableColumn::new(tr!("Action"))
            .width("100px")
            .render(|rule: &ListFirewallRules| html! {&rule.action})
            .into(),
        DataTableColumn::new(tr!("Rule"))
            .flex(1)
            .render(|rule: &ListFirewallRules| render_firewall_rule(rule))
            .into(),
        DataTableColumn::new(tr!("Comment"))
            .width("150px")
            .render(|rule: &ListFirewallRules| rule.comment.as_deref().unwrap_or("-").into())
            .into(),
    ])
}

impl From<FirewallRulesPanel> for VNode {
    fn from(prop: FirewallRulesPanel) -> VNode {
        let comp =
            VComp::new::<LoadableComponentMaster<FirewallRulesGuestComp>>(Rc::new(prop), None);
        VNode::from(comp)
    }
}
