use std::rc::Rc;

use pwt::props::SubmitCallback;
use serde_json::Value;

use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;

use crate::form::typed_load;
use crate::percent_encoding::percent_encode_component;
use crate::property_view::{PropertyGrid, PropertyList};
use crate::{ApiLoadCallback, EditableProperty};

use pve_api_types::NodeFirewallOptions;

use pwt_macros::builder;

use crate::form::pve::firewall_property;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct FirewallOptionsNodePanel {
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

impl FirewallOptionsNodePanel {
    pub fn new(node: AttrValue) -> Self {
        yew::props!(Self { node })
    }

    fn url(&self) -> String {
        if let Some(remote) = &self.remote {
            format!(
                "/pve/remotes/{}/nodes/{}/firewall/options",
                percent_encode_component(remote),
                percent_encode_component(&self.node)
            )
        } else {
            format!(
                "/nodes/{}/firewall/options",
                percent_encode_component(&self.node),
            )
        }
    }

    fn loader(&self) -> ApiLoadCallback<Value> {
        let url = self.url();
        typed_load::<NodeFirewallOptions>(&url)
    }

    fn on_submit(&self) -> Option<SubmitCallback<Value>> {
        (!self.readonly).then(|| {
            let url = self.url();
            SubmitCallback::new(move |value: Value| {
                let url = url.clone();
                async move { crate::http_put(url.clone(), Some(value.clone())).await }
            })
        })
    }
}

pub struct FirewallOptionsNodeComp {
    properties: Rc<Vec<EditableProperty>>,
    loader: ApiLoadCallback<Value>,
    on_submit: Option<SubmitCallback<Value>>,
}

fn properties(mobile: bool) -> Vec<EditableProperty> {
    vec![
        firewall_property::enable_node_property(mobile),
        firewall_property::nosmurfs_property(mobile),
        firewall_property::tcpflags_property(mobile),
        firewall_property::ndp_property(mobile),
        firewall_property::nf_conntrack_max_poperty(mobile),
        firewall_property::nf_timeout_established_poperty(mobile),
        firewall_property::log_level_in_property(mobile),
        firewall_property::log_level_out_property(mobile),
        firewall_property::log_level_forward_property(mobile),
        firewall_property::tcp_flags_log_level_property(mobile),
        firewall_property::smurf_log_level_property(mobile),
        firewall_property::input_policy_poperty(mobile),
        firewall_property::output_policy_poperty(mobile),
        firewall_property::nftables_property(mobile),
    ]
}

impl Component for FirewallOptionsNodeComp {
    type Message = ();
    type Properties = FirewallOptionsNodePanel;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        Self {
            properties: Rc::new(properties(props.mobile)),
            loader: props.loader(),
            on_submit: props.on_submit(),
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if props.url() != old_props.url() {
            self.loader = props.loader();
            self.on_submit = props.on_submit();
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        if props.mobile {
            PropertyList::new(Rc::clone(&self.properties))
                .key(props.url())
                .class(pwt::css::FlexFit)
                .loader(self.loader.clone())
                .on_submit(self.on_submit.clone())
                .into()
        } else {
            PropertyGrid::new(Rc::clone(&self.properties))
                .key(props.url())
                .class(pwt::css::FlexFit)
                .loader(self.loader.clone())
                .on_submit(self.on_submit.clone())
                .into()
        }
    }
}

impl From<FirewallOptionsNodePanel> for VNode {
    fn from(props: FirewallOptionsNodePanel) -> Self {
        let comp = VComp::new::<FirewallOptionsNodeComp>(Rc::new(props), None);
        VNode::from(comp)
    }
}
