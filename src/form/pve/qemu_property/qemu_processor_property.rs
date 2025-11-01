use std::rc::Rc;

use serde_json::Value;

use pwt::prelude::*;
use pwt::props::PwtSpace;
use pwt::widget::form::{Checkbox, DisplayField, Field, Number};
use pwt::widget::{Column, Container, InputPanel, Row};

use crate::form::pve::{QemuCpuFlags, QemuCpuModelSelector};
use crate::form::{
    delete_empty_values, flatten_property_string, property_string_add_missing_data,
    property_string_from_parts,
};
use crate::layout::mobile_form::label_field;
use crate::PropertyEditorState;
use crate::{EditableProperty, RenderPropertyInputPanelFn};

use pve_api_types::PveVmCpuConf;

fn renderer(_name: &str, _value: &Value, record: &Value) -> Html {
    let cpu = record["cpu"].as_str().unwrap_or("kvm64");
    let cores = record["cores"].as_u64().unwrap_or(1);
    let sockets = record["sockets"].as_u64().unwrap_or(1);
    let count = sockets * cores;

    let mut text = format!(
        "{count} ({}, {}) [{cpu}]",
        tr!("1 Core" | "{n} Cores" % cores),
        tr!("1 Socket" | "{n} Sockets" % sockets)
    );

    if let Value::Bool(true) = record["numa"] {
        text += " [numa]";
    }

    if let Some(n) = &record["vcpus"].as_u64() {
        text += &format!(" [vcpus={n}]");
    }
    if let Some(n) = &record["cpulimit"].as_f64() {
        text += &format!(" [cpulimit={n}]");
    }
    if let Some(n) = &record["cpuunits"].as_u64() {
        text += &format!(" [cpuunits={n}]");
    }
    if let Some(s) = &record["affinity"].as_str() {
        text += &format!(" [affinity={s}]");
    }

    text.into()
}

fn socket_cores_input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = state.form_ctx;
        let total_cores;
        {
            let guard = form_ctx.read();
            let cores = guard
                .get_last_valid_value("cores")
                .unwrap_or(Value::Number(1.into()))
                .as_u64()
                .unwrap_or(1);
            let sockets = guard
                .get_last_valid_value("sockets")
                .unwrap_or(Value::Number(1.into()))
                .as_u64()
                .unwrap_or(1);
            total_cores = sockets * cores;
        }

        let cpu_type_label = tr!("Type");
        let cpu_type_field = QemuCpuModelSelector::new().name("_cputype").mobile(mobile);

        let sockets_label = tr!("Sockets");
        let sockets_field = Number::<u64>::new().name("sockets").min(1);

        let cores_label = tr!("Cores");
        let cores_field = Number::<u64>::new().name("cores").min(1);

        let total_label = tr!("Total cores");

        let panel = InputPanel::new()
            .mobile(mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .padding_bottom(1); // avoid scrollbar

        if mobile {
            panel
                .with_field(cpu_type_label, cpu_type_field)
                .with_field(sockets_label, sockets_field)
                .with_field(cores_label, cores_field)
                .with_custom_child(
                    Row::new()
                        .padding_top(1)
                        .gap(PwtSpace::Em(0.5))
                        .with_child(total_label + ":")
                        .with_child(Container::new().with_child(total_cores.to_string())),
                )
                .into()
        } else {
            panel
                .with_field(sockets_label, sockets_field)
                .with_right_field(cpu_type_label, cpu_type_field)
                .with_field(cores_label, cores_field)
                .with_right_field(
                    total_label,
                    DisplayField::new().value(total_cores.to_string()),
                )
                .into()
        }
    })
}

// Note: For the desktop view, we want everything in one edit wondow!
fn processor_input_panel() -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = &state.form_ctx;
        let advanced = form_ctx.get_show_advanced();

        let main_view = socket_cores_input_panel(false).apply(state.clone());
        let flags_view = cpu_flags_input_panel(false).apply(state.clone());
        let scheduler_view = kernel_scheduler_input_panel(false).apply(state.clone());

        Column::new()
            .class(pwt::css::FlexFit)
            .class(pwt::css::AlignItems::Stretch)
            .with_child(Row::new().padding_y(2).with_child(main_view))
            .with_optional_child(advanced.then(|| html! {<hr/>}))
            .with_child(
                Column::new()
                    .class(pwt::css::FlexFit)
                    .class((!advanced).then(|| pwt::css::Display::None))
                    .with_child(Row::new().padding_y(2).with_child(scheduler_view))
                    .with_child(
                        Container::new()
                            .padding_top(2)
                            .padding_x(2)
                            .style("padding-bottom", "0.25em")
                            .border_bottom(true)
                            .with_child(tr!("Extra CPU Flags") + ":"),
                    )
                    .with_child(flags_view)
                    .max_height(500),
            )
            .into()
    })
}

