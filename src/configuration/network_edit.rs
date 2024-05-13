use std::rc::Rc;

use anyhow::Error;
use serde_json::Value;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::props::LoadCallback;
use pwt::widget::form::{delete_empty_values, Boolean, Field, FormContext, Number};
use pwt::widget::InputPanel;

use crate::utils::json_array_to_flat_string;

use crate::{BondModeSelector, BondXmitHashPolicySelector, EditWindow, SchemaValidation};

use proxmox_system_config_api::network::{
    NetworkInterfaceType,
    CIDR_V4_SCHEMA, CIDR_V6_SCHEMA, IP_V4_SCHEMA, IP_V6_SCHEMA,
};

use crate::percent_encoding::percent_encode_component;
use pwt_macros::builder;


async fn load_item(name: AttrValue) -> Result<Value, Error> {
    let url = format!(
        "/nodes/localhost/network/{}",
        percent_encode_component(&name)
    );
    let mut data: Value = crate::http_get(url, None).await?;

    if let Value::Array(bridge_ports) = &data["bridge_ports"] {
        data["bridge_ports"] = json_array_to_flat_string(bridge_ports).into();
    }
    if let Value::Array(slaves) = &data["slaves"] {
        data["slaves"] = json_array_to_flat_string(slaves).into();
    }

    // fix backup-server 3.0-1 API bug (spurious NULL value)
    if let Value::Object(map) = &mut data {
        if let Value::Null = map["bond_xmit_hash_policy"] {
            map.remove("bond_xmit_hash_policy");
        }
    }

    Ok(data)
}

async fn create_item(
    form_ctx: FormContext,
    interface_type: NetworkInterfaceType,
) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();

    if let Value::Object(map) = &mut data {
        if let Some(name) = map.remove("name") {
            data["iface"] = name;
        }
    }

    data["type"] = serde_json::to_value(&interface_type).unwrap();

    crate::http_post("/nodes/localhost/network", Some(data)).await
}

async fn update_item(form_ctx: FormContext) -> Result<(), Error> {
    let data = form_ctx.get_submit_data();
    let data = delete_empty_values(
        &data,
        &[
            "bridge_vlan_aware",
            "bond_xmit_hash_policy",
            "cidr",
            "cidr6",
            "gateway",
            "gateway6",
            "mtu",
        ],
        true,
    );

    let name = form_ctx.read().get_field_text("name");
    let url = format!(
        "/nodes/localhost/network/{}",
        percent_encode_component(&name)
    );

    crate::http_put(&url, Some(data)).await
}

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct NetworkEdit {
    pub interface_type: NetworkInterfaceType,
    /// Close/Abort callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,

    /// Edit existing server config.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub name: Option<AttrValue>,

    /// Default interface name (for create mode)
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default_name: Option<AttrValue>,
}

impl NetworkEdit {
    pub fn new(interface_type: NetworkInterfaceType) -> Self {
        yew::props!(Self { interface_type })
    }
}

pub struct ProxmoxNetworkEdit {
    loader: Option<LoadCallback<Value>>,
}

fn render_bridge_form(form_ctx: FormContext, props: &NetworkEdit) -> Html {
    let is_edit = props.name.is_some();

    InputPanel::new()
        .show_advanced(form_ctx.get_show_advanced())
        .class("pwt-p-4")
        .with_field(
            tr!("Name"),
            Field::new()
                .name("name")
                .tip(tr!("For example, vmbr0, vmbr0.100, vmbr1, ..."))
                .required(true)
                .default(&props.default_name)
                .disabled(is_edit)
                .submit(!is_edit),
        )
        .with_right_field(
            tr!("Autostart"),
            Boolean::new().name("autostart").default(true),
        )
        .with_field(
            tr!("IPv4/CIDR"),
            Field::new().name("cidr").schema(&CIDR_V4_SCHEMA),
        )
        .with_right_field(tr!("VLAN aware"), Boolean::new().name("bridge_vlan_aware"))
        .with_field(
            tr!("Gateway") + " (IPv4)",
            Field::new().name("gateway").schema(&IP_V4_SCHEMA),
        )
        .with_right_field(
            tr!("Bridge ports"),
            Field::new()
                .name("bridge_ports")
                .tip(tr!("Space-separated list of interfaces, for example: enp0s0 enp1s0"))
        )
        .with_field(
            tr!("IPv6/CIDR"),
            Field::new().name("cidr6").schema(&CIDR_V6_SCHEMA),
        )
        .with_right_field(
            tr!("Comment"),
            Field::new().name("comments").submit_empty(true),
        )
        .with_field(
            tr!("Gateway") + " (IPv6)",
            Field::new().name("gateway6").schema(&IP_V6_SCHEMA),
        )
        .with_advanced_spacer()
        .with_advanced_field(
            tr!("MTU"),
            Number::new().min(1).name("mtu").placeholder("1500"),
        )
        .into()
}

