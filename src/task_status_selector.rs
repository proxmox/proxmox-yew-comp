use anyhow::Error;
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use pwt::prelude::*;
use pwt::widget::form::{ManagedField, ManagedFieldContext, ManagedFieldMaster, ManagedFieldState};
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

pub struct ProxmoxTaskStatusSelector {}

#[derive(Copy, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum TaskFilterEntry {
    Ok,
    Error,
    Warning,
    Unknown,
}

impl ManagedField for ProxmoxTaskStatusSelector {
    type Properties = TaskStatusSelector;
    type Message = Msg;
    type ValidateClosure = ();

    fn validation_args(_props: &Self::Properties) -> Self::ValidateClosure {
        ()
    }

    fn validator(_props: &Self::ValidateClosure, value: &Value) -> Result<Value, Error> {
        let filter: Vec<TaskFilterEntry> = serde_json::from_value(value.clone())?;

        let filter_map: HashSet<TaskFilterEntry> = filter.into_iter().collect();

        let list: Vec<String> = filter_map
            .iter()
            .map(|i| serde_plain::to_string(i).unwrap())
            .collect();
        Ok(list.into())
    }

    fn setup(_props: &Self::Properties) -> ManagedFieldState {
        let value: Vec<String> = vec![];
        let default = value.clone();

        ManagedFieldState {
            value: value.into(),
            valid: Ok(()),
            default: default.into(),
            radio_group: false,
            unique: false,
        }
    }

    fn create(_ctx: &ManagedFieldContext<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, ctx: &ManagedFieldContext<Self>, msg: Self::Message) -> bool {
        let state = ctx.state();
        let filter: Vec<TaskFilterEntry> =
            serde_json::from_value(state.value.clone()).unwrap_or(Vec::new());
        let mut filter_map: HashSet<TaskFilterEntry> = filter.into_iter().collect();

        match msg {
            Msg::ToggleAll => {
                filter_map = HashSet::new();
            }
            Msg::ToggleOk => {
                if !filter_map.remove(&TaskFilterEntry::Ok) {
                    filter_map.insert(TaskFilterEntry::Ok);
                }
            }
            Msg::ToggleErrors => {
                if !filter_map.remove(&TaskFilterEntry::Error) {
                    filter_map.insert(TaskFilterEntry::Error);
                }
            }
            Msg::ToggleWarnings => {
                if !filter_map.remove(&TaskFilterEntry::Warning) {
                    filter_map.insert(TaskFilterEntry::Warning);
                }
            }
            Msg::ToggleUnknown => {
                if !filter_map.remove(&TaskFilterEntry::Unknown) {
                    filter_map.insert(TaskFilterEntry::Unknown);
                }
            }
        }

        let list: Vec<String> = filter_map
            .iter()
            .map(|i| serde_plain::to_string(i).unwrap())
            .collect();

        ctx.link().update_value(list);

        true
    }

    fn view(&self, ctx: &ManagedFieldContext<Self>) -> Html {
        let state = ctx.state();
        let filter: Vec<TaskFilterEntry> =
            serde_json::from_value(state.value.clone()).unwrap_or(Vec::new());
        let unique_map: HashSet<TaskFilterEntry> = filter.into_iter().collect();

        let pressed_scheme = "pwt-scheme-secondary-container";
        SegmentedButton::new()
            .class("pwt-button-elevated")
            .with_button(
                Button::new(tr!("All"))
                    .pressed(unique_map.is_empty())
                    .class(unique_map.is_empty().then(|| pressed_scheme))
                    .onclick(ctx.link().callback(|_| Msg::ToggleAll)),
            )
            .with_button(
                Button::new(tr!("Ok"))
                    .pressed(unique_map.contains(&TaskFilterEntry::Ok))
                    .class(
                        unique_map
                            .contains(&TaskFilterEntry::Ok)
                            .then(|| pressed_scheme),
                    )
                    .onclick(ctx.link().callback(|_| Msg::ToggleOk)),
            )
            .with_button(
                Button::new(tr!("Errors"))
                    .pressed(unique_map.contains(&TaskFilterEntry::Error))
                    .class(
                        unique_map
                            .contains(&TaskFilterEntry::Error)
                            .then(|| pressed_scheme),
                    )
                    .onclick(ctx.link().callback(|_| Msg::ToggleErrors)),
            )
            .with_button(
                Button::new(tr!("Warnings"))
                    .pressed(unique_map.contains(&TaskFilterEntry::Warning))
                    .class(
                        unique_map
                            .contains(&TaskFilterEntry::Warning)
                            .then(|| pressed_scheme),
                    )
                    .onclick(ctx.link().callback(|_| Msg::ToggleWarnings)),
            )
            .with_button(
                Button::new(tr!("Unknown"))
                    .pressed(unique_map.contains(&TaskFilterEntry::Unknown))
                    .class(
                        unique_map
                            .contains(&TaskFilterEntry::Unknown)
                            .then(|| pressed_scheme),
                    )
                    .onclick(ctx.link().callback(|_| Msg::ToggleUnknown)),
            )
            .into()
    }
}
