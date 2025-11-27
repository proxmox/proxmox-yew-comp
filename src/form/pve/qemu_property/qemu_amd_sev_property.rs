use proxmox_schema::property_string::PropertyString;
use serde_json::{json, Value};

use pve_api_types::{PveQemuSevFmt, PveQemuSevFmtType};

use pwt::prelude::*;

use pwt::widget::form::{Checkbox, Combobox};
use pwt::widget::{Container, FieldPosition, InputPanel};

use crate::form::{delete_empty_values, flatten_property_string, property_string_from_parts};
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

const AMD_SEV_PN: &'static str = "amd-sev";
const TYPE_PN: &'static str = "_type";
const NO_DEBUG_PN: &'static str = "_no-debug";
const NO_KEY_SHARING_PN: &'static str = "_no-key-sharing";

const DEBUG_FIELD_NAME: &'static str = "_debug";
const KEY_SHARING_FIELD_NAME: &'static str = "_key-sharing";
const ALLOW_SMT_FIELD_NAME: &'static str = "_allow-smt";
const KERNEL_HASHES_FIELD_NAME: &'static str = "_kernel-hashes";

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = state.form_ctx;
        let advanced = form_ctx.get_show_advanced();

        let hint = |msg: String| Container::new().class("pwt-color-warning").with_child(msg);

        let amd_sev_type = form_ctx.read().get_field_text(TYPE_PN);
        let snp_enabled = amd_sev_type == "snp";
        let sev_enabled = !amd_sev_type.is_empty();

        let type_label = tr!("AMD SEV Type");
        let type_field = Combobox::from_key_value_pairs([
            ("std", "AMD SEV"),
            ("es", "AMD SEV-ES (highly experimental)"),
            ("snp", "AMD SEV-SNP (highly experimental)"),
        ])
        .name(TYPE_PN)
        .force_selection(true)
        .placeholder(format!("{} ({})", tr!("Default"), tr!("Disabled")));

        let debug_hidden = !advanced || !sev_enabled;
        let debug_label = tr!("Allow Debugging");
        let debug_field = Checkbox::new()
            .switch(mobile)
            .disabled(!sev_enabled)
            .submit(false)
            .name(DEBUG_FIELD_NAME);

        let key_sharing_hidden = !advanced || !sev_enabled || snp_enabled;
        let key_sharing_label = tr!("Allow Key-Sharing");
        let key_sharing_field = Checkbox::new()
            .switch(mobile)
            .disabled(!sev_enabled || snp_enabled)
            .submit(false)
            .name(KEY_SHARING_FIELD_NAME);

        let allow_smt_hidden = !advanced || !snp_enabled;
        let allow_smt_label = tr!("Allow SMT");
        let allow_smt_field = Checkbox::new()
            .switch(mobile)
            .disabled(!snp_enabled)
            .default(true)
            .submit(false)
            .name(ALLOW_SMT_FIELD_NAME);

        let kernel_hashes_hidden = !advanced || !sev_enabled;
        let kernel_hashes_label = tr!("Enable Kernel Hashes");
        let kernel_hashes_field = Checkbox::new()
            .switch(mobile)
            .disabled(!sev_enabled)
            .name(KERNEL_HASHES_FIELD_NAME)
            .submit(false);

        let hint1 = hint(tr!(
            "WARNING: When using SEV-SNP no EFI disk is loaded as pflash."
        ))
        .key("hint1");

        let hint2 = hint(tr!(
            "Note: SEV-SNP requires host kernel version 6.11 or higher."
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
            .with_custom_child_and_options(FieldPosition::Left, false, !snp_enabled, hint1)
            .with_custom_child_and_options(FieldPosition::Left, false, !snp_enabled, hint2)
            .with_advanced_spacer()
            .with_single_line_field(true, debug_hidden, debug_label, debug_field)
            .with_single_line_field(
                true,
                key_sharing_hidden,
                key_sharing_label,
                key_sharing_field,
            )
            .with_single_line_field(true, allow_smt_hidden, allow_smt_label, allow_smt_field)
            .with_single_line_field(
                true,
                kernel_hashes_hidden,
                kernel_hashes_label,
                kernel_hashes_field,
            )
            .into()
    })
}

pub fn qemu_amd_sev_property(mobile: bool) -> EditableProperty {
    let placeholder = format!("{} ({})", tr!("Default"), tr!("Disabled"));
    EditableProperty::new(AMD_SEV_PN, tr!("AMD SEV"))
        .advanced_checkbox(true)
        .required(true)
        .render_input_panel(input_panel(mobile))
        .renderer(move |_, v, _| {
            if v == &Value::Null {
                placeholder.clone().into()
            } else {
                match serde_json::from_value::<Option<PropertyString<PveQemuSevFmt>>>(v.clone()) {
                    Ok(Some(data)) => {
                        let text = match data.ty {
                            PveQemuSevFmtType::Std => "AMD SEV",
                            PveQemuSevFmtType::Es => "AMD SEV-ES",
                            PveQemuSevFmtType::Snp => "AMD SEV-SNP",
                            PveQemuSevFmtType::UnknownEnumValue(value) => {
                                &format!("unknown '{value}'")
                            }
                        };
                        format!("{text} ({v})").into()
                    }
                    _ => v.into(),
                }
            }
        })
        .load_hook({
            move |mut record| {
                flatten_property_string::<PveQemuSevFmt>(&mut record, AMD_SEV_PN)?;

                let no_debug = record[NO_DEBUG_PN].as_bool().unwrap_or(false);
                record[DEBUG_FIELD_NAME] = (!no_debug).into();

                let no_key_sharing = record[NO_KEY_SHARING_PN].as_bool().unwrap_or(false);
                record[KEY_SHARING_FIELD_NAME] = (!no_key_sharing).into();

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
                    return Ok(json!({"delete": [ AMD_SEV_PN ] }));
                }

                let debug = form_ctx.read().get_field_checked(DEBUG_FIELD_NAME);
                if !debug {
                    form_data[NO_DEBUG_PN] = true.into();
                }

                let key_sharing = form_ctx.read().get_field_checked(KEY_SHARING_FIELD_NAME);
                if !key_sharing && ty != "snp" {
                    form_data[NO_KEY_SHARING_PN] = true.into();
                }

                let allow_smt = form_ctx.read().get_field_checked(ALLOW_SMT_FIELD_NAME);
                if !allow_smt && ty == "snp" {
                    form_data[ALLOW_SMT_FIELD_NAME] = false.into();
                }

                if form_ctx.read().get_field_checked(KERNEL_HASHES_FIELD_NAME) {
                    form_data[KERNEL_HASHES_FIELD_NAME] = true.into();
                }

                property_string_from_parts::<PveQemuSevFmt>(&mut form_data, AMD_SEV_PN, true)?;
                let form_data = delete_empty_values(&form_data, &[AMD_SEV_PN], false);
                Ok(form_data)
            }
        })
}
