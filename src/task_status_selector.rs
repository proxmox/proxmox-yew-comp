use serde_json::Value;

use pwt::prelude::*;
use pwt::widget::form::{
    ManagedField, ManagedFieldContext, ManagedFieldMaster, ManagedFieldState,
};
use pwt::widget::{Button, SegmentedButton};

use pwt_macros::widget;

#[widget(comp=ManagedFieldMaster<ProxmoxTaskStatusSelector>, @input)]
#[derive(Clone, Properties, PartialEq)]
pub struct TaskStatusSelector {}

impl TaskStatusSelector {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub enum Msg {
    ToggleAll,
    ToggleOk,
    ToggleErrors,
    ToggleWarnings,
    ToggleUnknown,
}

pub struct ProxmoxTaskStatusSelector {
    all: bool,
    ok: bool,
    errors: bool,
    warnings: bool,
    unknown: bool,
}

impl ManagedField for ProxmoxTaskStatusSelector {
    type Properties = TaskStatusSelector;
    type Message = Msg;

    fn validation_fn_need_update(_props: &Self::Properties, _old_props: &Self::Properties) -> bool {
        false
    }

    fn setup(_props: &Self::Properties) -> ManagedFieldState {
        let value = vec![true, false, false, false, false];
        let default = value.clone();

        ManagedFieldState {
            value: value.into(),
            valid: Ok(()),
            default: default.into(),
            radio_group: false,
            unique: false,
            submit_converter: Some(Callback::from(|value: Value| -> Value {
                let mut filter: Vec<Value> = Vec::new();
                let data: Result<[bool; 5], _> = serde_json::from_value(value);
                if let Ok(data) = data {
                    if !data[0] {
                        if data[1] {
                            filter.push("ok".into());
                        }
                        if data[2] {
                            filter.push("error".into());
                        }
                        if data[3] {
                            filter.push("warning".into());
                        }
                        if data[4] {
                            filter.push("unknown".into());
                        }
                    }
                }
                Value::Array(filter)
            })),
        }
    }

    fn value_changed(&mut self, ctx: &ManagedFieldContext<Self>) {
        let state = ctx.state();

        let value: Result<[bool; 5], _> = serde_json::from_value(state.value.clone());
        if let Ok(data) = value {
            self.all = data[0];
            self.ok = data[1];
            self.errors = data[2];
            self.warnings = data[3];
            self.unknown = data[4];
        }
    }

    fn create(_ctx: &ManagedFieldContext<Self>) -> Self {
        Self {
            all: true,
            ok: false,
            errors: false,
            warnings: false,
            unknown: false,
        }
    }

    fn update(&mut self, ctx: &ManagedFieldContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::ToggleAll => {
                self.all = !self.all;
                if self.all {
                    self.ok = false;
                    self.errors = false;
                    self.warnings = false;
                    self.unknown = false;
                }
            }
            Msg::ToggleOk => {
                self.ok = !self.ok;
                self.all = false;
            }
            Msg::ToggleErrors => {
                self.errors = !self.errors;
                self.all = false;
            }
            Msg::ToggleWarnings => {
                self.warnings = !self.warnings;
                self.all = false;
            }
            Msg::ToggleUnknown => {
                self.unknown = !self.unknown;
                self.all = false;
            }
        }
        if !(self.ok || self.errors || self.warnings || self.unknown) {
            self.all = true;
        }

        ctx.link().update_value(vec![
            self.all,
            self.ok,
            self.errors,
            self.warnings,
            self.unknown,
        ]);

        true
    }

    fn view(&self, ctx: &ManagedFieldContext<Self>) -> Html {
        let pressed_scheme = "pwt-scheme-secondary-container";
        SegmentedButton::new()
            .class("pwt-button-elevated")
            .with_button(
                Button::new(tr!("All"))
                    .pressed(self.all)
                    .class(self.all.then(|| pressed_scheme))
                    .onclick(ctx.link().callback(|_| Msg::ToggleAll)),
            )
            .with_button(
                Button::new(tr!("Ok"))
                    .pressed(self.ok)
                    .class(self.ok.then(|| pressed_scheme))
                    .onclick(ctx.link().callback(|_| Msg::ToggleOk)),
            )
            .with_button(
                Button::new(tr!("Errors"))
                    .pressed(self.errors)
                    .class(self.errors.then(|| pressed_scheme))
                    .onclick(ctx.link().callback(|_| Msg::ToggleErrors)),
            )
            .with_button(
                Button::new(tr!("Warnings"))
                    .pressed(self.warnings)
                    .class(self.warnings.then(|| pressed_scheme))
                    .onclick(ctx.link().callback(|_| Msg::ToggleWarnings)),
            )
            .with_button(
                Button::new(tr!("Unknown"))
                    .pressed(self.unknown)
                    .class(self.unknown.then(|| pressed_scheme))
                    .onclick(ctx.link().callback(|_| Msg::ToggleUnknown)),
            )
            .into()
    }
}
