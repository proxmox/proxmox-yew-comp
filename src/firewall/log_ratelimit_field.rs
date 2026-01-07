use anyhow::Error;
use proxmox_schema::ApiType;
use serde_json::Value;
use yew::html::IntoPropValue;

use pwt::prelude::*;
use pwt::props::WidgetBuilder;
use pwt::widget::form::{
    Checkbox, ManagedField, ManagedFieldContext, ManagedFieldMaster, ManagedFieldScopeExt,
    ManagedFieldState, Number,
};
use pwt::widget::{Container, InputPanel};

use pwt_macros::{builder, widget};

use crate::SchemaValidation;

use super::RateField;

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
    state: ManagedFieldState,
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

    fn value_changed(&mut self, _ctx: &ManagedFieldContext<Self>) {
        // Initialize with API defaults (enable=true is the API default)
        // When the property string is empty, rate and burst are unset
        self.enable = true;
        self.rate = String::new();
        self.burst = None;

        // If value is Null, use the default instead
        let value_to_parse = match &self.state.value {
            Value::Null => &self.state.default,
            other => other,
        };

        if let Value::String(v) = value_to_parse {
            if !v.is_empty() {
                match pve_api_types::ClusterFirewallOptionsLogRatelimit::API_SCHEMA
                    .parse_property_string(&v)
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
        let value = Value::Null;
        let default = match &ctx.props().default {
            Some(d) => Value::String(d.to_string()),
            None => Value::String(String::new()),
        };
        let mut me = Self {
            state: ManagedFieldState::new(value, default),
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

crate::impl_deref_mut_property!(LogRatelimitFieldImpl, state, ManagedFieldState);
