use proxmox_schema::property_string::PropertyString;
use pve_api_types::QemuConfigAgent;

use pwt::prelude::*;
use pwt::widget::form::{Checkbox, Combobox};
use pwt::widget::{Container, FieldPosition, InputPanel};
use serde_json::Value;

use crate::form::{property_string_load_hook, property_string_submit_hook};
use crate::utils::render_boolean;
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

fn renderer(_name: &str, value: &Value, _record: &Value) -> Html {
    let qga: Result<PropertyString<QemuConfigAgent>, _> = serde_json::from_value(value.clone());

    match qga {
        Ok(qga) => {
            if !qga.enabled {
                return tr!("Disabled").into();
            }
            let mut parts = Vec::new();
            parts.push(tr!("Enabled"));

            if let Some(ty) = qga.ty {
                parts.push(ty.to_string());
            }
            if let Some(enabled) = qga.fstrim_cloned_disks {
                parts.push(format!("fstrim-cloned-disks: {}", render_boolean(enabled)));
            }
            if let Some(false) = qga.freeze_fs_on_backup {
                parts.push(format!("freeze-fs-on-backup: {}", render_boolean(false)));
            }
            parts.join(", ").into()
        }
        Err(err) => {
            log::error!("failed to parse qemu agent property: {err}");
            match value {
                Value::String(s) => s.into(),
                _ => value.into(),
            }
        }
    }
}

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = state.form_ctx;
        let advanced = form_ctx.get_show_advanced();
        let enabled = form_ctx.read().get_field_checked("_enabled");
        let ffob_enabled = form_ctx.read().get_field_checked("_freeze-fs-on-backup");

        let warning = |msg: String| -> Container {
            Container::new().class("pwt-color-warning").with_child(msg)
        };

        let hint1_enabled = ffob_enabled;
        let hint1 = warning(tr!("Freeze/thaw for guest filesystems disabled. This can lead to inconsistent disk backups.")).key("hint1");

        let hint2_enabled = !enabled;
        let hint2 =
            warning(tr!("Make sure the QEMU Guest Agent is installed in the VM")).key("hint2");

        InputPanel::new()
            .mobile(mobile)
            .show_advanced(advanced)
            .label_width("300px")
            .padding_x(2)
            .padding_bottom(1) // avoid scroll
            .class(pwt::css::FlexFit)
            .with_single_line_field(
                false,
                false,
                tr!("Use QEMU Guest Agent"),
                Checkbox::new().switch(mobile).name("_enabled"),
            )
            .with_single_line_field(
                false,
                false,
                tr!("Run guest-trim after a disk move or VM migration"),
                Checkbox::new()
                    .switch(mobile)
                    .name("_fstrim_cloned_disks")
                    .disabled(!enabled),
            )
            .with_single_line_field(
                false,
                false,
                tr!("Freeze/thaw guest filesystems on backup for consistency"),
                Checkbox::new()
                    .switch(mobile)
                    .name("_freeze-fs-on-backup")
                    .disabled(!enabled),
            )
            .with_advanced_field(
                tr!("Type"),
                Combobox::from_key_value_pairs([("virtio", "VirtIO"), ("isa", "ISA")])
                    .name("_type")
                    .placeholder(tr!("Default") + " (VirtIO)"),
            )
            .with_custom_child_and_options(FieldPosition::Left, false, hint1_enabled, hint1)
            .with_custom_child_and_options(FieldPosition::Left, false, hint2_enabled, hint2)
            .into()
    })
}

pub fn qemu_agent_property(mobile: bool) -> EditableProperty {
    let name = String::from("agent");
    EditableProperty::new(name.clone(), tr!("QEMU Guest Agent"))
        .advanced_checkbox(true)
        .required(true)
        .placeholder(format!("{} ({})", tr!("Default"), tr!("Disabled")))
        .renderer(renderer)
        .render_input_panel(input_panel(mobile))
        .load_hook(property_string_load_hook::<QemuConfigAgent>(&name))
        .submit_hook(property_string_submit_hook::<QemuConfigAgent>(&name, true))
}
