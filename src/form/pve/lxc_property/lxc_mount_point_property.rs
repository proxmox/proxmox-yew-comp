use std::collections::HashSet;
use std::rc::Rc;

use anyhow::bail;
use pve_api_types::{LxcConfigMp, LxcConfigMpArray, LxcConfigRootfs, StorageContent, StorageInfo};
use regex::Regex;

use pwt::prelude::*;
use pwt::widget::form::{Checkbox, Combobox, Field, Number, ValidateFn};
use pwt::widget::{form::FormContextObserver, InputPanel};
use pwt::widget::{Container, Row};
use serde_json::{json, Value};
use yew::virtual_dom::VComp;

use crate::form::pve::{parse_unused_key, LxcMountOptionsSelector, PveStorageSelector};
use crate::form::{
    delete_default_values, flatten_property_string, property_string_add_missing_data,
    property_string_from_parts,
};
use crate::{EditableProperty, PropertyEditorState};

const MOUNT_POINT_ID: &str = "_mount_point_id_";
const NOREPLICATE_FIELD_NAME: &str = "_noreplicate_";
const DISK_SIZE_FIELD_NAME: &str = "_disk_size_";
const IMAGE_STORAGE: &str = "_storage_";

const VOLUME_PN: &str = "_volume";
const MOUNT_PATH_PN: &str = "_mp";
const MOUNT_OPTIONS_PN: &str = "_mountoptions";

const SHARED_PN: &str = "_shared";
const READONLY_PN: &str = "_ro";
const SIZE_PN: &str = "_size";
const REPLICATE_PN: &str = "_replicate";
const BACKUP_PN: &str = "_backup";
const ACL_PN: &str = "_acl";
const QUOTA_PN: &str = "_quota";

#[derive(Properties, Clone, PartialEq)]
struct MountPointPanel {
    name: Option<String>,
    rootfs: bool,

    unused_disk: Option<String>,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,

    state: PropertyEditorState,

    unprivileged: bool,
    mobile: bool,
}

struct MountPointComp {
    storage_info: Option<StorageInfo>,
    _observer: FormContextObserver,

    is_create: bool,

    unused_volume: String,
    used_mount_points: HashSet<String>,

    validate_id: Option<ValidateFn<u16>>,
}

enum Msg {
    FormUpdate,
    StorageInfo(Option<StorageInfo>),
}

impl MountPointComp {
    fn update_state(&mut self, ctx: &Context<Self>) {
        let props = ctx.props();

        self.unused_volume = props
            .unused_disk
            .as_ref()
            .map(|unused_disk| {
                props.state.record[unused_disk]
                    .as_str()
                    .map(|s| s.to_string())
            })
            .flatten()
            .unwrap_or_default();

        self.used_mount_points = extract_used_mount_points(&props.state.record);

        self.validate_id = Some(ValidateFn::from({
            let used_mount_points = self.used_mount_points.clone();
            move |id: &u16| {
                if used_mount_points.contains(&format!("mp{id}")) {
                    bail!(tr!("Mount point is already in use."));
                }
                Ok(())
            }
        }))
    }
}

impl Component for MountPointComp {
    type Message = Msg;
    type Properties = MountPointPanel;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        let _observer = props
            .state
            .form_ctx
            .add_listener(ctx.link().callback(|_| Msg::FormUpdate));

        Self {
            _observer,
            storage_info: None,
            is_create: props.name.is_none(),
            unused_volume: String::new(),
            used_mount_points: HashSet::new(),
            validate_id: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::StorageInfo(info) => self.storage_info = info,
            Msg::FormUpdate => { /* redraw */ }
        }

        self.update_state(ctx);

        true
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        self.update_state(ctx);
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let mobile = props.mobile;
        let state = &props.state;
        let form_ctx = &state.form_ctx;

        let advanced = form_ctx.get_show_advanced();

        let storage_type = match &self.storage_info {
            Some(StorageInfo { ty, .. }) => ty.clone(),
            _ => String::new(),
        };

