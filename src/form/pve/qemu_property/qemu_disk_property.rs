use std::rc::Rc;

use std::collections::HashSet;

use anyhow::{bail, Error};
use proxmox_schema::property_string::PropertyString;
use serde_json::{json, Value};

use pwt::prelude::*;
use pwt::widget::form::{Checkbox, FormContextObserver, RadioButton};
use pwt::widget::{Container, InputPanel, Row};

use pve_api_types::{
    PveQmIde, QemuConfigSata, QemuConfigScsi, QemuConfigScsiArray, QemuConfigUnused,
    QemuConfigVirtio, StorageContent, StorageInfo, StorageInfoFormatsDefault,
};
use yew::virtual_dom::VComp;

const MEDIA_TYPE: &'static str = "_media_type_";
const BUS_DEVICE: &'static str = "_device_";
const IMAGE_STORAGE: &'static str = "_storage_";
const NOREPLICATE_FIELD_NAME: &'static str = "_noreplicate_";
const DISCARD_CHECKBOX_NAME: &'static str = "_discard_checkbox_";

const FILE_PN: &'static str = "_file";
const DISCARD_PN: &'static str = "_discard";
const READONLY_PN: &'static str = "_ro";
const REPLICATE_PN: &'static str = "_replicate";
const BACKUP_PN: &'static str = "_backup";
const IOTHREAD_PN: &'static str = "_iothread";
const SSD_PN: &'static str = "_ssd";

use crate::form::pve::pve_storage_content_selector::PveStorageContentSelector;
use crate::form::pve::{
    parse_qemu_controller_name, PveStorageSelector, QemuCacheTypeSelector, QemuControllerSelector,
    QemuDiskSizeFormatSelector,
};
use crate::form::{
    delete_default_values, delete_empty_values, flatten_property_string,
    property_string_add_missing_data, property_string_from_parts,
};
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

#[derive(Properties, Clone, PartialEq)]
struct DiskPanel {
    name: Option<String>,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,

    state: PropertyEditorState,
    mobile: bool,
}

struct DiskPanelComp {
    storage_info: Option<StorageInfo>,
    _observer: FormContextObserver,
}

enum DiskPanelMsg {
    FormUpdate,
    StorageInfo(Option<StorageInfo>),
}

impl Component for DiskPanelComp {
    type Message = DiskPanelMsg;
    type Properties = DiskPanel;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        let _observer = props
            .state
            .form_ctx
            .add_listener(ctx.link().callback(|_| DiskPanelMsg::FormUpdate));

