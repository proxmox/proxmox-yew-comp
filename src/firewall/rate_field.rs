use yew::html::{IntoEventCallback, IntoPropValue};

use pwt::prelude::*;
use pwt::props::FieldBuilder;
use pwt::widget::form::{Combobox, Number};
use pwt::widget::Row;

use pwt::props::WidgetBuilder;
use pwt_macros::{builder, widget};

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
