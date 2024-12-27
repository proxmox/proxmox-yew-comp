use std::str::FromStr;

use anyhow::Error;
use serde_json::{json, Value};

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
    /// The default value, i.e. "10KiB"
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default: Option<AttrValue>,

    /// Default unit.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(SizeUnit::Mebi)]
    pub default_unit: SizeUnit,
}

impl Default for BandwidthSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl BandwidthSelector {
    /// Create a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub enum Msg {
    SelectUnit(SizeUnit),
    ChangeSize((String, Option<f64>)),
}

pub struct ProxmoxBandwidthField {
    current_size: String,
    current_unit: String,
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

    fn validator(props: &Self::ValidateClosure, value: &Value) -> Result<Value, Error> {
        let is_empty = match value {
            Value::Null => true,
            Value::Number(_) => false,
            Value::String(v) => v.is_empty(),
            Value::Object(map) => match &map["size"] {
                Value::String(v) => v.is_empty(),
                _ => return Err(Error::msg(tr!("Got wrong data type!"))),
            },
            _ => return Err(Error::msg(tr!("Got wrong data type!"))),
        };

        if is_empty {
            if props.required {
                return Err(Error::msg(tr!("Field may not be empty.")));
            } else {
                return Ok(Value::String(String::new()));
            }
        }

        match value {
            Value::Number(n) => {
                if n.as_f64().is_none() {
                    return Err(Error::msg(tr!("unable to parse number")));
                }
                Ok(Value::Number(n.clone()))
            }
            Value::String(v) => {
                if let Err(err) = HumanByte::from_str(v) {
                    return Err(Error::msg(tr!("unable to parse value: {}", err)));
                }
                Ok(Value::String(v.to_string()))
            }
            Value::Object(map) => match (&map["size"], &map["unit"]) {
                (Value::String(size), Value::String(unit)) => {
                    let size = pwt::dom::parse_float(size).map_err(Error::msg)?;
                    let hb_str = format!("{} {}", size, unit);
                    if let Err(err) = HumanByte::from_str(&hb_str) {
                        return Err(Error::msg(tr!("unable to parse value: {}", err)));
                    }
                    Ok(Value::String(hb_str))
                }
                _ => Err(Error::msg(tr!("Got wrong data type!"))),
            },
            _ => Err(Error::msg(tr!("Got wrong data type!"))),
        }
    }

    fn setup(props: &Self::Properties) -> ManagedFieldState {
        let default: Value = match &props.default {
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
        }
    }

    fn value_changed(&mut self, ctx: &ManagedFieldContext<Self>) {
        let props = ctx.props();
        let state = ctx.state();

        match &state.value {
            Value::Number(n) => {
                if let Some(n) = n.as_f64() {
                    let hb = HumanByte::new_binary(n);
                    self.current_size = pwt::dom::format_float(hb.size);
                    self.current_unit = hb.unit.to_string();
                } else {
                    self.current_size = n.to_string();
                    self.current_unit = "B".into();
                }
            }
            Value::String(v) => {
                if let Ok(hb) = HumanByte::from_str(v) {
                    self.current_size = pwt::dom::format_float(hb.size);
                    self.current_unit = hb.unit.to_string();
                } else {
                    self.current_size = v.into();
                    self.current_unit = props.default_unit.to_string();
                }
            }
            Value::Object(map) => {
                self.current_size = map["size"].as_str().unwrap_or("").to_string();
                if let Some(unit) = map["unit"].as_str() {
                    self.current_unit = unit.to_string();
                } else {
                    self.current_unit = props.default_unit.to_string();
                }
            }
            _ => {
                self.current_size = "".into();
                self.current_unit = props.default_unit.to_string();
            }
        }
    }

    fn create(ctx: &ManagedFieldContext<Self>) -> Self {
        let props = ctx.props();
        let mut me = Self {
            current_size: "".into(),
            current_unit: props.default_unit.to_string(),
        };
        me.value_changed(ctx);
        me
    }

    fn update(&mut self, ctx: &ManagedFieldContext<Self>, msg: Self::Message) -> bool {
        let (size, unit) = match msg {
            Msg::SelectUnit(unit) => (self.current_size.to_string(), unit.to_string()),
            Msg::ChangeSize((size_text, _size)) => (size_text, self.current_unit.to_string()),
        };

        // Note: we cannot store as valid HumanByte, so we store as Object.
        // This preserves localized number text, and errors in number format...

        let new_value = json!({ "size": size, "unit": unit });

        ctx.link().update_value(new_value);

        false
    }

    fn view(&self, ctx: &ManagedFieldContext<Self>) -> Html {
        let props = ctx.props();

        let mut input_props = props.input_props.clone();
        input_props.name = None;

        let input = Number::<f64>::new()
            .with_input_props(&input_props)
            .min(0.0)
            .value(self.current_size.clone())
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

        let unit_selector = MenuButton::new(self.current_unit.to_string())
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