        Self {
            storage_info: None,
            _observer,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            DiskPanelMsg::StorageInfo(info) => self.storage_info = info,
            DiskPanelMsg::FormUpdate => { /* redraw */ }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let mobile = props.mobile;
        let state = &props.state;
        let form_ctx = &state.form_ctx;
        let is_create = props.name.is_none();

        let used_devices = extract_used_devices(&state.record);
        let advanced = form_ctx.get_show_advanced();

        let (supported_formats, default_format, select_existing) = match &self.storage_info {
            Some(StorageInfo {
                formats: Some(formats),
                select_existing,
                ..
            }) => (
                formats.supported.clone(),
                formats.default,
                select_existing.unwrap_or(false),
            ),
            _ => (
                vec![StorageInfoFormatsDefault::Raw],
                StorageInfoFormatsDefault::Raw,
                false,
            ),
        };

        let bus_device_label = tr!("Bus/Device");
        let bus_device_field = QemuControllerSelector::new()
            .name(BUS_DEVICE)
            .submit(false)
            .exclude_devices(used_devices);

        let file_info_child = {
            let file_text = match state.record.get(FILE_PN) {
                Some(Value::String(file)) => file.clone(),
                _ => String::new(),
            };
            let size_text = match state.record.get("_size") {
                Some(Value::String(s)) => s.clone(),
                _ => "-".into(),
            };
            Row::new()
                .key("filename_and_size")
                .gap(1)
                .with_child(Container::new().with_child(file_text))
                .with_flex_spacer()
                .with_child(Container::new().with_child(size_text))
        };

        let cache_label = tr!("Cache");
        let cache_field = QemuCacheTypeSelector::new().name("_cache");

        let storage_label = tr!("Storage");
        let storage_field = PveStorageSelector::new(props.node.clone())
            .remote(props.remote.clone())
            .name(IMAGE_STORAGE)
            .submit(false)
            .required(true)
            .content_types(Some(vec![StorageContent::Images]))
            .on_change(ctx.link().callback(DiskPanelMsg::StorageInfo))
            .mobile(mobile);

        let disk_image_label = tr!("Disk image");
        let disk_image_field = PveStorageContentSelector::new()
            .mobile(mobile)
            .name(FILE_PN)
            .node(props.node.clone())
            .required(true)
            .storage(self.storage_info.as_ref().map(|info| info.storage.clone()));

        let disk_size_label = tr!("Disk size") + " (GiB)";
        let disk_size_field = QemuDiskSizeFormatSelector::new()
            .supported_formats(Some(supported_formats))
            .default_format(default_format);

        let discard_label = tr!("Discard");
        let discard_field = Checkbox::new()
            .switch(mobile)
            .name(DISCARD_CHECKBOX_NAME)
            .submit(false)
            .default(true);

        let io_thread_label = tr!("IO thread");
        let io_thread_field = Checkbox::new().switch(mobile).name(IOTHREAD_PN);

        let ssd_emulation_label = tr!("SSD emulation");
        let ssd_emulation_field = Checkbox::new().switch(mobile).name(SSD_PN);

        let backup_label = tr!("Backup");
        let backup_field = Checkbox::new().switch(mobile).name(BACKUP_PN).default(true);

        let skip_replication_label = tr!("Skip replication");
        let skip_replication_field = Checkbox::new()
            .switch(mobile)
            .name(NOREPLICATE_FIELD_NAME)
            .submit(false);

        let readonly_label = tr!("Read-only");
        let readonly_field = Checkbox::new().switch(mobile).name(READONLY_PN);

        let mut panel = InputPanel::new()
            .show_advanced(advanced)
            .mobile(props.mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2);

        if mobile {
            if is_create {
                panel.add_field(bus_device_label, bus_device_field);
            } else {
                panel.add_custom_child(file_info_child);
            }

            panel.add_field(cache_label, cache_field);

            if is_create {
                panel.add_field(storage_label, storage_field);
                if select_existing {
                    panel.add_field(disk_image_label, disk_image_field);
                } else {
                    panel.add_field(disk_size_label, disk_size_field);
                }
            }

            panel.add_single_line_field(false, false, discard_label, discard_field);
            panel.add_single_line_field(false, false, io_thread_label, io_thread_field);

            panel.add_spacer(true);
            panel.add_single_line_field(true, false, ssd_emulation_label, ssd_emulation_field);
            panel.add_single_line_field(true, false, backup_label, backup_field);
            panel.add_single_line_field(
                true,
                false,
                skip_replication_label,
                skip_replication_field,
            );
            panel.add_single_line_field(true, false, readonly_label, readonly_field);
        } else {
            panel.set_field_width("minmax(250px, 1fr)");
            if is_create {
                panel.add_field(bus_device_label, bus_device_field);
            } else {
                panel.add_custom_child(file_info_child);
            }

            panel.add_right_field(cache_label, cache_field);

            if is_create {
                panel.add_field(storage_label, storage_field);
                if select_existing {
                    panel.add_field(disk_image_label, disk_image_field);
                } else {
                    panel.add_field(disk_size_label, disk_size_field);
                }
            }

            panel.add_right_field(discard_label, discard_field);
            panel.add_right_field(io_thread_label, io_thread_field);

            panel.add_spacer(true);
            panel.add_field_with_options(
                pwt::widget::FieldPosition::Left,
                true,
                false,
                ssd_emulation_label,
                ssd_emulation_field,
            );
            panel.add_field_with_options(
                pwt::widget::FieldPosition::Right,
                true,
                false,
                backup_label,
                backup_field,
            );
            panel.add_field_with_options(
                pwt::widget::FieldPosition::Left,
                true,
                false,
                readonly_label,
                readonly_field,
            );
            panel.add_field_with_options(
                pwt::widget::FieldPosition::Right,
                true,
                false,
                skip_replication_label,
                skip_replication_field,
            );
        }
        panel.into()
    }
}

