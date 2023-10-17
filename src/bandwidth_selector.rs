use std::str::FromStr;

use anyhow::Error;
use serde_json::Value;

use pwt::widget::menu::{Menu, MenuButton, MenuItem};
use yew::html::IntoPropValue;

use pwt::prelude::*;
use pwt::widget::form::{
    ManagedField, ManagedFieldContext, ManagedFieldMaster, ManagedFieldState, Number,
};
use pwt::widget::Container;

use pwt::props::{FieldBuilder, WidgetBuilder};
use pwt_macros::{builder, widget};

use proxmox_human_byte::{HumanByte, SizeUnit};

pub type ProxmoxBandwidthSelector = ManagedFieldMaster<ProxmoxBandwidthField>;

#[widget(comp=ManagedFieldMaster<ProxmoxBandwidthField>, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct BandwidthSelector {
    /// The default value.
    #[prop_or_default]
    pub default: Option<HumanByte>,

    /// Default unit.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(SizeUnit::Mebi)]
    pub default_unit: SizeUnit,
}

pub trait IntoOptionalHumanByte {
    fn into_optional_human_byte(self) -> Option<HumanByte>;
}

impl IntoOptionalHumanByte for HumanByte {
    fn into_optional_human_byte(self) -> Option<HumanByte> {
        Some(self)
    }
}

impl IntoOptionalHumanByte for Option<HumanByte> {
    fn into_optional_human_byte(self) -> Option<HumanByte> {
        self
    }
}

impl IntoOptionalHumanByte for u64 {
    fn into_optional_human_byte(self) -> Option<HumanByte> {
        Some(HumanByte::new_binary(self as f64))
    }
}

impl IntoOptionalHumanByte for usize {
    fn into_optional_human_byte(self) -> Option<HumanByte> {
        Some(HumanByte::new_binary(self as f64))
    }
}

impl BandwidthSelector {
    /// Create a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    /// Builder style method to set the default value.
    pub fn default(mut self, default: impl IntoOptionalHumanByte) -> Self {
        self.set_default(default);
        self
    }

    /// Method to set the default value.
    pub fn set_default(&mut self, default: impl IntoOptionalHumanByte) {
        self.default = default.into_optional_human_byte();
    }
}

pub enum Msg {
    SelectUnit(SizeUnit),
    ChangeSize(String),
}

pub struct ProxmoxBandwidthField {
    current_value: Option<HumanByte>,
}
#[derive(PartialEq)]
pub struct ValidateClosure {
    required: bool,
}

impl ManagedField for ProxmoxBandwidthField {
    type Message = Msg;
    type Properties = BandwidthSelector;
    type ValidateClosure = ValidateClosure;

    fn validation_args(props: &Self::Properties) -> Self::ValidateClosure {
        ValidateClosure {
            required: props.input_props.required,
        }
    }
    fn validator(props: &Self::ValidateClosure, value: &Value) -> Result<(), Error> {
        let is_empty = match value {
            Value::Null => true,
            Value::Number(_) => false,
            Value::String(v) => v.is_empty(),
            _ => return Err(Error::msg(tr!("Got wrong data type!"))),
        };

        if is_empty {
            if props.required {
                return Err(Error::msg(tr!("Field may not be empty.")));
            } else {
                return Ok(());
            }
        }

        match value {
            Value::Number(n) => {
                if n.as_f64().is_none() {
                    return Err(Error::msg(tr!("unable to parse number")));
                }
            }
            Value::String(v) => {
                if let Err(err) = HumanByte::from_str(v) {
                    return Err(Error::msg(tr!("unable to parse value: {}", err)));
                }
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    fn setup(props: &Self::Properties) -> ManagedFieldState {
        let default: Value = match props.default {
            Some(default) => default.to_string().into(),
            None => String::new().into(),
        };

        let value: Value = default.clone();

        ManagedFieldState {
            value,
            valid: Ok(()),
            default,
            radio_group: false,
            unique: false,
            submit_converter: None,
        }
    }

    fn value_changed(&mut self, ctx: &ManagedFieldContext<Self>) {
        let state = ctx.state();

        match &state.value {
            Value::Number(n) => {
                self.current_value = n.as_f64().map(|n| HumanByte::new_binary(n));
            }
            Value::String(v) => {
                self.current_value = HumanByte::from_str(&v).ok();
            }
            _ => {
                self.current_value = None;
            }
        }
    }

    fn create(ctx: &ManagedFieldContext<Self>) -> Self {
        let mut me = Self {
            current_value: None,
        };
        me.value_changed(ctx);
        me
    }

    fn update(&mut self, ctx: &ManagedFieldContext<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::SelectUnit(unit) => {
                match &mut self.current_value {
                    Some(hb) => hb.unit = unit,
                    None => {
                        self.current_value = Some(HumanByte::with_unit(0.0, unit).unwrap());
                    }
                }
                let new_value: Value = self
                    .current_value
                    .as_ref()
                    .map(|hb| hb.to_string())
                    .unwrap_or(String::new())
                    .into();
                ctx.link().update_value(new_value);
                false
            }
            Msg::ChangeSize(size_text) => {
                if size_text.is_empty() {
                    ctx.link().update_value(Value::from(size_text));
                } else {
                    let unit = self
                        .current_value
                        .map(|hb| hb.unit)
                        .unwrap_or(props.default_unit);
                    let new_value: Value = format!("{}{}", size_text, unit).into();
                    ctx.link().update_value(new_value);
                }
                false
            }
        }
    }

    fn view(&self, ctx: &ManagedFieldContext<Self>) -> Html {
        let props = ctx.props();

        let mut input_props = props.input_props.clone();
        input_props.name = None;

        let input = Number::<u64>::new()
            .with_input_props(&input_props)
            .value(self.current_value.map(|hb| hb.size as u64))
            .valid(ctx.state().valid.clone())
            .on_input(ctx.link().callback(Msg::ChangeSize));

        let mut menu = Menu::new();

        for unit in [
            SizeUnit::Byte,
            SizeUnit::KByte,
            SizeUnit::Kibi,
            SizeUnit::MByte,
            SizeUnit::Mebi,
            SizeUnit::GByte,
            SizeUnit::Gibi,
        ] {
            menu.add_item(
                MenuItem::new(unit.to_string())
                    .on_select(ctx.link().callback(move |_| Msg::SelectUnit(unit))),
            )
        }

        let current_unit = self
            .current_value
            .map(|hb| hb.unit)
            .unwrap_or(props.default_unit);

        let unit_selector = MenuButton::new(current_unit.to_string())
            .show_arrow(true)
            .menu(menu);

        Container::new()
            .with_std_props(&props.std_props)
            .class("pwt-d-flex pwt-flex-fill-first-child pwt-gap-2")
            .with_child(input)
            .with_child(unit_selector)
            .into()
    }
}
