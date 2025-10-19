use std::rc::Rc;

use anyhow::bail;
use proxmox_schema::{ApiType, ObjectSchema, Schema};
use regex::Regex;
use serde_json::Value;

use pwt::prelude::*;
use pwt::widget::form::{delete_empty_values, Field, Number};
use pwt::widget::{Column, InputPanel};

use crate::layout::mobile_form::label_field;
use crate::SchemaValidation;

use pve_api_types::{QemuConfig, StorageContent};

use crate::form::pve::{
    format_hotplug_feature, format_qemu_ostype, property_string_load_hook,
    property_string_submit_hook, BootDeviceList, HotplugFeatureSelector, PveStorageSelector,
    QemuOstypeSelector,
};

use crate::pve_api_types::QemuConfigStartup;
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

mod qemu_disk_property;
pub use qemu_disk_property::{
    extract_used_devices, qemu_cdrom_property, qemu_disk_property, qemu_unused_disk_property,
};

mod qemu_display_property;
pub use qemu_display_property::qemu_display_property;

mod qemu_efidisk_property;
pub use qemu_efidisk_property::qemu_efidisk_property;

mod qemu_machine_property;
pub use qemu_machine_property::qemu_machine_property;

mod qemu_network_property;
pub use qemu_network_property::{qemu_network_mtu_property, qemu_network_property};

mod qemu_scsihw_property;
pub use qemu_scsihw_property::qemu_scsihw_property;

mod qemu_smbios1_property;
pub use qemu_smbios1_property::qemu_smbios_property;

mod qemu_spice_enhancement_property;
pub use qemu_spice_enhancement_property::qemu_spice_enhancement_property;

mod qemu_tpmstate_property;
pub use qemu_tpmstate_property::qemu_tpmstate_property;

mod qemu_amd_sev_property;
pub use qemu_amd_sev_property::qemu_amd_sev_property;

mod qemu_memory_property;
pub use qemu_memory_property::qemu_memory_property;

mod qemu_agent_property;
pub use qemu_agent_property::qemu_agent_property;

mod qemu_bios_property;
pub use qemu_bios_property::qemu_bios_property;

mod qemu_processor_property;
pub use qemu_processor_property::{
    qemu_cpu_flags_property, qemu_kernel_scheduler_property, qemu_sockets_cores_property,
};

fn lookup_schema(name: &str) -> Option<(bool, &'static Schema)> {
    let allof_schema = QemuConfig::API_SCHEMA.unwrap_all_of_schema();

    for entry in allof_schema.list {
        if let Schema::Object(object_schema) = entry {
            if let Some((optional, schema)) = lookup_object_property_schema(&object_schema, name) {
                return Some((optional, schema));
            }
        }
    }
    None
}

fn lookup_object_property_schema(
    object_schema: &ObjectSchema,
    name: &str,
) -> Option<(bool, &'static Schema)> {
    if let Ok(ind) = object_schema
        .properties
        .binary_search_by_key(&name, |(n, _, _)| n)
    {
        let (_name, optional, schema) = object_schema.properties[ind];
        return Some((optional, schema));
    }
    None
}

fn render_string_input_panel(
    name: &'static str,
    title: String,
    mobile: bool,
) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |_| {
        let mut input = Field::new().name(name.to_string()).submit_empty(true);

        if let Some((optional, schema)) = lookup_schema(&name) {
            input.set_schema(schema);
            input.set_required(!optional);
        }
        if mobile {
            input.into()
        } else {
            InputPanel::new()
                .class(pwt::css::FlexFit)
                .with_field(title.clone(), input)
                .into()
        }
    })
}

pub fn qemu_onboot_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("onboot", tr!("Start on boot"), false, mobile).required(true)
}

pub fn qemu_tablet_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("tablet", tr!("Use tablet for pointer"), true, mobile).required(true)
}

pub fn qemu_acpi_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("acpi", tr!("ACPI support"), true, mobile).required(true)
}

pub fn qemu_kvm_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("kvm", tr!("KVM hardware virtualization"), true, mobile)
        .required(true)
}
pub fn qemu_freeze_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("freeze", tr!("Freeze CPU on startup"), false, mobile).required(true)
}

pub fn qemu_localtime_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("localtime", tr!("Use local time for RTC"), false, mobile)
        .required(true)
}

pub fn qemu_protection_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("protection", tr!("Protection"), false, mobile).required(true)
}

pub fn qemu_name_property(vmid: u32, mobile: bool) -> EditableProperty {
    let title = tr!("Name");
    EditableProperty::new("name", title.clone())
        .required(true)
        .placeholder(format!("VM {}", vmid))
        .render_input_panel(render_string_input_panel("name", title, mobile))
        .submit_hook(|state: PropertyEditorState| {
            let data = state.get_submit_data();
            Ok(delete_empty_values(&data, &["name"], false))
        })
}

pub fn qemu_ostype_property(mobile: bool) -> EditableProperty {
    let title = tr!("OS Type");
    EditableProperty::new("ostype", title.clone())
        .required(true)
        .placeholder("Other")
        .renderer(|_, v, _| match v.as_str() {
            Some(s) => format_qemu_ostype(s).into(),
            None => v.into(),
        })
        .render_input_panel(move |_| {
            let input = QemuOstypeSelector::new()
                .style("width", "100%")
                .name("ostype")
                .submit_empty(true);

            if mobile {
                input.into()
            } else {
                InputPanel::new()
                    .style("min-width", "500px")
                    .class(pwt::css::FlexFit)
                    .with_field(title.clone(), input)
                    .into()
            }
        })
        .submit_hook(|state: PropertyEditorState| {
            let data = state.get_submit_data();
            Ok(delete_empty_values(&data, &["ostype"], false))
        })
}

