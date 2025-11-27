use std::rc::Rc;

use anyhow::Error;
use proxmox_schema::ApiType;
use pve_api_types::{
    ClusterFirewallOptions, ClusterFirewallOptionsLogRatelimit, GuestFirewallOptions,
    NodeFirewallOptions,
};
use serde_json::Value;
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Checkbox, Combobox, FormContext, Number};
use pwt::widget::InputPanel;

use pwt_macros::builder;

use crate::{form::delete_empty_values, ApiLoadCallback, EditWindow};

use super::{context::FirewallContext, LogRatelimitField};

fn enum_items_from_schema<T: ApiType>(name: &str) -> Vec<AttrValue> {
    let s = crate::form::get_field_schema(&T::API_SCHEMA, vec![name]);
    crate::form::enum_items_from_schema(s)
}

fn placeholder_from_schema<T: ApiType>(name: &str) -> String {
    let s = crate::form::get_field_schema(&T::API_SCHEMA, vec![name]);
    crate::form::placeholder_from_schema(s)
}

fn create_firewall_options_loader<F>(url: AttrValue, transform_fn: F) -> ApiLoadCallback<Value>
where
    F: Fn(&mut serde_json::Map<String, Value>) + Clone + 'static,
{
    ApiLoadCallback::new(move || {
        let url = url.clone();
        let transform_fn = transform_fn.clone();
        async move {
            let mut resp = crate::http_get_full(url.to_string(), None).await?;
            if let serde_json::Value::Object(ref mut map) = resp.data {
                transform_fn(map);
            }
            Ok::<_, anyhow::Error>(resp)
        }
    })
}

async fn update_firewall_options(
    form_ctx: FormContext,
    url: AttrValue,
    fields: &[&str],
    transform_fn: Option<impl FnOnce(&mut serde_json::Map<String, Value>)>,
) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();

    if let (Some(transform), serde_json::Value::Object(ref mut map)) = (transform_fn, &mut data) {
        transform(map);
    }

    let data = delete_empty_values(&data, fields, true);

    crate::http_put(&url.to_string(), Some(data)).await
}

/// Properties for the firewall options edit form.
///
/// This component provides a form for editing firewall options at different
/// levels: cluster, node, or guest (LXC/QEMU).
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct EditFirewallOptions {
    /// The firewall context specifying which level to edit (cluster, node, or guest).
    #[builder(IntoPropValue, into_prop_value)]
    pub context: FirewallContext,

    /// Callback invoked when the edit window is closed.
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,
}

impl EditFirewallOptions {
    /// Creates a new `EditFirewallOptions` for editing cluster-level firewall options.
    ///
    /// # Arguments
    ///
    /// * `remote` - The remote identifier for the PVE cluster.
    pub fn cluster(remote: impl Into<AttrValue>) -> Self {
        yew::props!(Self {
            context: FirewallContext::cluster(remote),
        })
    }

    /// Creates a new `EditFirewallOptions` for editing node-level firewall options.
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

    /// Creates a new `EditFirewallOptions` for editing guest-level firewall options.
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

/// Internal component that renders the firewall options edit form.
///
/// This component handles loading firewall options from the API and rendering
/// the appropriate form based on the firewall context (cluster, node, or guest).
pub struct ProxmoxEditFirewallOptions {
    loader: Option<ApiLoadCallback<Value>>,
}

impl Component for ProxmoxEditFirewallOptions {
    type Message = ();
    type Properties = EditFirewallOptions;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        let url: AttrValue = props.context.options_url().into();

        let loader = if !url.is_empty() {
            Some(create_firewall_options_loader(url, |map| {
                // Convert enable field from u64 to bool for cluster firewall
                if let Some(enable_num) = map.get("enable").and_then(|v| v.as_u64()) {
                    map.insert("enable".into(), serde_json::Value::Bool(enable_num != 0));
                }
            }))
        } else {
            None
        };

        Self { loader }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let url: AttrValue = props.context.options_url().into();

