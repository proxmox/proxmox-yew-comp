use anyhow::{bail, Error};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

use proxmox_client::ApiResponseData;
use proxmox_schema::{property_string::PropertyString, ApiType, ObjectSchemaType, Schema};

use yew::Callback;

use pwt::widget::form::FormContext;

pub mod pve;

use crate::{ApiLoadCallback, PropertyEditorState};

/// Delete default values fron submit data.
pub fn delete_default_values(record: &mut Value, defaults: &Value) {
    let defaults = match defaults.as_object() {
        Some(defaults) => defaults,
        None => return,
    };
    let record = match record.as_object_mut() {
        Some(record) => record,
        None => return,
    };

    record.retain(|k, v| defaults.get(k) != Some(v));
}

/// Delete empty values from the submit data.
///
/// And adds their names to the "delete" parameter.
///
/// By default only existing values are considered. if `delete_undefined` is
/// set, we also delete undefined values.
pub fn delete_empty_values(record: &Value, param_list: &[&str], delete_undefined: bool) -> Value {
    let mut new = json!({});
    let mut delete: Vec<String> = Vec::new();

    for (param, v) in record.as_object().unwrap().iter() {
        if !param_list.contains(&param.as_str()) {
            new[param] = v.clone();
            continue;
        }
        if v.is_null() || (v.is_string() && v.as_str().unwrap().is_empty()) {
            delete.push(param.to_string());
        } else {
            new[param] = v.clone();
        }
    }

    if delete_undefined {
        for param in param_list {
            if record.get(param).is_none() {
                delete.push(param.to_string());
            }
        }
    }

    if !delete.is_empty() {
        new["delete"] = delete.into();
    }

    new
}

/// Load data from API endpoint into a defined Rust type, then serialize it back into Value.
///
/// This is useful for Edit windows/dialogs, making sure returend values have correct type
/// (avoid bool as integer 0/1, or numbers as string).
pub fn typed_load<T: DeserializeOwned + Serialize>(
    url: impl Into<String>,
) -> ApiLoadCallback<Value> {
    let url = url.into();
    let url_cloned = url.clone();
    ApiLoadCallback::new(move || {
        let url = url.clone();
        async move {
            // use Rust type to correctly convert pve boolean 0, 1 values
            let resp: ApiResponseData<T> = crate::http_get_full(url, None).await?;

            Ok(ApiResponseData {
                data: serde_json::to_value(resp.data)?,
                attribs: resp.attribs,
            })
        }
    })
    .url(url_cloned)
}

/// Convert a property string to separate properties
///
/// This is useful for use in an [`crate::PropertyEditDialog`] when editing parts of a property string.
/// Takes the `name` property from `data`, parses it as property string, and sets it back to
/// `data` as `_{key}`, so that this can be used as a field. If it's not desired
/// to expose a property to the UI, simply add a hidden field to the form, or use
/// [property_string_add_missing_data] to re-add missing data before submit.
pub fn flatten_property_string<T: ApiType + Serialize + DeserializeOwned>(
    data: &mut Value,
    name: &str,
) -> Result<(), Error> {
    // Note: schema.parse_property_string does not work for schemas using KeyAliasInfo!!
    let test: Option<PropertyString<T>> = serde_json::from_value(data[name].clone())?;
    if let Some(test) = test {
        let record: Value = serde_json::to_value(test.into_inner())?;
        match record {
            Value::Object(map) => {
                for (part, v) in map {
                    data[format!("_{part}")] = v;
                }
            }
            _ => bail!("flatten_property_string {name:?} failed: result is not an Object"),
        }
    }

    Ok(())
}

/// Copy undefined object values from other object.
///
/// This is useful to assemble data inside form submit functions. Instead of adding Hidden fields to
/// include data, this function copies undefined values from the provided object (which
/// usually refers to the initial data loaded by the form).
pub fn property_string_add_missing_data<T: ApiType + Serialize + DeserializeOwned>(
    submit_data: &mut Value,
    original_data: &Value,
    form_ctx: &FormContext,
) -> Result<(), Error> {
    let props = match T::API_SCHEMA {
        Schema::Object(object_schema) => object_schema.properties(),
        _ => {
            bail!("property_string_add_missing_data: internal error - got unsupported schema type")
        }
    };

    let form_ctx = form_ctx.read();
    if let Value::Object(map) = submit_data {
        if let Value::Object(original_map) = original_data {
            for (part, _, _) in props {
                let part = format!("_{part}");
                if !map.contains_key(&part)
                    && !form_ctx.contains_field(&part)
                    && original_map.contains_key(&part)
                {
                    map.insert(part.clone(), original_data[&part].clone());
                }
            }
        }
    }
    Ok(())
}

/// Uses an [`proxmox_schema::ObjectSchema`] to generate a property string from separate properties.
///
/// This is useful for use in an [`crate::PropertyEditDialog`] when editing parts of a property string.
/// Takes the single properties from `data` and assembles a property string.
///
/// Property string data is removed from the original data, and re-added as assembled
/// property string with name `name`.
///
/// Returns the parsed rust type.
///
/// Uses "_{key}"" for property names like [flatten_property_string].
pub fn property_string_from_parts<T: ApiType + Serialize + DeserializeOwned>(
    data: &mut Value,
    name: &str,
    skip_empty_values: bool,
) -> Result<Option<T>, Error> {
    let props = match T::API_SCHEMA {
        Schema::Object(object_schema) => object_schema.properties(),
        _ => bail!("property_string_from_parts: internal error - got unsupported schema type"),
    };

    if let Value::Object(map) = data {
        let mut value = json!({});

        let mut has_parts = false;
        for (part, _, _) in props {
            if let Some(v) = map.remove(&format!("_{part}")) {
                has_parts = true;
                let is_empty = match &v {
                    Value::Null => true,
                    Value::String(s) => s.is_empty(),
                    _ => false,
                };
                if !(skip_empty_values && is_empty) {
                    value[part] = v;
                }
            }
        }

        if !has_parts {
            data[name] = "".into();
            return Ok(None);
        }

        let option: Option<T> = serde_json::from_value(value)?;
        data[name] = match option {
            Some(ref parsed) => proxmox_schema::property_string::print::<T>(parsed)?,
            None::<_> => String::new(),
        }
        .into();

        Ok(option)
    } else {
        bail!("property_string_from_parts: data is no Object");
    }
}

/// Standard load hook for property strings
///
/// Splits the property into separate values using [flatten_property_string].
pub fn property_string_load_hook<P: ApiType + Serialize + DeserializeOwned>(
    name: impl Into<String>,
) -> Callback<Value, Result<Value, Error>> {
    let name = name.into();
    Callback::from(move |mut record: Value| {
        flatten_property_string::<P>(&mut record, &name)?;
        Ok(record)
    })
}

/// Standard submit hook for property strings
///
/// Assembles the property from separate values using [property_string_from_parts].
pub fn property_string_submit_hook<P: ApiType + Serialize + DeserializeOwned>(
    name: impl Into<String>,
    delete_empty: bool,
) -> Callback<PropertyEditorState, Result<Value, Error>> {
    let name = name.into();
    Callback::from(move |state: PropertyEditorState| {
        let mut data = state.get_submit_data();
        // fixme: add defaults
        property_string_from_parts::<P>(&mut data, &name, true)?;
        if delete_empty {
            data = delete_empty_values(&data, &[&name], false);
        }
        Ok(data)
    })
}