pub fn qemu_startup_property(mobile: bool) -> EditableProperty {
    EditableProperty::new("startup", tr!("Start/Shutdown order"))
        .required(true)
        .placeholder("order=any")
        .render_input_panel(move |_| {
            let order_field = Number::<u32>::new().name("_order").placeholder(tr!("any"));
            let order_label = tr!("Order");
            let up_label = tr!("Startup delay");
            let up_field = Number::<u32>::new().name("_up").placeholder(tr!("default"));
            let down_label = tr!("Shutdown timeout");
            let down_field = Number::<u32>::new()
                .name("_down")
                .placeholder(tr!("default"));

            if mobile {
                Column::new()
                    .gap(2)
                    .class(pwt::css::Flex::Fill)
                    .class(pwt::css::AlignItems::Stretch)
                    .with_child(label_field(order_label, order_field, true))
                    .with_child(label_field(up_label, up_field, true))
                    .with_child(label_field(down_label, down_field, true))
                    .into()
            } else {
                InputPanel::new()
                    .class(pwt::css::FlexFit)
                    .style("min-width", "500px")
                    .with_field(order_label, order_field)
                    .with_field(up_label, up_field)
                    .with_field(down_label, down_field)
                    .into()
            }
        })
        .load_hook(property_string_load_hook::<QemuConfigStartup>("startup"))
        .submit_hook(property_string_submit_hook::<QemuConfigStartup>(
            "startup", true,
        ))
}

pub fn qemu_boot_property(mobile: bool) -> EditableProperty {
    EditableProperty::new("boot", tr!("Boot Order"))
        .revert_keys(Rc::new(
            ["boot", "bootdisk"]
                .into_iter()
                .map(AttrValue::from)
                .collect(),
        ))
        .placeholder(format!(
            "{}, {}, {}",
            tr!("first Disk"),
            tr!("any CD-ROM"),
            tr!("any net")
        ))
        .render_input_panel(move |state: PropertyEditorState| {
            BootDeviceList::new(state.record.clone())
                .mobile(mobile)
                .name("boot")
                .submit_empty(true)
                .into()
        })
        .required(true)
        .submit_hook(|state: PropertyEditorState| {
            let data = state.get_submit_data();
            Ok(delete_empty_values(&data, &["boot"], false))
        })
}

pub fn qemu_hotplug_property() -> EditableProperty {
    EditableProperty::new("hotplug", tr!("Hotplug"))
        .placeholder(format_hotplug_feature(&Value::Null))
        .renderer(|_, v, _| format_hotplug_feature(v).into())
        .load_hook(|mut record: Value| {
            record["hotplug"] = crate::form::pve::normalize_hotplug_value(&record["hotplug"]);
            Ok(record)
        })
        .render_input_panel(move |_| {
            HotplugFeatureSelector::new()
                .name("hotplug")
                .submit_empty(true)
                .into()
        })
        .required(true)
        .submit_hook(|state: PropertyEditorState| {
            let data = state.get_submit_data();
            Ok(delete_empty_values(&data, &["hotplug"], false))
        })
}

pub fn qemu_startdate_property(mobile: bool) -> EditableProperty {
    thread_local! {
        static QEMU_STARTDATE_MATCH: Regex = Regex::new(r#"^(now|\d{4}-\d{1,2}-\d{1,2}(T\d{1,2}:\d{1,2}:\d{1,2})?)$"#).unwrap();
    }
    let title = tr!("RTC start date");
    EditableProperty::new("startdate", title.clone())
        .placeholder("now")
        // Note current schema definition does not include the regex, so we
        // need to add a validate function to the field.
        .render_input_panel(move |_| {
            let input = Field::new()
                .name("startdate")
                .placeholder("now")
                .submit_empty(true)
                .validate(|v: &String| {
                    if QEMU_STARTDATE_MATCH.with(|r| r.is_match(v)) {
                        return Ok(());
                    }
                    bail!(tr!("Format") + ": \"now\" or \"2006-06-17T16:01:21\" or \"2006-06-17\"")
                });
            if mobile {
                input.into()
            } else {
                InputPanel::new()
                    .class(pwt::css::FlexFit)
                    .with_field(title.clone(), input)
                    .into()
            }
        })
        .required(true)
        .submit_hook(|state: PropertyEditorState| {
            let data = state.get_submit_data();
            Ok(delete_empty_values(&data, &["startdate"], false))
        })
}

pub fn qemu_vmstatestorage_property(node: &str, mobile: bool) -> EditableProperty {
    let title = tr!("VM State storage");
    EditableProperty::new("vmstatestorage", title.clone())
        .required(true)
        .placeholder(tr!("Automatic"))
        .render_input_panel({
            let node = node.to_owned();
            move |_| {
                let selector = PveStorageSelector::new(node.clone())
                    .mobile(true)
                    .name("vmstatestorage")
                    .submit_empty(true)
                    .content_types(vec![StorageContent::Images])
                    .placeholder(tr!("Automatic (Storage used by the VM, or 'local')"));
                if mobile {
                    selector.into()
                } else {
                    InputPanel::new()
                        .style("min-width", "600px")
                        .class(pwt::css::FlexFit)
                        .with_field(title.clone(), selector)
                        .into()
                }
            }
        })
        .submit_hook(|state: PropertyEditorState| {
            let data = state.get_submit_data();
            Ok(delete_empty_values(&data, &["vmstatestorage"], false))
        })
}

pub fn qemu_vmstate_property() -> EditableProperty {
    EditableProperty::new("vmstate", tr!("Hibernation VM State"))
}