pub fn qemu_disk_property(
    name: Option<String>,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    mobile: bool,
) -> EditableProperty {
    let mut title = tr!("Hard Disk");
    if let Some(name) = name.as_deref() {
        title = title + " (" + name + ")";
    }

    EditableProperty::new(name.clone(), title)
        .advanced_checkbox(true)
        .render_input_panel({
            let name = name.clone();
            move |state: PropertyEditorState| {
                let props = DiskPanel {
                    name: name.clone(),
                    node: node.clone(),
                    remote: remote.clone(),
                    state,
                    mobile,
                };
                VComp::new::<DiskPanelComp>(Rc::new(props), None).into()
            }
        })
        .load_hook({
            let name = name.clone();

            move |mut record: Value| {
                if let Some(name) = &name {
                    flatten_device_data(&mut record, name)?;
                    record[BUS_DEVICE] = name.clone().into();
                } else {
                    let used_devices = extract_used_devices(&record);
                    let default_device = first_unused_scsi_device(&used_devices);
                    record[BUS_DEVICE] = default_device.clone().into();
                }

                Ok(record)
            }
        })
        .submit_hook({
            let name = name.clone();

            move |state: PropertyEditorState| {
                let form_ctx = &state.form_ctx;
                let mut data = form_ctx.get_submit_data();
                let is_create = name.is_none();

                let device = match &name {
                    Some(name) => name.clone(),
                    None::<_> => form_ctx.read().get_field_text(BUS_DEVICE),
                };

                if is_create {
                    if data[FILE_PN].is_null() {
                        let image_storage = form_ctx.read().get_field_text(IMAGE_STORAGE);
                        let image_size = match form_ctx
                            .read()
                            .get_last_valid_value(QemuDiskSizeFormatSelector::DISK_SIZE)
                        {
                            Some(Value::Number(size)) => size.as_f64().unwrap(),
                            _ => bail!("got invalid disk size"),
                        };
                        let image = format!("{image_storage}:{image_size}");
                        data[FILE_PN] = image.into();

                        let image_format = form_ctx
                            .read()
                            .get_field_text(QemuDiskSizeFormatSelector::DISK_FORMAT);

                        if !image_format.is_empty() {
                            data["_format"] = Value::String(image_format);
                        }
                    }
                }

                let data = assemble_device_data(&state, &mut data, &device)?;
                Ok(data)
            }
        })
}

fn add_unused_disk_panel(name: String, mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let used_devices = extract_used_devices(&state.record);

        let disk_image = state
            .record
            .get(&name)
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        InputPanel::new()
            .mobile(mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .with_custom_child(Container::new().with_child(disk_image))
            .with_field(
                tr!("Bus/Device"),
                QemuControllerSelector::new()
                    .name(BUS_DEVICE)
                    .submit(false)
                    .exclude_devices(used_devices),
            )
            .into()
    })
}

pub fn qemu_unused_disk_property(name: &str, mobile: bool) -> EditableProperty {
    let title = tr!("Unused Disk");

    EditableProperty::new(name.to_string(), title)
        .render_input_panel(add_unused_disk_panel(name.to_string(), mobile))
        .load_hook({
            // let name = name.to_string();
            move |mut record: Value| {
                let used_devices = extract_used_devices(&record);
                let default_device = first_unused_scsi_device(&used_devices);
                record[BUS_DEVICE] = default_device.clone().into();
                Ok(record)
            }
        })
        .submit_hook({
            let name = name.to_string();

            move |state: PropertyEditorState| {
                let form_ctx = &state.form_ctx;
                let mut data = form_ctx.get_submit_data();

                let device = form_ctx.read().get_field_text(BUS_DEVICE);
                let unused: PropertyString<QemuConfigUnused> =
                    serde_json::from_value(state.record[&name].clone())?;

                data[FILE_PN] = unused.file.clone().into();

                let data = assemble_device_data(&state, &mut data, &device)?;
                Ok(data)
            }
        })
}

pub fn extract_used_devices(record: &Value) -> HashSet<String> {
    let mut list = HashSet::new();
    if let Some(map) = record.as_object() {
        for key in map.keys() {
            if let Ok(_) = parse_qemu_controller_name(key) {
                list.insert(key.to_string());
            }
        }
    }
    list
}

