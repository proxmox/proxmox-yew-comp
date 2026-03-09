use std::rc::Rc;

use pwt::prelude::*;
use pwt::widget::{form::Number, InputPanel};
use serde_json::{json, Value};

use crate::form::delete_default_values;
use crate::{
    form::delete_empty_values, EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn,
};

const CORES_PN: &str = "cores";
const CPU_LIMIT_PN: &str = "cpulimit";
const CPU_UNITS_PN: &str = "cpuunits";

const CPU_UNITS_DEFAULT: u64 = 100;

fn renderer(_name: &str, _value: &Value, record: &Value) -> Html {
    let mut text = match record[CORES_PN].as_u64() {
        Some(cores) => cores.to_string(),
        _ => tr!("unlimited"),
    };

    if let Some(n) = &record[CPU_LIMIT_PN].as_f64() {
        text += &format!(" [cpulimit={n}]");
    }
    if let Some(n) = &record[CPU_UNITS_PN].as_u64() {
        text += &format!(" [cpuunits={n}]");
    }

    text.into()
}

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |state: PropertyEditorState| {
        let form_ctx = state.form_ctx;
        let advanced = form_ctx.get_show_advanced();

        let cores_label = tr!("Cores");
        let cores_field = Number::<u16>::new()
            .name(CORES_PN)
            .placeholder(tr!("unlimited"))
            .min(1)
            .max(8192)
            .submit_empty(true);

        let units_label = tr!("CPU units");
        let units_field = Number::<u64>::new()
            .name(CPU_UNITS_PN)
            .min(8)
            .max(10000)
            .placeholder(CPU_UNITS_DEFAULT.to_string())
            .submit_empty(true);

        let limit_label = tr!("CPU limit");
        let limit_field = Number::<f64>::new()
            .name(CPU_LIMIT_PN)
            .placeholder(tr!("unlimited"))
            .min(0.0)
            .max(8192.0)
            .submit_empty(true);

        InputPanel::new()
            .class(pwt::css::FlexFit)
            .mobile(mobile)
            .show_advanced(advanced)
            .padding_x(2)
            .with_field(cores_label, cores_field)
            .with_advanced_spacer()
            .with_advanced_field(limit_label, limit_field)
            .with_field_and_options(
                pwt::widget::FieldPosition::Right,
                true,
                false,
                units_label,
                units_field,
            )
            .into()
    })
}

pub fn lxc_cores_property(mobile: bool) -> EditableProperty {
    const KEYS: &[&str] = &[CORES_PN, CPU_UNITS_PN, CPU_LIMIT_PN];
    let title = tr!("Cores");
    EditableProperty::new(CORES_PN, title)
        .required(true)
        .advanced_checkbox(true)
        .revert_keys(Rc::new(KEYS.iter().map(|s| AttrValue::from(*s)).collect()))
        .renderer(renderer)
        .render_input_panel(input_panel(mobile))
        .submit_hook(|state: PropertyEditorState| {
            let form_ctx = state.form_ctx;
            let mut data = form_ctx.get_submit_data();

            let defaults = json!({
                CPU_LIMIT_PN: 0,
                CPU_UNITS_PN: CPU_UNITS_DEFAULT,
            });
            delete_default_values(&mut data, &defaults);

            let data = delete_empty_values(&data, KEYS, false);
            Ok(data)
        })
}