pub fn qemu_sockets_cores_property(mobile: bool) -> EditableProperty {
    const KEYS: &[&'static str] = &[
        "sockets", "cores", "cpu", "vcpus", "cpuunits", "cpulimit", "affinity", "numa",
    ];

    EditableProperty::new(
        "sockets",
        format!(
            "{}, {}, {}",
            tr!("Processor"),
            tr!("Sockets"),
            tr! {"Cores"}
        ),
    )
    .required(true)
    .advanced_checkbox(!mobile)
    .revert_keys(Rc::new(KEYS.iter().map(|s| AttrValue::from(*s)).collect()))
    .renderer(renderer)
    .render_input_panel(if mobile {
        socket_cores_input_panel(mobile)
    } else {
        processor_input_panel()
    })
    .load_hook(move |mut record: Value| {
        flatten_property_string::<PveVmCpuConf>(&mut record, "cpu")?;
        Ok(record)
    })
    .submit_hook(|state: PropertyEditorState| {
        let mut record = state.get_submit_data();
        property_string_add_missing_data::<PveVmCpuConf>(
            &mut record,
            &state.record,
            &state.form_ctx,
        )?;
        if record["_cputype"] == Value::Null {
            // PVE API actually requires this value to be set!
            record["_cputype"] = "kvm64".into();
        }
        property_string_from_parts::<PveVmCpuConf>(&mut record, "cpu", true)?;
        let record = delete_empty_values(&record, KEYS, false);
        Ok(record)
    })
}

fn cpu_flags_input_panel(_mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |_| QemuCpuFlags::new().name("_flags").into())
}

pub fn qemu_cpu_flags_property(mobile: bool) -> EditableProperty {
    EditableProperty::new("cpu", tr!("CPU flags"))
        .required(true)
        .renderer(renderer)
        .render_input_panel(cpu_flags_input_panel(mobile))
        .load_hook(move |mut record: Value| {
            flatten_property_string::<PveVmCpuConf>(&mut record, "cpu")?;
            Ok(record)
        })
        .submit_hook(|state: PropertyEditorState| {
            let mut record = state.get_submit_data();
            property_string_add_missing_data::<PveVmCpuConf>(
                &mut record,
                &state.record,
                &state.form_ctx,
            )?;
            property_string_from_parts::<PveVmCpuConf>(&mut record, "cpu", true)?;
            let record = delete_empty_values(&record, &["cpu"], false);
            Ok(record)
        })
}

fn kernel_scheduler_input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let record = state.record;
        let cores = record["cores"].as_u64().unwrap_or(1);
        let sockets = record["sockets"].as_u64().unwrap_or(1);
        let total_cores = cores * sockets;

        let vcpus_label = tr!("VCPUs");
        let vcpus_field = Number::<u64>::new()
            .name("vcpus")
            .min(1)
            .max(total_cores)
            .placeholder(total_cores.to_string())
            .submit_empty(true);

        let units_label = tr!("CPU units");
        let units_field = Number::<u64>::new()
            .name("cpuunits")
            .min(1)
            .max(10000)
            .placeholder("100")
            .submit_empty(true);

        let limit_label = tr!("CPU limit");
        let limit_field = Number::<f64>::new()
            .name("cpulimit")
            .placeholder(tr!("unlimited"))
            .min(0.0)
            .max(128.0) // api maximum
            .submit_empty(true);

        let affinity_label = tr!("CPU Affinity");
        let affinity_field = Field::new()
            .name("affinity")
            .placeholder(tr!("All Cores"))
            .submit_empty(true);

        let numa_label = tr!("Enable NUMA");
        let numa_field = Checkbox::new().name("numa").switch(mobile);

        let panel = InputPanel::new()
            .mobile(mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .padding_bottom(1); // avoid scrollbar

        if mobile {
            panel
                .with_child(label_field(vcpus_label, vcpus_field, true))
                .with_child(label_field(units_label, units_field, true))
                .with_child(label_field(limit_label, limit_field, true))
                .with_child(label_field(affinity_label, affinity_field, true))
                .with_child(
                    Row::new()
                        .padding_top(1)
                        .class(pwt::css::AlignItems::Center)
                        .with_child(numa_label)
                        .with_flex_spacer()
                        .with_child(numa_field),
                )
                .into()
        } else {
            panel
                .with_field(vcpus_label, vcpus_field)
                .with_right_field(units_label, units_field)
                .with_field(limit_label, limit_field)
                .with_right_field(numa_label, numa_field)
                .with_field(affinity_label, affinity_field)
                .into()
        }
    })
}

pub fn qemu_kernel_scheduler_property(mobile: bool) -> EditableProperty {
    EditableProperty::new("cpuunits", tr!("Kernel scheduler settings"))
        .required(true)
        .renderer(renderer)
        .render_input_panel(kernel_scheduler_input_panel(mobile))
        .submit_hook(|state: PropertyEditorState| {
            let mut record = state.get_submit_data();

            if let Some(cpulimit) = record["cpulimit"].as_f64() {
                if cpulimit == 0.0 {
                    record["cpulimit"] = Value::Null;
                }
            }

            let record = delete_empty_values(
                &record,
                &["vcpus", "cpuunits", "cpulimit", "affinity", "numa"],
                false,
            );
            Ok(record)
        })
}