        let is_bindmount = if props.unused_disk.is_some() {
            false
        } else {
            match state.record.get(VOLUME_PN) {
                Some(Value::String(volume)) => {
                    if volume.starts_with("/dev/") {
                        false
                    } else if volume_storage(volume).is_some() {
                        false
                    } else {
                        true
                    }
                }
                _ => false,
            }
        };

        let enable_quota = !(storage_type == "zfs"
            || storage_type == "zfspool"
            || props.unprivileged
            || is_bindmount);

        let file_info_child = {
            let row = Row::new().key("filename_and_size").gap(1);

            if props.unused_disk.is_some() {
                row.with_child(Container::new().with_child(&self.unused_volume))
            } else {
                let file_text = match state.record.get(VOLUME_PN) {
                    Some(Value::String(file)) => file.clone(),
                    _ => String::new(),
                };
                let size_text = match state.record.get(SIZE_PN) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "-".into(),
                };

                row.with_child(Container::new().with_child(file_text))
                    .with_flex_spacer()
                    .with_child(Container::new().with_child(size_text))
            }
        };

        let mount_point_id_label = tr!("Mount Point ID");
        let mount_point_id_field = Number::<u16>::new()
            .name(MOUNT_POINT_ID)
            .submit(false)
            .required(true)
            .min(0)
            .max((LxcConfigMpArray::MAX - 1) as u16)
            .validate(self.validate_id.clone());

        let storage_label = tr!("Storage");
        let storage_field = PveStorageSelector::new(props.node.clone())
            .remote(props.remote.clone())
            .name(IMAGE_STORAGE)
            .submit(false)
            .required(true)
            .content_types(Some(vec![StorageContent::Rootdir]))
            .on_change(ctx.link().callback(Msg::StorageInfo))
            .mobile(mobile);

        let disk_size_label = tr!("Disk size") + " (GiB)";
        let disk_size_field = Number::<f64>::new()
            .name(DISK_SIZE_FIELD_NAME)
            .submit(false)
            .required(true)
            .min(0.001)
            .max(128.0 * 1024.0)
            .default(32.0);

        let mount_path_label = tr!("Path");
        let mount_path_field = Field::new()
            .name(MOUNT_PATH_PN)
            .required(true)
            .placeholder("/some/path");

        let backup_label = tr!("Backup");
        let backup_field = Checkbox::new()
            .switch(mobile)
            .name(BACKUP_PN)
            .disabled(is_bindmount || props.rootfs);

        let skip_replication_label = tr!("Skip replication");
        let skip_replication_field = Checkbox::new()
            .switch(mobile)
            .name(NOREPLICATE_FIELD_NAME)
            .submit(false);

        let readonly_label = tr!("Read-only");
        let readonly_field = Checkbox::new().switch(mobile).name(READONLY_PN);

        let quota_label = tr!("Enable quota");
        let quota_field = Checkbox::new()
            .switch(mobile)
            .name(QUOTA_PN)
            .disabled(!enable_quota);

        let acl_label = "ACLs";
        let acl_field = Combobox::from_key_value_pairs([
            ("", tr!("Default")),
            ("1", tr!("Enabled")),
            ("0", tr!("Disabled")),
        ])
        .name(ACL_PN);

        let mount_options_label = tr!("Mount options");
        let mount_options_field = LxcMountOptionsSelector::new().name(MOUNT_OPTIONS_PN);

        let mut panel = InputPanel::new()
            .class(pwt::css::FlexFit)
            .mobile(mobile)
            .show_advanced(advanced)
            .padding_x(2)
            .padding_bottom(1); // avoid scrollbar

        if mobile {
            if props.unused_disk.is_some() {
                panel.add_custom_child(file_info_child);
                panel.add_field(mount_point_id_label, mount_point_id_field);
            } else {
                if self.is_create {
                    panel.add_field(mount_point_id_label, mount_point_id_field);
                    panel.add_field(storage_label, storage_field);
                    panel.add_field(disk_size_label, disk_size_field);
                } else {
                    panel.add_custom_child(file_info_child);
                }
            }

            if !props.rootfs {
                panel.add_field(mount_path_label, mount_path_field);
                panel.add_single_line_field(false, false, backup_label, backup_field);
            }

            panel.add_spacer(true);
            panel.add_field_with_options(
                pwt::widget::FieldPosition::Left,
                true,
                false,
                acl_label,
                acl_field,
            );
            panel.add_field_with_options(
                pwt::widget::FieldPosition::Left,
                true,
                false,
                mount_options_label,
                mount_options_field,
            );
            panel.add_single_line_field(true, false, quota_label, quota_field);
            panel.add_single_line_field(
                true,
                false,
                skip_replication_label,
                skip_replication_field,
            );
            panel.add_single_line_field(true, false, readonly_label, readonly_field);
        } else {
            if props.unused_disk.is_some() {
                panel.add_field(mount_point_id_label, mount_point_id_field);
                panel.add_custom_child(file_info_child);
            } else {
                if self.is_create {
                    panel.add_field(mount_point_id_label, mount_point_id_field);
                    panel.add_field(storage_label, storage_field);
                    panel.add_field(disk_size_label, disk_size_field);
                } else {
                    panel.add_custom_child(file_info_child);
                }
            }

            if !props.rootfs {
                panel.add_field_with_options(
                    pwt::widget::FieldPosition::Right,
                    false,
                    false,
                    mount_path_label,
                    mount_path_field,
                );
                panel.add_field_with_options(
                    pwt::widget::FieldPosition::Right,
                    false,
                    false,
                    backup_label,
                    backup_field,
                );
            }

            panel.add_spacer(true);

            panel.add_field_with_options(
                pwt::widget::FieldPosition::Left,
                true,
                false,
                quota_label,
                quota_field,
            );
            panel.add_field_with_options(
                pwt::widget::FieldPosition::Right,
                true,
                false,
                acl_label,
                acl_field,
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
            panel.add_field_with_options(
                pwt::widget::FieldPosition::Large,
                true,
                false,
                mount_options_label,
                mount_options_field,
            );
        }
        panel.into()
    }
}

