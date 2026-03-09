use anyhow::{bail, Error};
use proxmox_schema::property_string::PropertyString;
use proxmox_schema::{ApiType, Schema};
use serde_json::{from_value, json, Value};

use pwt::prelude::*;
use pwt::props::PwtSpace;
use pwt::widget::form::{Field, Number, RadioButton};
use pwt::widget::Row;
use pwt::widget::{form::Checkbox, InputPanel};

use pve_api_types::{LxcConfigNet, LxcConfigNetArray};

use crate::form::pve::PveNetworkSelector;
use crate::form::{
    delete_default_values, delete_empty_values, flatten_property_string,
    property_string_add_missing_data, property_string_from_parts,
};
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn, SchemaValidation};

const NAME_PN: &str = "_name";
const IP_PN: &str = "_ip";
const IP6_PN: &str = "_ip6";
const GW_PN: &str = "_gw";
const GW6_PN: &str = "_gw6";
const HWADDR_PN: &str = "_hwaddr";
const BRIDGE_PN: &str = "_bridge";
const TAG_PN: &str = "_tag";
const RATE_PN: &str = "_rate";
const MTU_PN: &str = "_mtu";
const HOST_MANAGED_PN: &str = "_host-managed";

const FIREWALL_PN: &str = "_firewall";
const DISCONNECT_PN: &str = "_link_down";

const IPV4_MODE_FIELD_NAME: &str = "_ipv4_mode_";
const IPV6_MODE_FIELD_NAME: &str = "_ipv6_mode_";

fn get_schema(name: &str) -> &'static Schema {
    let object_schema = LxcConfigNet::API_SCHEMA.unwrap_object_schema();
    let (_optional, schema) = super::lookup_object_property_schema(object_schema, name).unwrap();
    schema
}

fn net_property_id(name: &str) -> Option<usize> {
    if let Some(name) = name.strip_prefix("net") {
        if let Ok(id) = name.parse::<usize>() {
            return Some(id);
        }
    }
    None
}

fn validate_network_name(value: &str, name: Option<&str>, record: &Value) -> Result<(), Error> {
    let map = match record.as_object() {
        Some(map) => map,
        None => return Ok(()),
    };

    for (k, v) in map {
        if net_property_id(k).is_none() || name == Some(k) {
            continue;
        }
        if let Ok(Some(net)) = from_value::<Option<PropertyString<LxcConfigNet>>>(v.clone()) {
            let net = net.into_inner();
            if net.name == *value {
                bail!(tr!("interface name already in use"));
            }
        }
    }

    Ok(())
}

fn input_panel(
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    name: Option<String>,
    mobile: bool,
) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = state.form_ctx;
        let advanced = form_ctx.get_show_advanced();

        let ipv4_mode = form_ctx.read().get_field_text(IPV4_MODE_FIELD_NAME);
        let ipv6_mode = form_ctx.read().get_field_text(IPV6_MODE_FIELD_NAME);

        let name_label = tr!("Name");
        let name_field = Field::new()
            .name(NAME_PN)
            .placeholder(tr!("(e.g. eth0)"))
            .validate({
                let record = state.record.clone();
                let name = name.clone();
                move |v: &String| validate_network_name(v, name.as_deref(), &record)
            })
            .required(true);

        let firewall_label = tr!("Firewall");
        let firewall_field = Checkbox::new().name(FIREWALL_PN);

        let disconnect_label = tr!("Disconnect");
        let disconnect_field = Checkbox::new().name(DISCONNECT_PN);

        let host_managed_label = tr!("Host-Managed");
        let host_managed_field = Checkbox::new().name(HOST_MANAGED_PN);

        let ipv4_mode_child = {
            Row::new()
                .class(pwt::css::AlignItems::Center)
                .gap(PwtSpace::Em(1.0))
                .with_child("IPv4:") // do not localize
                .with_child(
                    RadioButton::new("static")
                        .name(IPV4_MODE_FIELD_NAME)
                        .box_label("Static")
                        .submit(false),
                )
                .with_child(
                    RadioButton::new("dhcp")
                        .name(IPV4_MODE_FIELD_NAME)
                        .box_label("DHCP") // do not localize
                        .submit(false),
                )
        };

        let ipv6_mode_child = {
            Row::new()
                .class(pwt::css::AlignItems::Center)
                .gap(PwtSpace::Em(1.0))
                .with_child("IPv6:") // do not localize
                .with_child(
                    RadioButton::new("static")
                        .name(IPV6_MODE_FIELD_NAME)
                        .box_label("Static")
                        .submit(false),
                )
                .with_child(
                    RadioButton::new("dhcp")
                        .name(IPV6_MODE_FIELD_NAME)
                        .box_label("DHCP") // do not localize
                        .submit(false),
                )
                .with_child(
                    RadioButton::new("auto")
                        .name(IPV6_MODE_FIELD_NAME)
                        .box_label("SLAAC") // do not localize
                        .submit(false),
                )
        };

        let ipv4_cidr_label = "IPv4/CIDR"; //  do not localize
        let ipv4_cidr_field = Field::new()
            .name(IP_PN)
            .schema(get_schema("ip"))
            .submit_empty(true)
            .disabled(ipv4_mode != "static");

        let gateway_label = tr!("Gateway") + " (IPv4)";
        let gateway_field = Field::new()
            .name(GW_PN)
            .schema(get_schema("gw"))
            .submit_empty(true)
            .disabled(ipv4_mode != "static");

        let ipv6_cidr_label = "IPv6/CIDR"; //  do not localize
        let ipv6_cidr_field = Field::new()
            .name(IP6_PN)
            .schema(get_schema("ip6"))
            .submit_empty(true)
            .disabled(ipv6_mode != "static");

        let gateway6_label = tr!("Gateway") + " (IPv6)";
        let gateway6_field = Field::new()
            .name(GW6_PN)
            .schema(get_schema("gw6"))
            .submit_empty(true)
            .disabled(ipv6_mode != "static");

        let bridge_label = tr!("Bridge");
        let bridge_field = PveNetworkSelector::new()
            .node(node.clone())
            .remote(remote.clone())
            .name(BRIDGE_PN)
            .required(true);

        let hwaddr_label = tr!("MAC address");
        let hwaddr_field = Field::new()
            .name(HWADDR_PN)
            .placeholder("auto")
            .schema(get_schema("hwaddr"))
            .submit_empty(true);

        let tag_label = tr!("VLAN Tag");
        let tag_field = Number::<u16>::new()
            .name(TAG_PN)
            .min(1)
            .max(4094)
            .placeholder("no VLAN")
            .submit_empty(true);

        let rate_label = tr!("Rate limit") + " (MB/s)";
        let rate_field = Number::<f64>::new()
            .name(RATE_PN)
            .min(0.0)
            .max(10.0 * 1024.0)
            .placeholder(tr!("unlimited"))
            .submit_empty(true);

        let mtu_label = tr!("MTU");
        let mtu_field = Number::<u32>::new()
            .name(MTU_PN)
            .min(64)
            .placeholder(tr!("Same as bridge"));

        InputPanel::new()
            .class(pwt::css::FlexFit)
            .mobile(mobile)
            .show_advanced(advanced)
            .padding_x(2)
            .field_width((!mobile).then_some("300px"))
            .with_field(name_label, name_field)
            .with_field(hwaddr_label, hwaddr_field)
            .with_field(bridge_label, bridge_field)
            .with_field(tag_label, tag_field)
            .with_right_custom_child(ipv4_mode_child)
            .with_right_field(ipv4_cidr_label, ipv4_cidr_field)
            .with_right_field(gateway_label, gateway_field)
            .with_right_custom_child(ipv6_mode_child)
            .with_right_field(ipv6_cidr_label, ipv6_cidr_field)
            .with_right_field(gateway6_label, gateway6_field)
            .with_single_line_field(false, false, firewall_label, firewall_field)
            .with_advanced_spacer()
            .with_single_line_field(true, false, disconnect_label, disconnect_field)
            .with_field_and_options(
                pwt::widget::FieldPosition::Right,
                true,
                false,
                rate_label,
                rate_field,
            )
            .with_advanced_field(mtu_label, mtu_field)
            .with_field_and_options(
                pwt::widget::FieldPosition::Right,
                true,
                false,
                host_managed_label,
                host_managed_field,
            )
            .into()
    })
}

