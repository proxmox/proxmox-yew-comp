use proxmox_schema::property_string::PropertyString;
use serde_json::{json, Value};

use pve_api_types::{PveQemuSevFmt, PveQemuSevFmtType};

use pwt::prelude::*;

use pwt::widget::form::{Checkbox, Combobox};
use pwt::widget::{Column, Container, FieldPosition, InputPanel};

use crate::form::{delete_empty_values, flatten_property_string, property_string_from_parts};
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = state.form_ctx;
        let advanced = form_ctx.get_show_advanced();

        let hint = |msg: String| Container::new().class("pwt-color-warning").with_child(msg);

        let amd_sev_type = form_ctx.read().get_field_text("_type");
        let snp_enabled = amd_sev_type == "snp";
        let sev_enabled = !amd_sev_type.is_empty();

        let type_label = tr!("AMD SEV Type");
        let type_field = Combobox::from_key_value_pairs([
            ("std", "AMD SEV"),
            ("es", "AMD SEV-ES (highly experimental)"),
            ("snp", "AMD SEV-SNP (highly experimental)"),
        ])
        .name("_type")
        .force_selection(true)
        .placeholder(format!("{} ({})", tr!("Default"), tr!("Disabled")));

        let debug_hidden = !advanced || !sev_enabled;
        let debug_label = tr!("Allow Debugging");
        let debug_field = Checkbox::new()
            .disabled(!sev_enabled)
            .submit(false)
            .name("_debug");

        let key_sharing_hidden = !advanced || !sev_enabled || snp_enabled;
        let key_sharing_label = tr!("Allow Key-Sharing");
        let key_sharing_field = Checkbox::new()
            .disabled(!sev_enabled || snp_enabled)
            .submit(false)
            .name("_key-sharing");

        let allow_smt_hidden = !advanced || !snp_enabled;
        let allow_smt_label = tr!("Allow SMT");
        let allow_smt_field = Checkbox::new()
            .disabled(!snp_enabled)
            .default(true)
            .submit(false)
            .name("_allow-smt");

        let kernel_hashes_hidden = !advanced || !sev_enabled;
        let kernel_hashes_label = tr!("Enable Kernel Hashes");
        let kernel_hashes_field = Checkbox::new()
            .disabled(!sev_enabled)
            .name("_kernel-hashes")
            .submit(false);

        let hint1 = hint(tr!(
            "WARNING: When using SEV-SNP no EFI disk is loaded as pflash."
        ))
        .key("hint1")
        .class((!snp_enabled).then(|| pwt::css::Display::None));

        let hint2 = hint(tr!(
            "Note: SEV-SNP requires host kernel version 6.11 or higher."
        ))
        .key("hint2")
        .class((!snp_enabled).then(|| pwt::css::Display::None));

        if mobile {
            Column::new()
                .style("min-width", (!mobile).then(|| "500px"))
                .gap(2)
                .padding_x(2)
                .padding_bottom(1) // avoid scrollbar ?!
                .with_child(type_field)
                .with_child(
                    debug_field
                        .box_label(debug_label)
                        .class(debug_hidden.then(|| pwt::css::Display::None)),
                )
                .with_child(
                    key_sharing_field
                        .box_label(key_sharing_label)
                        .class(key_sharing_hidden.then(|| pwt::css::Display::None)),
                )
                .with_child(
                    allow_smt_field
                        .box_label(allow_smt_label)
                        .class(allow_smt_hidden.then(|| pwt::css::Display::None)),
                )
                .with_child(
                    kernel_hashes_field
                        .box_label(kernel_hashes_label)
                        .class(kernel_hashes_hidden.then(|| pwt::css::Display::None)),
                )
                .with_child(hint1)
                .with_child(hint2)
                .into()
        } else {
            InputPanel::new()
                .show_advanced(advanced)
                .label_width("max-content")
                .field_width("350px")
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .padding_bottom(1) // avoid scrollbar
                .with_field(type_label, type_field)
                .with_custom_child_and_options(FieldPosition::Left, false, !snp_enabled, hint1)
                .with_custom_child_and_options(FieldPosition::Left, false, !snp_enabled, hint2)
                .with_advanced_spacer()
                .with_field_and_options(
                    FieldPosition::Left,
                    true,
                    debug_hidden,
                    debug_label,
                    debug_field,
                )
                .with_field_and_options(
                    FieldPosition::Left,
                    true,
                    key_sharing_hidden,
                    key_sharing_label,
                    key_sharing_field,
                )
                .with_field_and_options(
                    FieldPosition::Left,
                    true,
                    allow_smt_hidden,
                    allow_smt_label,
                    allow_smt_field,
                )
                .with_field_and_options(
                    FieldPosition::Left,
                    true,
                    kernel_hashes_hidden,
                    kernel_hashes_label,
                    kernel_hashes_field,
                )
                .into()
        }
    })
}

pub fn qemu_amd_sev_property(mobile: bool) -> EditableProperty {
    EditableProperty::new("amd-sev", tr!("AMD SEV"))
        .advanced_checkbox(true)
        .required(true)
        .placeholder(format!("{} ({})", tr!("Default"), tr!("Disabled")))
        .render_input_panel(input_panel(mobile))
        .renderer(|_, v, _| {
            match serde_json::from_value::<Option<PropertyString<PveQemuSevFmt>>>(v.clone()) {
                Ok(Some(data)) => {
                    let text = match data.ty {
                        PveQemuSevFmtType::Std => "AMD SEV",
                        PveQemuSevFmtType::Es => "AMD SEV-ES",
                        PveQemuSevFmtType::Snp => "AMD SEV-SNP",
                    };
                    format!("{text} ({v})").into()
                }
                _ => v.into(),
            }
        })
        .load_hook({
            move |mut record| {
                flatten_property_string::<PveQemuSevFmt>(&mut record, "amd-sev")?;

                let no_debug = record["_no-debug"].as_bool().unwrap_or(false);
                record["_debug"] = (!no_debug).into();

                let no_key_sharing = record["_no-key-sharing"].as_bool().unwrap_or(false);
                record["_key-sharing"] = (!no_key_sharing).into();

                Ok(record)
            }
        })
        .submit_hook({
            move |state: PropertyEditorState| {
                let form_ctx = state.form_ctx;
                let mut form_data = form_ctx.get_submit_data();
                let ty = match form_data.get("_type") {
                    Some(Value::String(ty)) => ty.clone(),
                    _ => String::new(),
                };
                if ty.is_empty() {
                    return Ok(json!({"delete": "amd-sev"}));
                }

                let debug = form_ctx.read().get_field_checked("_debug");
                if !debug {
                    form_data["_no-debug"] = true.into();
                }

                let key_sharing = form_ctx.read().get_field_checked("_key-sharing");
                if !key_sharing && ty != "snp" {
                    form_data["_no-key-sharing"] = true.into();
                }

                let allow_smt = form_ctx.read().get_field_checked("_allow-smt");
                if !allow_smt && ty == "snp" {
                    form_data["_allow-smt"] = false.into();
                }

                if form_ctx.read().get_field_checked("_kernel-hashes") {
                    form_data["_kernel-hashes"] = true.into();
                }

                property_string_from_parts::<PveQemuSevFmt>(&mut form_data, "amd-sev", true)?;
                let form_data = delete_empty_values(&form_data, &["amd-sev"], false);
                Ok(form_data)
            }
        })
}
