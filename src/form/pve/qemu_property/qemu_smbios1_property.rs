use anyhow::bail;
use regex::Regex;
use serde_json::Value;

use pve_api_types::PveQmSmbios1;

use pwt::prelude::*;
use pwt::widget::form::{Field, TextArea};
use pwt::widget::{Column, InputPanel};

use crate::form::{delete_empty_values, flatten_property_string, property_string_from_parts};
use crate::layout::mobile_form::label_field;
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

thread_local! {
    static UUID_MATCH: Regex = Regex::new(r#"^[a-fA-F0-9]{8}(?:-[a-fA-F0-9]{4}){3}-[a-fA-F0-9]{12}$"#).unwrap();
}

// All base64 encodable properties (without "uuid")
const PROPERTIES: &[&str] = &[
    "manufacturer",
    "product",
    "version",
    "serial",
    "sku",
    "family",
];

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |_| {
        let field_height = "3em";

        let uuid_label = tr!("UUID");
        let uuid_field = Field::new().name("_uuid").validate(|v: &String| {
            if UUID_MATCH.with(|r| r.is_match(v)) {
                return Ok(());
            }
            bail!(
                tr!("Format")
                    + ": xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx (where x is 0-9 or a-f or A-F)"
            )
        });

        let manu_label = tr!("Manufacturer");
        let manu_field = TextArea::new()
            .class("pwt-w-100")
            .name("_manufacturer")
            .style("height", field_height);

        let product_label = tr!("Product");
        let product_field = TextArea::new()
            .class("pwt-w-100")
            .name("_product")
            .style("height", field_height);

        let version_label = tr!("Version");
        let version_field = TextArea::new()
            .class("pwt-w-100")
            .name("_version")
            .style("height", field_height);

        let serial_label = tr!("Serial");
        let serial_field = TextArea::new()
            .class("pwt-w-100")
            .name("_serial")
            .style("height", field_height);

        let sku_label = "SKU";
        let sku_field = TextArea::new()
            .class("pwt-w-100")
            .name("_sku")
            .style("height", field_height);

        let family_label = tr!("Family");
        let family_field = TextArea::new()
            .class("pwt-w-100")
            .name("_family")
            .style("height", field_height);

        if mobile {
            Column::new()
                .gap(2)
                .class(pwt::css::FlexFit)
                .class(pwt::css::AlignItems::Stretch)
                .with_child(label_field(uuid_label, uuid_field, true))
                .with_child(label_field(manu_label, manu_field, true))
                .with_child(label_field(product_label, product_field, true))
                .with_child(label_field(version_label, version_field, true))
                .with_child(label_field(serial_label, serial_field, true))
                .with_child(label_field(sku_label, sku_field, true))
                .with_child(label_field(family_label, family_field, true))
                .into()
        } else {
            InputPanel::new()
                .field_width("300px")
                .class(pwt::css::FlexFit)
                .with_field(uuid_label, uuid_field)
                .with_field(manu_label, manu_field)
                .with_field(product_label, product_field)
                .with_field(version_label, version_field)
                .with_field(serial_label, serial_field)
                .with_field(sku_label, sku_field)
                .with_field(family_label, family_field)
                .into()
        }
    })
}

pub fn qemu_smbios_property(mobile: bool) -> EditableProperty {
    EditableProperty::new("smbios1", tr!("SMBIOS settings (type1)"))
        .required(true)
        .render_input_panel(input_panel(mobile))
        .load_hook(move |mut record: Value| {
            flatten_property_string::<PveQmSmbios1>(&mut record, "smbios1")?;

            // decode base64 encoded properties
            if let Some(Value::Bool(true)) = record.get("_base64") {
                for prop in PROPERTIES.iter().map(|prop| format!("_{prop}")) {
                    if let Some(Value::String(base64)) = record.get(&prop) {
                        if let Ok(bin_data) = proxmox_base64::decode(base64) {
                            record[prop] = String::from_utf8_lossy(&bin_data).into();
                        }
                    }
                }
            }
            Ok(record)
        })
        .submit_hook(move |state: PropertyEditorState| {
            let mut value = state.get_submit_data();
            let mut base64 = false;

            // always base64 encoded properties
            for name in PROPERTIES.iter().map(|n| format!("_{n}")) {
                if let Some(Value::String(utf8)) = value.get(&name) {
                    base64 = true;
                    value[name] = proxmox_base64::encode(utf8).into();
                }
            }
            if base64 {
                value["_base64"] = true.into();
            }
            property_string_from_parts::<PveQmSmbios1>(&mut value, "smbios1", true)?;
            let value = delete_empty_values(&value, &["smbios1"], false);
            Ok(value)
        })
}
