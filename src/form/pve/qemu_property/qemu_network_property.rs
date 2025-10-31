use anyhow::{bail, Error};
use serde_json::Value;

use pve_api_types::QemuConfigNet;

use pwt::prelude::*;
use pwt::widget::form::{Checkbox, Combobox, Field, Number};
use pwt::widget::InputPanel;

use crate::form::{
    flatten_property_string, property_string_add_missing_data, property_string_from_parts,
};

use crate::form::delete_empty_values;
use crate::form::pve::{PveNetworkSelector, PveVlanField};
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

fn mtu_label() -> String {
    tr!("MTU")
}

fn mtu_field() -> Number<u16> {
    Number::<u16>::new()
        .name("_mtu")
        .placeholder("Same as bridge")
        .min(1)
        .max(65520)
        .validate(|val: &u16| {
            if *val >= 576 || *val == 1 {
                return Ok(());
            }
            bail!("MTU needs to be >= 576 or 1 to inherit the MTU from the underlying bridge.");
        })
}

fn rate_label() -> String {
    tr!("Rate limit") + " (MB/s)"
}

fn rate_field() -> Number<f64> {
    Number::<f64>::new()
        .name("_rate")
        .placeholder(tr!("unlimited"))
        .min(0.0)
        .max(10.0 * 1024.0)
}

fn multiqueue_label() -> String {
    tr!("Multiqueue")
}

fn multiqueue_field() -> Number<u8> {
    Number::<u8>::new().name("_queues").min(1).max(64)
}

fn input_panel(node: Option<AttrValue>, mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let advanced = state.form_ctx.get_show_advanced();
        let panel = InputPanel::new()
            .mobile(mobile)
            .show_advanced(advanced)
            .label_width("max-content")
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .with_field(
                tr!("Bridge"),
                PveNetworkSelector::new()
                    .node(node.clone())
                    .name("_bridge")
                    .required(true),
            )
            .with_right_field(
                tr!("Model"),
                Combobox::from_key_value_pairs([
                    ("e1000", String::from("Intel E1000")),
                    ("e1000e", String::from("Intel E1000E")),
                    (
                        "virtio",
                        String::from("VirtIO (") + &tr!("paravirtualized") + ")",
                    ),
                    ("rtl8139", String::from("Realtek RTL8139")),
                    ("vmxnet3", String::from("VMware vmxnet3")),
                ])
                .name("_model")
                .required(true)
                .default("virtio"),
            )
            .with_field(
                PveVlanField::get_std_label(),
                PveVlanField::new().name("_tag"),
            )
            .with_right_field(
                tr!("MAC address"),
                Field::new().name("_macaddr").placeholder("auto"),
            )
            .with_single_line_field(
                false,
                false,
                tr!("Firewall"),
                Checkbox::new()
                    .switch(mobile)
                    .name("_firewall")
                    .default(true),
            );

        let disconnect_label = tr!("Disconnect");
        let disconnect_field = Checkbox::new().switch(mobile).name("_link_down");

        if mobile {
            return panel
                .with_single_line_field(false, false, disconnect_label, disconnect_field)
                .into();
        }

        panel
            .with_advanced_spacer()
            .with_field_and_options(
                pwt::widget::FieldPosition::Left,
                true,
                false,
                disconnect_label,
                disconnect_field,
            )
            .with_field_and_options(
                pwt::widget::FieldPosition::Right,
                true,
                false,
                rate_label(),
                rate_field(),
            )
            .with_field_and_options(
                pwt::widget::FieldPosition::Left,
                true,
                false,
                mtu_label(),
                mtu_field(),
            )
            .with_field_and_options(
                pwt::widget::FieldPosition::Right,
                true,
                false,
                multiqueue_label(),
                multiqueue_field(),
            )
            .into()
    })
}

fn find_free_network(record: &Value) -> Result<String, Error> {
    if let Some(map) = record.as_object() {
        for i in 0..16 {
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

pub fn qemu_network_property(
    name: Option<String>,
    node: Option<AttrValue>,
    mobile: bool,
) -> EditableProperty {
    let mut title = tr!("Network Device");
    if let Some(name) = name.as_deref() {
        title = title + " (" + name + ")";
    }
    EditableProperty::new(name.clone(), title)
        .advanced_checkbox(!mobile)
        .render_input_panel(input_panel(node.clone(), mobile))
        .submit_hook({
            let name = name.clone();
            move |state: PropertyEditorState| {
                let mut data = state.get_submit_data();
                let network = find_free_network(&state.record)?;
                let name = name.clone().unwrap_or(network);
                property_string_add_missing_data::<QemuConfigNet>(
                    &mut data,
                    &state.record,
                    &state.form_ctx,
                )?;

                if let Value::Bool(false) = data["_link_down"] {
                    data["_link_down"] = Value::Null; // do not set unnecessary value
                }

                property_string_from_parts::<QemuConfigNet>(&mut data, &name, true)?;
                data = delete_empty_values(&data, &[&name], false);
                Ok(data)
            }
        })
        .load_hook({
            let name = name.clone();
            move |mut record: Value| {
                if let Some(name) = name.as_deref() {
                    flatten_property_string::<QemuConfigNet>(&mut record, name)?;
                } else {
                    let _ = find_free_network(&record)?; // test early
                }
                Ok(record)
            }
        })
}

fn mtu_input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |_| {
        InputPanel::new()
            .mobile(mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .with_field(mtu_label(), mtu_field())
            .with_field(rate_label(), rate_field())
            .with_field(multiqueue_label(), multiqueue_field())
            .into()
    })
}

pub fn qemu_network_mtu_property(
    name: Option<String>,
    node: Option<AttrValue>,
    mobile: bool,
) -> EditableProperty {
    let mut property = qemu_network_property(name.clone(), node, mobile)
        .render_input_panel(mtu_input_panel(mobile));

    let mut title = format!("MTU, {}, Multiqueue", tr!("Rate limit"));
    if let Some(name) = name.as_deref() {
        title = title + " (" + name + ")";
    }
    property.title = title.into();
    property
}