fn mount_point_property(
    name: Option<String>,
    title: String,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    unused_disk: Option<String>,
    rootfs: bool,
    unprivileged: bool,
    mobile: bool,
) -> EditableProperty {
    EditableProperty::new(name.clone(), title)
        .advanced_checkbox(true)
        .required(rootfs)
        .render_input_panel({
            let name = name.clone();
            let unused_disk = unused_disk.clone();
            move |state: PropertyEditorState| {
                let props = MountPointPanel {
                    name: name.clone(),
                    unused_disk: unused_disk.clone(),
                    node: node.clone(),
                    remote: remote.clone(),
                    rootfs,
                    state,
                    unprivileged,
                    mobile,
                };
                VComp::new::<MountPointComp>(Rc::new(props), None).into()
            }
        })
        .load_hook({
            let name = name.clone();
            let unused_disk = unused_disk.clone();

            move |mut record: Value| {
                let used_mount_points = extract_used_mount_points(&record);
                let default_mount_point = first_unused_mount_point(&used_mount_points);
                record[MOUNT_POINT_ID] = default_mount_point.into();

                if unused_disk.is_none() {
                    if let Some(name) = &name {
                        if rootfs {
                            flatten_property_string::<LxcConfigRootfs>(&mut record, name)?;
                        } else {
                            flatten_property_string::<LxcConfigMp>(&mut record, name)?;
                        }
                        if let Some(Value::Bool(replicate)) = record.get(REPLICATE_PN) {
                            record[NOREPLICATE_FIELD_NAME] = (!replicate).into();
                        }
                    } else {
                        record[BACKUP_PN] = true.into();
                    }
                }
                Ok(record)
            }
        })
        .submit_hook({
            move |state: PropertyEditorState| {
                let form_ctx = &state.form_ctx;
                let mut data = form_ctx.get_submit_data();
                let is_create = name.is_none();

                let prop_name = match (&name, &unused_disk) {
                    (Some(name), None) => name.clone(),
                    _ => format!("mp{}", form_ctx.read().get_field_text(MOUNT_POINT_ID)),
                };

                if let Some(unused_disk) = &unused_disk {
                    match state.record.get(unused_disk) {
                        Some(Value::String(unused_volume)) => {
                            data[VOLUME_PN] = unused_volume.clone().into();
                        }
                        _ => bail!("got invalid value for unused volume"),
                    }
                } else if is_create {
                    if data[VOLUME_PN].is_null() {
                        let image_storage = form_ctx.read().get_field_text(IMAGE_STORAGE);
                        let image_size =
                            match form_ctx.read().get_last_valid_value(DISK_SIZE_FIELD_NAME) {
                                Some(Value::Number(size)) => size.as_f64().unwrap(),
                                _ => bail!("got invalid disk size"),
                            };
                        let image = format!("{image_storage}:{image_size}");
                        data[VOLUME_PN] = image.into();
                    }
                }
                if let Some((_, _, Some(Value::Bool(no_replicate)))) =
                    form_ctx.read().get_field_data(NOREPLICATE_FIELD_NAME)
                {
                    data[REPLICATE_PN] = (!no_replicate).into();
                }

                let defaults = json!({
                    REPLICATE_PN: true,
                    ACL_PN: false,
                    QUOTA_PN: false,
                    READONLY_PN: false,
                    BACKUP_PN: false,
                    SHARED_PN: false,
                });

                property_string_add_missing_data::<LxcConfigMp>(
                    &mut data,
                    &state.record,
                    form_ctx,
                )?;
                delete_default_values(&mut data, &defaults);
                if rootfs {
                    property_string_from_parts::<LxcConfigRootfs>(&mut data, &prop_name, true)?;
                } else {
                    property_string_from_parts::<LxcConfigMp>(&mut data, &prop_name, true)?;
                }
                Ok(data)
            }
        })
}