fn first_unused_scsi_device(used_devices: &HashSet<String>) -> Option<String> {
    for n in 0..QemuConfigScsiArray::MAX {
        let name = format!("scsi{n}");
        if !used_devices.contains(&name) {
            return Some(name);
        }
    }
    None
}

fn cdrom_input_panel(
    name: Option<String>,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    mobile: bool,
) -> RenderPropertyInputPanelFn {
    let is_create = name.is_none();
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = state.form_ctx;
        let media_type = form_ctx.read().get_field_text(MEDIA_TYPE);
        let image_storage = form_ctx.read().get_field_text(IMAGE_STORAGE);

        let used_devices = extract_used_devices(&state.record);

        let mut panel = InputPanel::new()
            .mobile(mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2);

        if is_create {
            panel.add_field(
                tr!("Bus/Device"),
                QemuControllerSelector::new()
                    .name(BUS_DEVICE)
                    .submit(false)
                    .exclude_devices(used_devices),
            );
        }

        panel
            .with_custom_child(
                RadioButton::new("iso")
                    .default(true)
                    .box_label(tr!("Use CD/DVD disc image file (iso)"))
                    .name(MEDIA_TYPE)
                    .key("media-type-iso")
                    .submit(false),
            )
            .with_field(
                tr!("Storage"),
                PveStorageSelector::new(node.clone())
                    .mobile(mobile)
                    .disabled(media_type != "iso")
                    .remote(remote.clone())
                    .name(IMAGE_STORAGE)
                    .content_types(Some(vec![StorageContent::Iso]))
                    .submit(false)
                    .required(true)
                    .autoselect(true),
            )
            .with_field(
                tr!("ISO image"),
                PveStorageContentSelector::new()
                    .mobile(mobile)
                    .disabled(media_type != "iso")
                    .name(FILE_PN)
                    .required(true)
                    .node(node.clone())
                    .storage(image_storage.clone())
                    .content_filter(StorageContent::Iso),
            )
            .with_custom_child(
                RadioButton::new("cdrom")
                    .box_label(tr!("Use physical CD/DVD Drive"))
                    .name(MEDIA_TYPE)
                    .key("media-type-cdrom")
                    .submit(false),
            )
            .with_custom_child(
                RadioButton::new("none")
                    .box_label(tr!("Do not use any media"))
                    .name(MEDIA_TYPE)
                    .key("media-type-none")
                    .submit(false),
            )
            .into()
    })
}

pub fn qemu_cdrom_property(
    name: Option<String>,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    mobile: bool,
) -> EditableProperty {
    let mut title = tr!("CD/DVD Drive");
    if let Some(name) = name.as_deref() {
        title = title + " (" + name + ")";
    }
    EditableProperty::new(name.clone(), title)
        .render_input_panel(cdrom_input_panel(
            name.clone(),
            node.clone(),
            remote.clone(),
            mobile,
        ))
        .load_hook({
            let name = name.clone();

            move |mut record: Value| {
                if let Some(name) = &name {
                    flatten_device_data(&mut record, name)?;
                    record[BUS_DEVICE] = name.clone().into();
                } else {
                    let used_devices = extract_used_devices(&record);
                    if !used_devices.contains("ide2") {
                        record[BUS_DEVICE] = "ide2".into();
                    }
                }

                match record["_file"].as_str() {
                    Some("cdrom") => {
                        record[MEDIA_TYPE] = "cdrom".into();
                        record[FILE_PN] = Value::Null;
                    }
                    Some("none") => {
                        record[MEDIA_TYPE] = "none".into();
                        record[FILE_PN] = Value::Null;
                    }
                    Some(volid) => {
                        if let Some((storage, _rest)) = volid.split_once(':') {
                            record[IMAGE_STORAGE] = storage.into();
                        }
                    }
                    _ => {}
                }

                Ok(record)
            }
        })
        .submit_hook({
            let name = name.clone();

            move |state: PropertyEditorState| {
                let form_ctx = &state.form_ctx;
                let mut data = form_ctx.get_submit_data();

                let device = match &name {
                    Some(name) => name.clone(),
                    None::<_> => form_ctx.read().get_field_text(BUS_DEVICE),
                };

                let media_type = form_ctx.read().get_field_text(MEDIA_TYPE);

                match media_type.as_str() {
                    "cdrom" => data[FILE_PN] = "cdrom".into(),
                    "none" => data[FILE_PN] = "none".into(),
                    _ => {}
                };

                data["_media"] = "cdrom".into();

                let data = assemble_device_data(&state, &mut data, &device)?;

                Ok(data)
            }
        })
        .on_change(|state: PropertyEditorState| {
            let form_ctx = state.form_ctx;
            let image_storage = form_ctx.read().get_field_text(IMAGE_STORAGE);
            let file = form_ctx.read().get_field_text(FILE_PN);
            if !image_storage.is_empty() {
                if !file.starts_with(&(image_storage + ":")) {
                    form_ctx.write().set_field_value(FILE_PN, "".into());
                }
            }
        })
}