        type ContextConfig<'a> = (
            String,
            fn(&FormContext) -> Html,
            &'a [&'a str],
            Option<fn(&mut serde_json::Map<String, Value>)>,
        );
        let (title, renderer, fields, transform_fn): ContextConfig<'_> = match &props.context {
            FirewallContext::Cluster { .. } => (
                props.context.title(&tr!("Edit Cluster Firewall")),
                edit_cluster_firewall_input_panel,
                &[
                    "enable",
                    "ebtables",
                    "policy_in",
                    "policy_out",
                    "policy_forward",
                    "log_ratelimit",
                ],
                Some(|map: &mut serde_json::Map<String, Value>| {
                    if let Some(enable) = map.get("enable").and_then(|v| v.as_bool()) {
                        map.insert("enable".into(), Value::from(u8::from(enable)));
                    }

                    // normalize log_ratelimit property string:
                    // - if present and non-empty: parse and re-serialize
                    // - if missing or empty: use the API default (enable=true, no rate/burst)
                    let ratelimit = map
                        .get("log_ratelimit")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .and_then(|raw| {
                            ClusterFirewallOptionsLogRatelimit::API_SCHEMA
                                .parse_property_string(raw)
                                .ok()
                                .and_then(|parsed| {
                                    serde_json::from_value::<ClusterFirewallOptionsLogRatelimit>(
                                        parsed,
                                    )
                                    .ok()
                                })
                        })
                        .unwrap_or(ClusterFirewallOptionsLogRatelimit {
                            enable: true,
                            rate: None,
                            burst: None,
                        });

                    let mut parts = Vec::new();
                    parts.push(format!("enable={}", if ratelimit.enable { 1 } else { 0 }));
                    if let Some(rate) = ratelimit.rate {
                        parts.push(format!("rate={rate}"));
                    }
                    if let Some(burst) = ratelimit.burst {
                        parts.push(format!("burst={burst}"));
                    }
                    map.insert("log_ratelimit".into(), Value::String(parts.join(",")));
                }),
            ),
            FirewallContext::Node { .. } => (
                props.context.title(&tr!("Edit Node Firewall")),
                edit_node_firewall_input_panel,
                &[
                    "enable",
                    "nosmurfs",
                    "tcpflags",
                    "ndp",
                    "nf_conntrack_max",
                    "nf_conntrack_tcp_timeout_established",
                    "log_level_in",
                    "log_level_out",
                    "log_level_forward",
                    "tcp_flags_log_level",
                    "smurf_log_level",
                    "nftables",
                ],
                None,
            ),
            FirewallContext::Guest { .. } => (
                props.context.title(&tr!("Edit Guest Firewall")),
                edit_guest_firewall_input_panel,
                &[
                    "enable",
                    "dhcp",
                    "ndp",
                    "radv",
                    "macfilter",
                    "ipfilter",
                    "log_level_in",
                    "log_level_out",
                    "policy_in",
                    "policy_out",
                ],
                None,
            ),
        };

        EditWindow::new(title)
            .loader(self.loader.clone())
            .on_close(props.on_close.clone())
            .on_done(props.on_close.clone())
            .renderer(renderer)
            .on_submit({
                let url = url.clone();
                move |form_ctx: FormContext| {
                    let url = url.clone();
                    let fields = fields.to_vec();
                    async move { update_firewall_options(form_ctx, url, &fields, transform_fn).await }
                }
            })
            .into()
    }
}

fn edit_cluster_firewall_input_panel(_form_ctx: &FormContext) -> Html {
    InputPanel::new()
        .padding(4)
        .with_large_field(
            tr!("Enable Firewall"),
            Checkbox::new().name("enable").key("enable"),
        )
        .with_large_field(
            tr!("Enable ebtables"),
            Checkbox::new()
                .name("ebtables")
                .default(true)
                .key("ebtables"),
        )
        .with_field(
            tr!("Input Policy"),
            Combobox::new()
                .name("policy_in")
                .key("policy_in")
                .placeholder("DROP")
                .items(enum_items_from_schema::<ClusterFirewallOptions>("policy_in").into()),
        )
        .with_field(
            tr!("Output Policy"),
            Combobox::new()
                .name("policy_out")
                .key("policy_out")
                .placeholder("ACCEPT")
                .items(enum_items_from_schema::<ClusterFirewallOptions>("policy_out").into()),
        )
        .with_field(
            tr!("Forward Policy"),
            Combobox::new()
                .name("policy_forward")
                .key("policy_forward")
                .placeholder(placeholder_from_schema::<ClusterFirewallOptions>(
                    "policy_forward",
                ))
                .items(enum_items_from_schema::<ClusterFirewallOptions>("policy_forward").into()),
        )
        .with_large_field(
            tr!("Log Rate Limiting"),
            LogRatelimitField::new()
                .name("log_ratelimit")
                .key("log_ratelimit"),
        )
        .into()
}

impl From<EditFirewallOptions> for VNode {
    fn from(val: EditFirewallOptions) -> Self {
        let comp = VComp::new::<ProxmoxEditFirewallOptions>(Rc::new(val), None);
        VNode::from(comp)
    }
}

