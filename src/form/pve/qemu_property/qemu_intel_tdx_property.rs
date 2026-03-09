use proxmox_schema::property_string::PropertyString;
use serde_json::{json, Value};

use pve_api_types::{PveQemuTdxFmt, PveQemuTdxFmtType};

use pwt::prelude::*;

use pwt::widget::form::{Checkbox, Combobox, Number};
use pwt::widget::{Container, FieldPosition, InputPanel};

use crate::form::{delete_empty_values, flatten_property_string, property_string_from_parts};
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

const INTEL_TDX_PN: &str = "intel-tdx";
const TYPE_PN: &str = "_type";
const ATTESTATION_PN: &str = "_attestation";
const VSOCK_CID_PN: &str = "_vsock-cid";
const VSOCK_PORT_PN: &str = "_vsock-port";

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = state.form_ctx;
        let advanced = form_ctx.get_show_advanced();

        let attestation_enabled = form_ctx.read().get_field_checked(ATTESTATION_PN);
        let intel_tdx_type = form_ctx.read().get_field_text(TYPE_PN);
        let tdx_enabled = !intel_tdx_type.is_empty();

        let hint = |msg: String| Container::new().class("pwt-color-warning").with_child(msg);

        let type_label = tr!("Intel TDX Type");
        let type_field = Combobox::from_key_value_pairs([("tdx", "Intel TDX")])
            .name(TYPE_PN)
            .force_selection(true)
            .placeholder(format!("{} ({})", tr!("Default"), tr!("Disabled")));

        let attestation_label = tr!("Enable Attestation");
        let attestation_field = Checkbox::new()
            .name(ATTESTATION_PN)
            .switch(mobile)
            .default(true)
            .disabled(!tdx_enabled);

        let cid_label = tr!("CID");
        let cid_field = Number::<u64>::new()
            .name(VSOCK_CID_PN)
            .min(2)
            .default(2)
            .disabled(!(attestation_enabled && tdx_enabled))
            .required(true);

        let port_label = tr!("Port");
        let port_field = Number::<u64>::new()
            .name(VSOCK_PORT_PN)
            .default(4050)
            .disabled(!(attestation_enabled && tdx_enabled))
            .required(true);

        let hint1 = hint(tr!(
            "WARNING: When using Intel TDX no EFI disk is loaded as pflash."
        ))
        .key("hint1");

        let hint2 = hint(tr!(
            "Note: Intel TDX is only supported by specific recent CPU models and requires host kernel version 6.16 or higher."
        ))
        .key("hint2");

        InputPanel::new()
            .mobile(mobile)
            .show_advanced(advanced)
            .label_width("max-content")
            .field_width((!mobile).then(|| "350px"))
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .padding_bottom(1) // avoid scrollbar
            .with_field(type_label, type_field)
            .with_custom_child_and_options(FieldPosition::Left, false, !tdx_enabled, hint1)
            .with_custom_child_and_options(FieldPosition::Left, false, !tdx_enabled, hint2)
            .with_advanced_spacer()
            .with_single_line_field(true, !tdx_enabled, attestation_label, attestation_field)
            .with_field_and_options(
                FieldPosition::Left,
                true,
                !tdx_enabled,
                cid_label,
                cid_field,
            )
            .with_field_and_options(
                FieldPosition::Left,
                true,
                !tdx_enabled,
                port_label,
                port_field,
            )
            .into()
    })
}

pub fn qemu_intel_tdx_property(mobile: bool) -> EditableProperty {
    let placeholder = format!("{} ({})", tr!("Default"), tr!("Disabled"));
    EditableProperty::new(INTEL_TDX_PN, tr!("Intel Trust Domain Extension (TDX)"))
        .advanced_checkbox(true)
        .required(true)
        .render_input_panel(input_panel(mobile))
        .renderer(move |_, v, _| {
            if v == &Value::Null {
                placeholder.clone().into()
            } else {
                match serde_json::from_value::<Option<PropertyString<PveQemuTdxFmt>>>(v.clone()) {
                    Ok(Some(data)) => {
                        let text = match data.ty {
                            PveQemuTdxFmtType::Tdx => "Intel TDX",
                            PveQemuTdxFmtType::UnknownEnumValue(_) => "Unknown",
                        };
                        format!("{text} ({v})").into()
                    }
                    _ => v.into(),
                }
            }
        })
        .load_hook({
            move |mut record| {
                flatten_property_string::<PveQemuTdxFmt>(&mut record, INTEL_TDX_PN)?;

                Ok(record)
            }
        })
        .submit_hook({
            move |state: PropertyEditorState| {
                let form_ctx = state.form_ctx;
                let mut form_data = form_ctx.get_submit_data();
                let ty = match form_data.get(TYPE_PN) {
                    Some(Value::String(ty)) => ty.clone(),
                    _ => String::new(),
                };
                if ty.is_empty() {
                    return Ok(json!({"delete": [ INTEL_TDX_PN ] }));
                }

                property_string_from_parts::<PveQemuTdxFmt>(&mut form_data, INTEL_TDX_PN, true)?;
                let form_data = delete_empty_values(&form_data, &[INTEL_TDX_PN], false);
                Ok(form_data)
            }
        })
}
