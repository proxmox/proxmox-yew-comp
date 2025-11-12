use std::rc::Rc;

use proxmox_human_byte::HumanByte;
use proxmox_schema::property_string::PropertyString;
use serde_json::Value;

use pve_api_types::QemuConfigMemory;

use pwt::prelude::*;
use pwt::widget::form::{Checkbox, FormContext, Hidden, Number};
use pwt::widget::{FieldPosition, InputPanel};

use crate::form::{delete_empty_values, flatten_property_string, property_string_from_parts};
use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

fn read_u64(form_ctx: &FormContext, name: &str) -> Option<u64> {
    let value = form_ctx.read().get_last_valid_value(name.to_string());
    match value {
        Some(Value::Number(n)) => n.as_u64(),
        _ => None,
    }
}

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = state.form_ctx;
        let advanced = form_ctx.get_show_advanced();

        let current_memory = read_u64(&form_ctx, "_current");

        let use_ballooning = form_ctx.read().get_field_checked("_use_ballooning");

        let disable_shares = {
            let balloon = read_u64(&form_ctx, "balloon");
            match (current_memory, balloon) {
                (Some(memory), Some(balloon)) => memory == balloon,
                _ => false,
            }
        };

        let memory_default = 512u64;

        let current_label = tr!("Memory") + " (MiB)";
        let current_field = Number::<u64>::new()
            .name("_current")
            .placeholder(memory_default.to_string())
            .min(16)
            .step(32);

        let balloon_label = tr!("Minimum memory") + " (MiB)";
        let balloon_field = Number::<u64>::new()
            .name("balloon")
            .disabled(!use_ballooning)
            .submit_empty(true)
            .min(1)
            .max(current_memory)
            .step(32)
            .placeholder(current_memory.map(|n| n.to_string()));

        let shares_enable = use_ballooning && !disable_shares;
        let shares_label = tr!("Shares");
        let shares_field = Number::<u64>::new()
            .name("shares")
            .disabled(!shares_enable)
            .submit_empty(true)
            .placeholder(tr!("Default") + " (1000)")
            .max(50000)
            .step(10);

        let use_balloon_label = tr!("Ballooning Device");
        let use_balloon_field = Checkbox::new()
            .name("_use_ballooning")
            .switch(mobile)
            .submit(false);

        InputPanel::new()
            .class(pwt::css::FlexFit)
            .mobile(mobile)
            .label_width("max-content")
            .show_advanced(advanced)
            .padding_x(2)
            .with_field(current_label, current_field)
            .with_custom_child_and_options(
                FieldPosition::Left,
                false,
                true,
                Hidden::new()
                    .key("old_memory_cache")
                    .name("_old_memory")
                    .submit(false),
            )
            .with_advanced_spacer()
            .with_advanced_field(balloon_label, balloon_field)
            .with_advanced_field(shares_label, shares_field)
            .with_single_line_field(true, false, use_balloon_label, use_balloon_field)
            .into()
    })
}

fn render_value(_name: &str, v: &Value, record: &Value) -> Html {
    let current =
        match serde_json::from_value::<Option<PropertyString<QemuConfigMemory>>>(v.clone()) {
            Ok(None) => 512,
            Ok(Some(parsed)) => parsed.current,
            Err(err) => {
                log::error!("qemu_memory_property renderer: {err}");
                return v.into();
            }
        };

    let balloon = record["balloon"].as_u64();

    let current_hb = HumanByte::new_binary((current * 1024 * 1024) as f64);

    let mut text = match balloon {
        Some(0) => format!("{current_hb} [balloon=0]"),
        Some(balloon) => {
            let balloon_hb = HumanByte::new_binary((balloon * 1024 * 1024) as f64);
            if current > balloon {
                format!("{balloon_hb}/{current_hb}")
            } else {
                current_hb.to_string()
            }
        }
        None => current_hb.to_string(),
    };

    if let Some(shares) = record["shares"].as_u64() {
        text += &format!(" [shares={shares}]");
    }

    text.into()
}

pub fn qemu_memory_property(mobile: bool) -> EditableProperty {
    EditableProperty::new("memory", tr!("Memory"))
        .advanced_checkbox(true)
        .required(true)
        .revert_keys(Rc::new(
            ["memory", "balloon", "shares"]
                .into_iter()
                .map(AttrValue::from)
                .collect(),
        ))
        .render_input_panel(input_panel(mobile))
        .renderer(render_value)
        .submit_hook(|state: PropertyEditorState| {
            let form_ctx = state.form_ctx;
            let mut data = form_ctx.get_submit_data();

            if !form_ctx.read().get_field_checked("_use_ballooning") {
                data["balloon"] = 0.into(); // value 0 disables ballooning
                data["shares"] = Value::Null; // delete shares
            }

            property_string_from_parts::<QemuConfigMemory>(&mut data, "memory", true)?;
            data = delete_empty_values(&data, &["memory", "balloon", "shares"], false);
            Ok(data)
        })
        .load_hook(|mut record| {
            flatten_property_string::<QemuConfigMemory>(&mut record, "memory")?;

            let use_ballooning = record["balloon"].as_u64() != Some(0);
            record["_use_ballooning"] = use_ballooning.into();

            if let Some(current_memory) = record["_current"].as_u64() {
                match record["balloon"].as_u64() {
                    Some(0) => record["balloon"] = Value::Null,
                    Some(_) => { /* keep value */ }
                    None => record["balloon"] = current_memory.into(),
                }
                record["_old_memory"] = current_memory.into();
            }

            Ok(record)
        })
        .on_change(|state: PropertyEditorState| {
            let form_ctx = state.form_ctx;
            let current_memory = form_ctx.read().get_last_valid_value("_current");
            let old_memory = form_ctx.read().get_field_value("_old_memory");
            let balloon = form_ctx.read().get_last_valid_value("balloon");

            match (&old_memory, &current_memory, &balloon) {
                (Some(old_memory), Some(current_memory), Some(balloon)) => {
                    if balloon == old_memory && old_memory != current_memory {
                        form_ctx
                            .write()
                            .set_field_value("balloon", current_memory.clone().into());
                    }
                }
                _ => {}
            }

            if let Some(current_memory) = current_memory {
                form_ctx
                    .write()
                    .set_field_value("_old_memory", current_memory.into());
            }
        })
}
