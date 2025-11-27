use anyhow::Error;
use proxmox_schema::ApiType;
use serde_json::Value;
use yew::html::{IntoEventCallback, IntoPropValue};

use pwt::prelude::*;
use pwt::props::FieldBuilder;
use pwt::widget::form::{
    Checkbox, Combobox, ManagedField, ManagedFieldContext, ManagedFieldMaster, ManagedFieldState,
    Number,
};
use pwt::widget::{Container, InputPanel, Row};

use pwt::props::WidgetBuilder;
use pwt_macros::{builder, widget};

use crate::SchemaValidation;

const TIME_UNITS: &[&str] = &["second", "minute", "hour", "day"];

/// A field widget for entering a rate value in the format "number/unit".
///
/// The rate is displayed as a number input followed by a unit selector
/// (second, minute, hour, or day). The value is formatted as "number/unit"
/// (e.g., "5/minute").
#[widget(comp=RateFieldImpl, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct RateField {
    /// The current value of the rate field in "number/unit" format.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub value: Option<AttrValue>,

    /// Callback invoked when the rate value changes.
    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_input: Option<Callback<String>>,
}

impl Default for RateField {
    fn default() -> Self {
        Self::new()
    }
}

impl RateField {
    /// Creates a new `RateField` with default values.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

enum RateMsg {
    ChangeNumber(Option<u64>),
    ChangeUnit(String),
}

struct RateFieldImpl {
    number: Option<u64>,
    unit: String,
}

impl yew::Component for RateFieldImpl {
    type Message = RateMsg;
    type Properties = RateField;

    fn create(ctx: &yew::Context<Self>) -> Self {
        let mut me = Self {
            number: None,
            unit: "second".to_string(),
        };
        me.parse_value(&ctx.props().value);
        me
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        match msg {
            RateMsg::ChangeNumber(number) => {
                self.number = number;
            }
            RateMsg::ChangeUnit(unit) => self.unit = unit,
        }

        let new_value_str = if let Some(num) = self.number {
            format!("{}/{}", num, self.unit)
        } else {
            String::new()
        };

        if let Some(callback) = &ctx.props().on_input {
            callback.emit(new_value_str);
        }

        true
    }

    fn changed(&mut self, ctx: &yew::Context<Self>, _old_props: &Self::Properties) -> bool {
        self.parse_value(&ctx.props().value);
        true
    }

    fn view(&self, ctx: &yew::Context<Self>) -> Html {
        let is_empty = self.number.is_none();
        let number_value = self.number.map(|n| n.to_string()).unwrap_or_default();

        let units: Vec<AttrValue> = TIME_UNITS.iter().map(|&u| AttrValue::from(u)).collect();

        Row::new()
            .style("align-items", "center")
            .gap(1)
            .with_child(
                Number::<u64>::new()
                    .key("rate_number")
                    .value(number_value)
                    .placeholder("1")
                    .min(1)
                    .max(99)
                    .on_change(ctx.link().callback(|result: Option<Result<u64, String>>| {
                        RateMsg::ChangeNumber(result.and_then(|r| r.ok()))
                    })),
            )
            .with_child("/")
            .with_child(
                Combobox::new()
                    .key("rate_unit")
                    .items(std::rc::Rc::new(units))
                    .value(self.unit.clone())
                    .disabled(is_empty)
                    .required(true)
                    .on_change(ctx.link().callback(RateMsg::ChangeUnit)),
            )
            .into()
    }
}

impl RateFieldImpl {
    fn parse_value(&mut self, value: &Option<AttrValue>) {
        self.number = None;
        self.unit = "second".to_string();

        if let Some(v) = value {
            if !v.is_empty() {
                if let Some((num, unit)) = v.split_once('/') {
                    self.number = num.parse::<u64>().ok();
                    self.unit = unit.to_string();
                }
            }
        }
    }
}

/// A managed field widget for configuring firewall log rate limiting.
///
/// This field allows users to configure:
/// - Whether rate limiting is enabled
/// - The rate limit (e.g., "5/minute")
/// - The burst value
///
/// The value is stored as a property string in the format
/// "enable=0|1,rate=number/unit,burst=number".
#[widget(comp=ManagedFieldMaster<LogRatelimitFieldImpl>, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct LogRatelimitField {
    /// The default value to use when the field is empty.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default: Option<AttrValue>,
}

impl Default for LogRatelimitField {
    fn default() -> Self {
        Self::new()
    }
}

