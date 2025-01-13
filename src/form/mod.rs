use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

use proxmox_schema::{ApiType, ObjectSchemaType, Schema};

#[inline]
fn format_property(name: &str, part: &str) -> String {
    format!("_{name}_{part}")
}

/// Convert a property string to separate properties
///
/// This is useful for use in an [`crate::EditWindow`] when editing parts of a property string.
/// Takes the `name` property from `data`, parses it a s property string, and sets it back to
/// `data` as `_{name}_{key}` so this should be used as a field. If it's not desired
/// to expose a property to the UI, simply add a hidden field to the form.
pub fn flatten_property_string(data: &mut Value, name: &str, schema: &'static Schema) {
    if let Some(prop_str) = data[name].as_str() {
        if let Ok(Value::Object(map)) = schema.parse_property_string(prop_str) {
            for (part, v) in map {
                data[format_property(name, &part)] = v;
            }
        }
    }
}

/// Uses an [`proxmox_schema::ObjectSchema`] to generate a property string from separate properties.
///
/// This is useful for use in an [`crate::EditWindow`] when editing parts of a property string.
/// Takes the single properties from `data` and adds it as a property string as `name`.
pub fn property_string_from_parts<T: ApiType + Serialize + DeserializeOwned>(
    data: &mut Value,
    name: &str,
    skip_empty_values: bool,
) {
    let props = match T::API_SCHEMA {
        Schema::Object(object_schema) => object_schema.properties(),
        _ => return, // not supported
    };

    if let Value::Object(map) = data {
        let mut value = json!({});

        let mut has_parts = false;
        for (part, _, _) in props {
            if let Some(v) = map.remove(&format_property(name, part)) {
                has_parts = true;
                let is_empty = match &v {
                    Value::String(s) => s.is_empty(),
                    _ => false,
                };
                if !(skip_empty_values && is_empty) {
                    value[part] = v;
                }
            }
        }

        if !has_parts {
            return;
        }

        let parsed: Option<T> = serde_json::from_value(value).ok();

        if let Some(parsed) = parsed {
            match proxmox_schema::property_string::print(&parsed) {
                Ok(prop_string) => data[name] = prop_string.into(),
                Err(err) => log::error!("error during property string print for {name}: {err}"),
            }
        } else {
            data[name] = "".into();
        }
    }
}