fn find_free_network(record: &Value) -> Result<String, Error> {
    if let Some(map) = record.as_object() {
        for i in 0..LxcConfigNetArray::MAX {
            let name = format!("net{i}");
            if !map.contains_key(&name) {
                return Ok(name);
            }
        }
        bail!(tr!("All network devices in use."));
    } else {
        Ok("net0".into())
    }
}

pub fn lxc_network_property(
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    name: Option<String>,
    mobile: bool,
) -> EditableProperty {
    let mut title = tr!("Network Device");
    if let Some(name) = name.as_deref() {
        title = title + " (" + name + ")";
    }
    EditableProperty::new(name.clone(), title)
        .required(true)
        .advanced_checkbox(true)
        .render_input_panel(input_panel(
            node.clone(),
            remote.clone(),
            name.clone(),
            mobile,
        ))
        .load_hook({
            let name = name.clone();
            move |mut record: Value| {
                if let Some(name) = name.as_deref() {
                    flatten_property_string::<LxcConfigNet>(&mut record, name)?;
                } else {
                    let _ = find_free_network(&record)?; // test early
                }

                record[IPV4_MODE_FIELD_NAME] = match &record[IP_PN] {
                    Value::String(s) if s == "dhcp" => {
                        let mode = s.to_owned().into();
                        record[IP_PN] = "".into();
                        mode
                    }
                    _ => "static".into(),
                };
                record[IPV6_MODE_FIELD_NAME] = match &record[IP6_PN] {
                    Value::String(s) if s == "dhcp" || s == "auto" => {
                        let mode = s.to_owned().into();
                        record[IP6_PN] = "".into();
                        mode
                    }
                    _ => "static".into(),
                };

                Ok(record)
            }
        })
        .submit_hook({
            let name = name.clone();
            move |state: PropertyEditorState| {
                let form_ctx = state.form_ctx.clone();
                let mut data = form_ctx.get_submit_data();
                let network = find_free_network(&state.record)?;
                let name = name.clone().unwrap_or(network);
                property_string_add_missing_data::<LxcConfigNet>(
                    &mut data,
                    &state.record,
                    &state.form_ctx,
                )?;

                let ipv4_mode = form_ctx.read().get_field_text(IPV4_MODE_FIELD_NAME);
                if ipv4_mode != "static" {
                    data[IP_PN] = ipv4_mode.into();
                }

                let ipv6_mode = form_ctx.read().get_field_text(IPV6_MODE_FIELD_NAME);
                if ipv6_mode != "static" {
                    data[IP6_PN] = ipv6_mode.into();
                }

                let defaults = json!({
                    DISCONNECT_PN: false,
                    FIREWALL_PN: false,
                    HOST_MANAGED_PN: false,
                });
                delete_default_values(&mut data, &defaults);

                property_string_from_parts::<LxcConfigNet>(&mut data, &name, true)?;
                data = delete_empty_values(&data, &[&name], false);
                Ok(data)
            }
        })
}
