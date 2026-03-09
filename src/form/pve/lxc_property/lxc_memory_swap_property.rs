use proxmox_human_byte::HumanByte;
use pwt::widget::Container;
use serde_json::Value;

use pwt::prelude::*;
use pwt::widget::{form::Number, InputPanel};

use crate::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

const MEMORY_PN: &str = "memory";
const SWAP_PN: &str = "swap";
const DEFAULT_MEMORY: u64 = 512;

fn renderer(_name: &str, value: &Value, _record: &Value) -> Html {
    match value {
        Value::Null => Container::new()
            .class(pwt::css::Opacity::Half)
            .with_child(DEFAULT_MEMORY)
            .into(),
        Value::Number(memory) => {
            if let Some(memory) = memory.as_f64() {
                HumanByte::new_binary(memory).into()
            } else {
                memory.to_string().into()
            }
        }
        Value::String(s) => s.into(),
        other => other.into(),
    }
}

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |_state: PropertyEditorState| {
        let memory_label = tr!("Memory") + " (MiB)";
        let memory_field = Number::<u64>::new()
            .name(MEMORY_PN)
            .required(true)
            .min(16)
            .step(32);

        let swap_label = tr!("Swap") + " (MiB)";
        let swap_field = Number::<u64>::new()
            .name(SWAP_PN)
            .required(true)
            .min(0)
            .step(32);

        InputPanel::new()
            .class(pwt::css::FlexFit)
            .mobile(mobile)
            .padding_x(2)
            .with_field(memory_label, memory_field)
            .with_field(swap_label, swap_field)
            .into()
    })
}

fn lxc_memory_swap_property(name: &str, title: String, mobile: bool) -> EditableProperty {
    EditableProperty::new(name.to_string(), title)
        .required(true)
        .renderer(renderer)
        .render_input_panel(input_panel(mobile))
        .submit_hook(|state: PropertyEditorState| {
            let form_ctx = state.form_ctx;
            let data = form_ctx.get_submit_data();
            Ok(data)
        })
}

pub fn lxc_memory_property(mobile: bool) -> EditableProperty {
    lxc_memory_swap_property(MEMORY_PN, tr!("Memory"), mobile)
}

pub fn lxc_swap_property(mobile: bool) -> EditableProperty {
    lxc_memory_swap_property(SWAP_PN, tr!("Swap"), mobile)
}