fn flatten_device_data(record: &mut Value, name: &str) -> Result<(), Error> {
    if name.starts_with("ide") {
        flatten_property_string::<PveQmIde>(record, name)?;
    } else if name.starts_with("sata") {
        flatten_property_string::<QemuConfigSata>(record, name)?;
    } else if name.starts_with("scsi") {
        flatten_property_string::<QemuConfigScsi>(record, name)?;
    } else if name.starts_with("virtio") {
        flatten_property_string::<QemuConfigVirtio>(record, name)?;
    } else {
        bail!("flatten_device_data: unsupported device type '{name}'");
    }

    if let Some(Value::String(discard)) = record.get(DISCARD_PN) {
        record[DISCARD_CHECKBOX_NAME] = match discard.as_str() {
            "on" => true,
            "ignore" => false,
            _ => {
                bail!("got unknown value for discard property: {discard}");
            }
        }
        .into();
    } else {
        record[DISCARD_CHECKBOX_NAME] = false.into();
    }

    if let Some(Value::Bool(replicate)) = record.get(REPLICATE_PN) {
        record[NOREPLICATE_FIELD_NAME] = (!replicate).into();
    }

    Ok(())
}

fn assemble_device_data(
    state: &PropertyEditorState,
    data: &mut Value,
    device: &str,
) -> Result<Value, Error> {
    let form_ctx = &state.form_ctx;

    if let Some((_, _, Some(Value::Bool(no_replicate)))) =
        form_ctx.read().get_field_data(NOREPLICATE_FIELD_NAME)
    {
        data[REPLICATE_PN] = (!no_replicate).into();
    }
    if let Some((_, _, Some(Value::Bool(discard)))) =
        form_ctx.read().get_field_data(DISCARD_CHECKBOX_NAME)
    {
        data[DISCARD_PN] = if discard { "on" } else { "ignore" }.into();
    }

    let defaults = json!({
        DISCARD_PN: "ignore",
        REPLICATE_PN: true,
        READONLY_PN: false,
        BACKUP_PN: true,
        IOTHREAD_PN: false,
        SSD_PN: false,
    });

    if device.starts_with("ide") {
        property_string_add_missing_data::<PveQmIde>(data, &state.record, form_ctx)?;
        delete_default_values(data, &defaults);
        property_string_from_parts::<PveQmIde>(data, device, true)?;
    } else if device.starts_with("sata") {
        property_string_add_missing_data::<QemuConfigSata>(data, &state.record, form_ctx)?;
        delete_default_values(data, &defaults);
        property_string_from_parts::<QemuConfigSata>(data, device, true)?;
    } else if device.starts_with("scsi") {
        property_string_add_missing_data::<QemuConfigScsi>(data, &state.record, form_ctx)?;
        delete_default_values(data, &defaults);
        property_string_from_parts::<QemuConfigScsi>(data, device, true)?;
    } else if device.starts_with("virtio") {
        property_string_add_missing_data::<QemuConfigVirtio>(data, &state.record, form_ctx)?;
        delete_default_values(data, &defaults);
        property_string_from_parts::<QemuConfigVirtio>(data, device, true)?;
    } else {
        bail!("assemble_device_data: unsupported device type '{device}'");
    }
    let data = delete_empty_values(data, &[device], false);
    Ok(data)
}