impl LogRatelimitField {
    /// Creates a new `LogRatelimitField` with default values.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

enum LogRatelimitMsg {
    Enable(bool),
    Rate(String),
    Burst(Option<u64>),
}

struct LogRatelimitFieldImpl {
    enable: bool,
    rate: String,
    burst: Option<u64>,
}

impl ManagedField for LogRatelimitFieldImpl {
    type Message = LogRatelimitMsg;
    type Properties = LogRatelimitField;
    type ValidateClosure = ();

    fn validation_args(_props: &Self::Properties) -> Self::ValidateClosure {}

    fn validator(_props: &Self::ValidateClosure, value: &Value) -> Result<Value, Error> {
        Ok(value.clone())
    }

    fn setup(props: &Self::Properties) -> ManagedFieldState {
        let value = Value::Null;
        let default = match &props.default {
            Some(d) => Value::String(d.to_string()),
            None => Value::String(String::new()),
        };
        ManagedFieldState::new(value, default)
    }

    fn value_changed(&mut self, ctx: &ManagedFieldContext<Self>) {
        let state = ctx.state();

        // Initialize with API defaults (enable=true is the API default)
        // When the property string is empty, rate and burst are unset
        self.enable = true;
        self.rate = String::new();
        self.burst = None;

        // If value is Null, use the default instead
        let value_to_parse = match &state.value {
            Value::Null => &state.default,
            other => other,
        };

        if let Value::String(v) = value_to_parse {
            if !v.is_empty() {
                match pve_api_types::ClusterFirewallOptionsLogRatelimit::API_SCHEMA
                    .parse_property_string(v)
                {
                    Ok(parsed) => {
                        match serde_json::from_value::<
                            pve_api_types::ClusterFirewallOptionsLogRatelimit,
                        >(parsed)
                        {
                            Ok(ratelimit) => {
                                self.enable = ratelimit.enable;
                                if let Some(rate) = ratelimit.rate {
                                    self.rate = rate.clone();
                                }
                                self.burst = ratelimit.burst;
                            }
                            Err(e) => {
                                log::error!("Failed to parse log_ratelimit value: {e:?}");
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to parse log_ratelimit property string '{v}': {e:?}");
                    }
                }
            }
        }
    }

    fn create(ctx: &ManagedFieldContext<Self>) -> Self {
        let mut me = Self {
            enable: true,
            rate: String::new(),
            burst: None,
        };
        me.value_changed(ctx);
        me
    }

    fn update(&mut self, ctx: &ManagedFieldContext<Self>, msg: Self::Message) -> bool {
        match msg {
            LogRatelimitMsg::Enable(enable) => self.enable = enable,
            LogRatelimitMsg::Rate(rate) => self.rate = rate,
            LogRatelimitMsg::Burst(burst_value) => self.burst = burst_value,
        }

        let mut parts = Vec::new();
        parts.push(format!("enable={}", if self.enable { 1 } else { 0 }));
        if !self.rate.is_empty() {
            parts.push(format!("rate={}", self.rate));
        }
        if let Some(burst) = self.burst {
            parts.push(format!("burst={}", burst));
        }
        let property_string = parts.join(",");
        let new_value = Value::String(property_string);

        ctx.link().update_value(new_value);
        true
    }

    fn view(&self, ctx: &ManagedFieldContext<Self>) -> Html {
        let props = ctx.props();
        let base_schema = &pve_api_types::ClusterFirewallOptionsLogRatelimit::API_SCHEMA;

        Container::new()
            .style("border", "1px solid var(--pwt-border-color, #ccc)")
            .style("border-radius", "4px")
            .padding(2)
            .with_child(
                InputPanel::new()
                    .with_std_props(&props.std_props)
                    .with_field(
                        tr!("Enable"),
                        Checkbox::new()
                            .key("enable")
                            .checked(self.enable)
                            .on_change(ctx.link().callback(LogRatelimitMsg::Enable)),
                    )
                    .with_field(
                        tr!("Rate"),
                        RateField::new()
                            .key("rate")
                            .value(self.rate.clone())
                            .on_input(ctx.link().callback(LogRatelimitMsg::Rate)),
                    )
                    .with_field(
                        tr!("Burst"),
                        Number::<u64>::new()
                            .key("burst")
                            .value(self.burst.map(|b| b.to_string()))
                            .on_change(ctx.link().callback(
                                |result: Option<Result<u64, String>>| {
                                    LogRatelimitMsg::Burst(result.and_then(|r| r.ok()))
                                },
                            ))
                            .schema(crate::form::get_field_schema(base_schema, vec!["burst"])),
                    ),
            )
            .into()
    }
}