fn edit_guest_firewall_input_panel(_form_ctx: &FormContext) -> Html {
    InputPanel::new()
        .padding(4)
        .with_field(
            tr!("Enable Firewall"),
            Checkbox::new().name("enable").key("enable"),
        )
        .with_right_field(
            tr!("Enable DHCP"),
            Checkbox::new().name("dhcp").default(true).key("dhcp"),
        )
        .with_field(
            tr!("Enable NDP"),
            Checkbox::new().name("ndp").default(true).key("ndp"),
        )
        .with_right_field(
            tr!("Router Advertisement"),
            Checkbox::new().name("radv").key("radv"),
        )
        .with_field(
            tr!("MAC filter"),
            Checkbox::new()
                .name("macfilter")
                .default(true)
                .key("macfilter"),
        )
        .with_right_field(
            tr!("IP filter"),
            Checkbox::new().name("ipfilter").key("ipfilter"),
        )
        .with_field(
            tr!("Log Level In"),
            Combobox::new()
                .name("log_level_in")
                .key("log_level_in")
                .placeholder(placeholder_from_schema::<GuestFirewallOptions>(
                    "log_level_in",
                ))
                .items(enum_items_from_schema::<GuestFirewallOptions>("log_level_in").into()),
        )
        .with_right_field(
            tr!("Log Level Out"),
            Combobox::new()
                .name("log_level_out")
                .key("log_level_out")
                .placeholder(placeholder_from_schema::<GuestFirewallOptions>(
                    "log_level_out",
                ))
                .items(enum_items_from_schema::<GuestFirewallOptions>("log_level_out").into()),
        )
        .with_field(
            tr!("Input Policy"),
            Combobox::new()
                .name("policy_in")
                .key("policy_in")
                .placeholder("DROP")
                .items(enum_items_from_schema::<GuestFirewallOptions>("policy_in").into()),
        )
        .with_right_field(
            tr!("Output Policy"),
            Combobox::new()
                .name("policy_out")
                .key("policy_out")
                .placeholder("ACCEPT")
                .items(enum_items_from_schema::<GuestFirewallOptions>("policy_out").into()),
        )
        .into()
}

fn edit_node_firewall_input_panel(_form_ctx: &FormContext) -> Html {
    InputPanel::new()
        .padding(4)
        .with_field(
            tr!("Enable Firewall"),
            Checkbox::new().name("enable").default(true).key("enable"),
        )
        .with_right_field(
            tr!("SMURFS filter"),
            Checkbox::new()
                .name("nosmurfs")
                .default(true)
                .key("nosmurfs"),
        )
        .with_field(
            tr!("TCP flags filter"),
            Checkbox::new().name("tcpflags").key("tcpflags"),
        )
        .with_right_field(
            tr!("Enable NDP"),
            Checkbox::new().name("ndp").default(true).key("ndp"),
        )
        .with_field(
            tr!("Connection Tracking Max"),
            Number::<u64>::new()
                .name("nf_conntrack_max")
                .key("nf_conntrack_max")
                .placeholder(placeholder_from_schema::<NodeFirewallOptions>(
                    "nf_conntrack_max",
                )),
        )
        .with_right_field(
            tr!("TCP Timeout Established"),
            Number::<u64>::new()
                .name("nf_conntrack_tcp_timeout_established")
                .key("nf_conntrack_tcp_timeout_established")
                .placeholder(placeholder_from_schema::<NodeFirewallOptions>(
                    "nf_conntrack_tcp_timeout_established",
                )),
        )
        .with_field(
            tr!("Log Level In"),
            Combobox::new()
                .name("log_level_in")
                .key("log_level_in")
                .placeholder(placeholder_from_schema::<NodeFirewallOptions>(
                    "log_level_in",
                ))
                .items(enum_items_from_schema::<NodeFirewallOptions>("log_level_in").into()),
        )
        .with_right_field(
            tr!("Log Level Out"),
            Combobox::new()
                .name("log_level_out")
                .key("log_level_out")
                .placeholder(placeholder_from_schema::<NodeFirewallOptions>(
                    "log_level_out",
                ))
                .items(enum_items_from_schema::<NodeFirewallOptions>("log_level_out").into()),
        )
        .with_field(
            tr!("Log Level Forward"),
            Combobox::new()
                .name("log_level_forward")
                .key("log_level_forward")
                .placeholder(placeholder_from_schema::<NodeFirewallOptions>(
                    "log_level_forward",
                ))
                .items(enum_items_from_schema::<NodeFirewallOptions>("log_level_forward").into()),
        )
        .with_right_field(
            tr!("TCP Flags Log Level"),
            Combobox::new()
                .name("tcp_flags_log_level")
                .key("tcp_flags_log_level")
                .placeholder(placeholder_from_schema::<NodeFirewallOptions>(
                    "tcp_flags_log_level",
                ))
                .items(enum_items_from_schema::<NodeFirewallOptions>("tcp_flags_log_level").into()),
        )
        .with_field(
            tr!("SMURF Log Level"),
            Combobox::new()
                .name("smurf_log_level")
                .key("smurf_log_level")
                .placeholder(placeholder_from_schema::<NodeFirewallOptions>(
                    "smurf_log_level",
                ))
                .items(enum_items_from_schema::<NodeFirewallOptions>("smurf_log_level").into()),
        )
        .with_right_field(
            tr!("nftables (tech preview)"),
            Checkbox::new().name("nftables").key("nftables"),
        )
        .into()
}
