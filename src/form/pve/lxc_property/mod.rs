mod lxc_cores_property;
pub use lxc_cores_property::lxc_cores_property;

mod lxc_nameserver_property;
pub use lxc_nameserver_property::{lxc_nameserver_property, lxc_searchdomain_property};

mod lxc_features_property;
pub use lxc_features_property::lxc_features_property;

mod lxc_mount_point_property;
pub use lxc_mount_point_property::{
    extract_used_mount_points, first_unused_mount_point, lxc_mount_point_property,
    lxc_rootfs_property, lxc_unused_volume_property,
};

mod lxc_memory_swap_property;
pub use lxc_memory_swap_property::{lxc_memory_property, lxc_swap_property};

mod lxc_network_property;
pub use lxc_network_property::lxc_network_property;

use proxmox_schema::{ApiType, ObjectSchema, Schema};
use pve_api_types::LxcConfig;

use pwt::prelude::*;
use pwt::widget::form::{Combobox, Field, Number};
use pwt::widget::{Column, InputPanel};
use serde_json::Value;

use crate::form::delete_empty_values;
use crate::utils::render_boolean;
use crate::SchemaValidation;
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

fn lookup_schema(name: &str) -> Option<(bool, &'static Schema)> {
    let allof_schema = LxcConfig::API_SCHEMA.unwrap_all_of_schema();

    for entry in allof_schema.list {
        if let Schema::Object(object_schema) = entry {
            if let Some((optional, schema)) = lookup_object_property_schema(object_schema, name) {
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

        if let Some((optional, schema)) = lookup_schema(name) {
            input.set_schema(schema);
            input.set_required(!optional);
        }
        if mobile {
            Column::new()
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_child(input)
                .into()
        } else {
            InputPanel::new()
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), input)
                .into()
        }
    })
}

pub fn lxc_console_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("console", "/dev/console", true, mobile).required(true)
}

pub fn lxc_unpriviledged_property() -> EditableProperty {
    EditableProperty::new("unprivileged", tr!("Unprivileged container"))
        .required(true)
        .renderer(move |_name, value, _data| match value {
            Value::Null => render_boolean(false).into(),
            Value::Bool(value) => render_boolean(*value).into(),
            _ => value.into(),
        })
}

pub fn lxc_ostype_property() -> EditableProperty {
    let title = tr!("OS Type");
    EditableProperty::new("ostype", title.clone())
        .required(true)
        .placeholder("Unknown")
}

pub fn lxc_architecture_property() -> EditableProperty {
    let title = tr!("Architecture");
    EditableProperty::new("arch", title.clone())
        .required(true)
        .placeholder("Unknown")
}

pub fn lxc_hookscript_property() -> EditableProperty {
    EditableProperty::new("hookscript", tr!("Hookscript"))
}

pub fn lxc_tty_count_property(mobile: bool) -> EditableProperty {
    let title = tr!("TTY count");
    EditableProperty::new("tty", title.clone())
        .required(true)
        .placeholder("2")
        .render_input_panel(move |_| {
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                //.style("min-width", (!mobile).then(|| "500px"))
                .with_field(
                    title.clone(),
                    Number::<u32>::new()
                        .submit_empty(true)
                        .placeholder("2")
                        .name("tty")
                        .min(0)
                        .max(6)
                        .default(2),
                )
                .into()
        })
        .submit_hook(|state: PropertyEditorState| {
            let data = state.get_submit_data();
            Ok(delete_empty_values(&data, &["tty"], false))
        })
}

pub fn lxc_console_mode_property(mobile: bool) -> EditableProperty {
    let title = tr!("Console mode");
    EditableProperty::new("cmode", title.clone())
        .required(true)
        .placeholder(tr!("Default") + " (tty)")
        .render_input_panel(move |_| {
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                //.style("min-width", (!mobile).then(|| "500px"))
                .with_field(
                    title.clone(),
                    Combobox::from_key_value_pairs([
                        ("tty", "/dev/tty[X]"),
                        ("console", "/dev/console"),
                        ("shell", "shell"),
                    ])
                    .placeholder("tty")
                    .name("cmode")
                    .submit_empty(true),
                )
                .into()
        })
        .submit_hook(|state: PropertyEditorState| {
            let data = state.get_submit_data();
            Ok(delete_empty_values(&data, &["cmode"], false))
        })
}

pub fn lxc_hostname_property(vmid: u32, mobile: bool) -> EditableProperty {
    let title = tr!("Hostname");
    EditableProperty::new("hostname", title.clone())
        .required(true)
        .placeholder(format!("CT{}", vmid))
        .render_input_panel(render_string_input_panel("hostname", title, mobile))
        .submit_hook(|state: PropertyEditorState| {
            let data = state.get_submit_data();
            Ok(delete_empty_values(&data, &["hostname"], false))
        })
}