fn render_bond_form(form_ctx: FormContext, props: &NetworkEdit) -> Html {
    let is_edit = props.name.is_some();

    let mode = form_ctx
        .read()
        .get_field_value("bond_mode")
        .map(|v| v.as_str().map(String::from))
        .flatten()
        .unwrap_or(String::new());

    let allow_xmit_hash_policy = mode == "balance-xor" || mode == "802.3ad";

    if !allow_xmit_hash_policy {
        form_ctx
           .write()
           .set_field_value("bond_xmit_hash_policy", "".into())
    }

    let allow_bond_primary = mode == "active-backup";
    if !allow_bond_primary  {
        form_ctx
            .write()
            .set_field_value("bond-primary", "".into())
    }

    InputPanel::new()
        .show_advanced(form_ctx.get_show_advanced())
        .class("pwt-p-4")
        .with_field(
            tr!("Name"),
            Field::new()
                .name("name")
                .default(&props.default_name)
                .tip(tr!("For example, bond0, bond0.100, bond1, ..."))
                .required(true)
                .disabled(is_edit)
                .submit(!is_edit),
        )
        .with_right_field(
            tr!("Autostart"),
            Boolean::new().name("autostart").default(true),
        )
        .with_field(
            tr!("IPv4/CIDR"),
            Field::new().name("cidr").schema(&CIDR_V4_SCHEMA),
        )
        .with_right_field(
            tr!("Slaves"),
            Field::new()
                .name("slaves")
                .tip(tr!("Space-separated list of interfaces, for example: enp0s0 enp1s0"))
        )
        .with_field(
            tr!("Gateway") + " (IPv4)",
            Field::new().name("gateway").schema(&IP_V4_SCHEMA),
        )
        .with_right_field(
            tr!("Mode"),
            BondModeSelector::new()
                .name("bond_mode")
                .default("balance-rr"), //.placeholder(tr!("Default"))
        )
        .with_field(
            tr!("IPv6/CIDR"),
            Field::new().name("cidr6").schema(&CIDR_V6_SCHEMA),
        )
        .with_right_field(
            tr!("Hash policy"),
            BondXmitHashPolicySelector::new()
                .name("bond_xmit_hash_policy")
                .disabled(!allow_xmit_hash_policy),
        )
        .with_field(
            tr!("Gateway") + " (IPv6)",
            Field::new().name("gateway6").schema(&IP_V6_SCHEMA),
        )
        .with_right_field(
            "bond-primary",
            Field::new()
                .name("bond-primary")
                .disabled(!allow_bond_primary),
        )
        .with_right_field(
            tr!("Comment"),
            Field::new().name("comments").submit_empty(true),
        )
        .with_advanced_spacer()
        .with_advanced_field(
            tr!("MTU"),
            Number::new().min(1).name("mtu").placeholder("1500"),
        )
        .into()
}

fn render_common_form(form_ctx: FormContext, props: &NetworkEdit) -> Html {
    let is_edit = props.name.is_some();

    InputPanel::new()
        .show_advanced(form_ctx.get_show_advanced())
        .class("pwt-p-4")
        .with_field(
            tr!("Name"),
            Field::new()
                .name("name")
                .default(&props.default_name)
                .required(true)
                .disabled(is_edit)
                .submit(!is_edit),
        )
        .with_right_field(
            tr!("Autostart"),
            Boolean::new().name("autostart").default(true),
        )
        .with_field(
            tr!("IPv4/CIDR"),
            Field::new().name("cidr").schema(&CIDR_V4_SCHEMA),
        )
        .with_right_field(
            tr!("Comment"),
            Field::new().name("comments").submit_empty(true),
        )
        .with_field(
            tr!("Gateway") + " (IPv6)",
            Field::new().name("gateway6").schema(&IP_V6_SCHEMA),
        )
        .with_field(
            tr!("IPv6/CIDR"),
            Field::new().name("cidr6").schema(&CIDR_V6_SCHEMA),
        )
        .with_field(
            tr!("Gateway") + " (IPv6)",
            Field::new().name("gateway6").schema(&IP_V6_SCHEMA),
        )
        .with_advanced_spacer()
        .with_advanced_field(
            tr!("MTU"),
            Number::new().min(1).name("mtu").placeholder("1500"),
        )
        .into()
}

fn render_input_form(form_ctx: FormContext, props: &NetworkEdit) -> Html {
    match props.interface_type {
        NetworkInterfaceType::Bridge => render_bridge_form(form_ctx, props),
        NetworkInterfaceType::Bond => render_bond_form(form_ctx, props),
        _ => render_common_form(form_ctx, props),
    }
}

impl Component for ProxmoxNetworkEdit {
    type Message = ();
    type Properties = NetworkEdit;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let loader = props.name.as_ref().map(|name| {
            let name = name.clone();
            LoadCallback::new(move || load_item(name.clone()))
        });

        Self { loader }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let is_edit = props.name.is_some();

        let action = if is_edit { tr!("Edit") } else { tr!("Create") };

        let interface_type = props.interface_type;
        let on_submit = move |form_context| async move {
            if is_edit {
                update_item(form_context).await
            } else {
                create_item(form_context, interface_type).await
            }
        };

        let interface_type = crate::utils::format_network_interface_type(props.interface_type);

        EditWindow::new(action + ": " + &interface_type)
            .advanced_checkbox(true)
            .loader(self.loader.clone())
            .renderer({
                let props = props.clone();
                move |form_ctx: &FormContext| render_input_form(form_ctx.clone(), &props)
            })
            .on_done(props.on_close.clone())
            .on_submit(on_submit)
            .into()
    }
}

impl Into<VNode> for NetworkEdit {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxNetworkEdit>(Rc::new(self), None);
        VNode::from(comp)
    }
}
