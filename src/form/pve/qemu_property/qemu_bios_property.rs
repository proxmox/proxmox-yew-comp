use pwt::prelude::*;
use pwt::widget::form::Combobox;
use pwt::widget::{Container, FieldPosition, InputPanel};

use pve_api_types::QemuConfigBios;

use crate::form::delete_empty_values;
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = state.form_ctx;
        let show_efi_disk_hint =
            form_ctx.read().get_field_text("bios") == "ovmf" && state.record["efidisk0"].is_null();

        let hint = |msg: String| Container::new().class("pwt-color-warning").with_child(msg);

        let efidisk_hint = hint(tr!(
            "You need to add an EFI disk for storing the EFI settings. See the online help for details."
        )).key("efidisk_hint");

        let bios_label = "BIOS";
        let bios_field =
            Combobox::from_key_value_pairs([("ovmf", "OVMF (UEFI)"), ("seabios", "SeaBIOS")])
                .name("bios")
                .key("bios")
                .submit_empty(true)
                .placeholder("SeaBIOS");

        let mut panel = InputPanel::new()
            .mobile(mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .padding_bottom(1); // avoid scrollbar ?!

        if mobile {
            panel.add_custom_child(bios_field);
        } else {
            panel.add_field(bios_label, bios_field);
        }

        panel
            .with_custom_child_and_options(
                FieldPosition::Left,
                false,
                !show_efi_disk_hint,
                efidisk_hint,
            )
            .into()
    })
}

pub fn qemu_bios_property(mobile: bool) -> EditableProperty {
    EditableProperty::new("bios", "BIOS")
        .required(true)
        .placeholder(tr!("Default") + " (SeaBIOS)")
        .renderer(
            |_, v, _| match serde_json::from_value::<QemuConfigBios>(v.clone()) {
                Ok(bios) => match bios {
                    QemuConfigBios::Seabios => "SeaBIOS".into(),
                    QemuConfigBios::Ovmf => "OVMF (UEFI)".into(),
                    QemuConfigBios::UnknownEnumValue(value) => format!("unknown '{value}'").into(),
                },
                Err(_) => v.into(),
            },
        )
        .render_input_panel(input_panel(mobile))
        .submit_hook(move |state: PropertyEditorState| {
            let mut data = state.get_submit_data();
            data = delete_empty_values(&data, &["bios"], false);
            Ok(data)
        })
}