pub fn lxc_mount_point_property(
    name: Option<String>,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    unprivileged: bool,
    mobile: bool,
) -> EditableProperty {
    let mut title = tr!("Mount Point");
    if let Some(name) = &name {
        title = title + " (" + name + ")";
    }

    mount_point_property(
        name.clone(),
        title,
        node.clone(),
        remote.clone(),
        None,
        false,
        unprivileged,
        mobile,
    )
}

pub fn lxc_unused_volume_property(
    name: String,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    unprivileged: bool,
    mobile: bool,
) -> EditableProperty {
    let mut title = tr!("Unused Volume");
    if let Some(id) = parse_unused_key(&name) {
        title = title + " " + &id.to_string();
    }

    mount_point_property(
        Some(name.clone()),
        title,
        node.clone(),
        remote.clone(),
        Some(name.clone()),
        false,
        unprivileged,
        mobile,
    )
}

pub fn lxc_rootfs_property(
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    unprivileged: bool,
    mobile: bool,
) -> EditableProperty {
    let title = tr!("Root Disk");
    mount_point_property(
        Some("rootfs".into()),
        title,
        node,
        remote,
        None,
        true,
        unprivileged,
        mobile,
    )
}

pub fn extract_used_mount_points(record: &Value) -> HashSet<String> {
    let mut list = HashSet::new();
    if let Some(map) = record.as_object() {
        for key in map.keys() {
            if key.starts_with("mp") && key[2..].parse::<u16>().is_ok() {
                list.insert(key.to_string());
            }
        }
    }
    list
}

pub fn first_unused_mount_point(used_prop_names: &HashSet<String>) -> Option<usize> {
    for n in 0..LxcConfigMpArray::MAX {
        let name = format!("mp{n}");
        if !used_prop_names.contains(&name) {
            return Some(n);
        }
    }
    None
}

fn volume_storage(volume: &str) -> Option<String> {
    thread_local! {
        static VOLUME_MATCH: Regex = Regex::new(r#"^([a-zA-Z][a-zA-Z0-9\-_.]*[a-zA-Z0-9]):"#).unwrap();
    }
    match VOLUME_MATCH.with(|r| r.captures(volume)) {
        Some(caps) => match caps.get(1) {
            Some(storage) => Some(storage.as_str().into()),
            None => None,
        },
        None => None,
    }
}
