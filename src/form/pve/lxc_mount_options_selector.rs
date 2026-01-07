use std::collections::HashSet;

use anyhow::Error;
use pwt::props::{CssLength, PwtSpace};
use serde_json::Value;

use pwt::prelude::*;
use pwt::widget::form::{
    Checkbox, ManagedField, ManagedFieldContext, ManagedFieldMaster, ManagedFieldScopeExt,
    ManagedFieldState,
};
use pwt::widget::{Column, Row};

use pwt_macros::{builder, widget};

#[widget(comp=ManagedFieldMaster<LxcMountOptionsMaster>, @input)]
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct LxcMountOptionsSelector {}

impl LxcMountOptionsSelector {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub enum Msg {
    SetValue(String, bool),
}

#[doc(hidden)]
pub struct LxcMountOptionsMaster {
    state: ManagedFieldState,
    selection: HashSet<String>,
}

impl LxcMountOptionsMaster {
    pub fn update_selection(&mut self, value: Value) {
        let value = match value {
            Value::String(s) => s,
            Value::Array(_) => {
                return; // internal state, no update necessary
            }
            _ => {
                log::error!("unable to parse lxc mount options string: got wrong type");
                String::new()
            }
        };

        let mut selection = HashSet::new();
        for part in value.split(',') {
            selection.insert(part.to_string());
        }

        self.selection = selection;
    }
}

pwt::impl_deref_mut_property!(LxcMountOptionsMaster, state, ManagedFieldState);

impl ManagedField for LxcMountOptionsMaster {
    type Message = Msg;
    type Properties = LxcMountOptionsSelector;
    type ValidateClosure = ();

    fn validation_args(_props: &Self::Properties) -> Self::ValidateClosure {
        ()
    }

    fn validator(_props: &Self::ValidateClosure, value: &Value) -> Result<Value, Error> {
        let value = match value {
            Value::Array(list) => {
                let mut list: Vec<String> = list
                    .iter()
                    .filter_map(|item| item.as_str().map(String::from))
                    .filter(|s| !s.is_empty())
                    .collect();

                list.sort();

                list.join(",");

                Value::from(list.join(","))
            }
            _ => value.clone(),
        };

        Ok(value)
    }

    fn create(_ctx: &ManagedFieldContext<Self>) -> Self {
        let mut me = Self {
            selection: HashSet::new(),
            state: ManagedFieldState::new(Value::Null, Value::Null),
        };
        me.update_selection(me.state.value.clone());
        me
    }

    fn value_changed(&mut self, _ctx: &ManagedFieldContext<Self>) {
        self.update_selection(self.state.value.clone());
    }

    fn update(&mut self, ctx: &ManagedFieldContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SetValue(name, checked) => {
                if checked {
                    self.selection.insert(name);
                } else {
                    self.selection.remove(&name);
                }
            }
        }
        ctx.link()
            .update_value(serde_json::to_value(&self.selection).unwrap());
        true
    }

    fn view(&self, ctx: &ManagedFieldContext<Self>) -> Html {
        let cb = |value: &str| -> Checkbox {
            let checked = self.selection.contains(value);
            let value = value.to_string();
            Checkbox::new().checked(checked).on_input(
                ctx.link()
                    .callback(move |checked| Msg::SetValue(value.clone(), checked)),
            )
        };

        let list = vec![
            "discard", "lazytime", "noatime", "nodev", "noexec", "nosuid",
        ];

        let children: Vec<Html> = list
            .into_iter()
            .map(|value| {
                Column::new()
                    .min_width(CssLength::Px(60.0))
                    .class(pwt::css::AlignItems::Center)
                    .gap(PwtSpace::Em(0.5))
                    .with_child(cb(value))
                    .with_child(value)
                    .into()
            })
            .collect();
        Row::new()
            .gap(1)
            .style("flex-wrap", "wrap")
            .children(children)
            .into()
    }
}
