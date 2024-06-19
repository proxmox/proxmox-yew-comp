use anyhow::{bail, Error};
use serde_json::{json, Value};
use std::rc::Rc;

use yew::html::IntoEventCallback;
use yew::prelude::*;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::state::local_storage;
use pwt::widget::form::Combobox;

use pwt_macros::builder;

/// Combobox for selecting the theme density.
///
/// You can use the `on_change` callback to listen for changes.
/// Outside this widget, you can listen to the DOM `proxmox-rrd-timeframe-changed` event
/// to track changes.
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct RRDTimeframeSelector {
    #[prop_or_default]
    class: Classes,

    #[builder_cb(IntoEventCallback, into_event_callback, RRDTimeframe)]
    #[prop_or_default]
    on_change: Option<Callback<RRDTimeframe>>,
}

impl RRDTimeframeSelector {
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    /// Builder style method to add a html class
    pub fn class(mut self, class: impl Into<Classes>) -> Self {
        self.add_class(class);
        self
    }

    /// Method to add a html class
    pub fn add_class(&mut self, class: impl Into<Classes>) {
        self.class.push(class);
    }
}

#[derive(Copy, Clone, PartialEq, Default)]
pub enum RRDTimeframe {
    HourAvg,
    HourMax,
    #[default]
    DayAvg,
    DayMax,
    WeekAvg,
    WeekMax,
    MonthAvg,
    MonthMax,
    YearAvg,
    YearMax,
    DecadeAvg,
    DecadeMax,
}

impl TryFrom<&str> for RRDTimeframe {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "hour-avg" => RRDTimeframe::HourAvg,
            "hour-max" => RRDTimeframe::HourMax,
            "day-avg" => RRDTimeframe::DayAvg,
            "day-max" => RRDTimeframe::DayMax,
            "week-avg" => RRDTimeframe::WeekAvg,
            "week-max" => RRDTimeframe::WeekMax,
            "month-avg" => RRDTimeframe::MonthAvg,
            "month-max" => RRDTimeframe::MonthMax,
            "year-avg" => RRDTimeframe::YearAvg,
            "year-max" => RRDTimeframe::YearMax,
            "decade-avg" => RRDTimeframe::DecadeAvg,
            "decade-max" => RRDTimeframe::DecadeMax,
            _ => bail!("'{}' is not a valid RRD timeframe", value),
        })
    }
}

fn emit_rrd_timeframe_changed_event() {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            let event = web_sys::Event::new("proxmox-rrd-timeframe-changed").unwrap();
            let _ = document.dispatch_event(&event);
        }
    }
}

impl RRDTimeframe {
    pub fn load() -> Self {
        let timeframe = RRDTimeframe::default();

        let store = match local_storage() {
            Some(store) => store,
            None => return timeframe,
        };

        if let Ok(Some(timeframe_str)) = store.get_item("ProxmoxRRDTimeframe") {
            if let Ok(timeframe) = RRDTimeframe::try_from(timeframe_str.as_str()) {
                return timeframe;
            }
        }

        timeframe
    }

    fn serialize(&self) -> String {
        match self {
            RRDTimeframe::HourAvg => "hour-avg",
            RRDTimeframe::HourMax => "hour-max",
            RRDTimeframe::DayAvg => "day-avg",
            RRDTimeframe::DayMax => "day-max",
            RRDTimeframe::WeekAvg => "week-avg",
            RRDTimeframe::WeekMax => "week-max",
            RRDTimeframe::MonthAvg => "month-avg",
            RRDTimeframe::MonthMax => "month-max",
            RRDTimeframe::YearAvg => "year-avg",
            RRDTimeframe::YearMax => "year-max",
            RRDTimeframe::DecadeAvg => "decade-avg",
            RRDTimeframe::DecadeMax => "decade-max",
        }
        .to_string()
    }

    pub fn store(&self) {
        if let Some(store) = local_storage() {
            let timeframe_str = self.serialize();
            if let Err(_) = store.set_item("ProxmoxRRDTimeframe", &timeframe_str) {
                log::error!("RRDTimeframe::store - set_item failed");
            } else {
                emit_rrd_timeframe_changed_event();
            }
        } else {
            log::error!("RRDTimeframe::store  - no storage");
        }
    }

    pub fn api_params(&self) -> Value {
        match self {
            RRDTimeframe::HourAvg => {
                json!({"timeframe": "hour", "cf": "AVERAGE"})
            }
            RRDTimeframe::HourMax => {
                json!({"timeframe": "hour", "cf": "MAX"})
            }
            RRDTimeframe::DayAvg => {
                json!({"timeframe": "day", "cf": "AVERAGE"})
            }
            RRDTimeframe::DayMax => {
                json!({"timeframe": "day", "cf": "MAX"})
            }
            RRDTimeframe::WeekAvg => {
                json!({"timeframe": "week", "cf": "AVERAGE"})
            }
            RRDTimeframe::WeekMax => {
                json!({"timeframe": "week", "cf": "MAX"})
            }
            RRDTimeframe::MonthAvg => {
                json!({"timeframe": "month", "cf": "AVERAGE"})
            }
            RRDTimeframe::MonthMax => {
                json!({"timeframe": "month", "cf": "MAX"})
            }
            RRDTimeframe::YearAvg => {
                json!({"timeframe": "year", "cf": "AVERAGE"})
            }
            RRDTimeframe::YearMax => {
                json!({"timeframe": "year", "cf": "MAX"})
            }
            RRDTimeframe::DecadeAvg => {
                json!({"timeframe": "decade", "cf": "AVERAGE"})
            }
            RRDTimeframe::DecadeMax => {
                json!({"timeframe": "decade", "cf": "MAX"})
            }
        }
    }
}

#[doc(hidden)]
pub struct PwtRRDTimeframeSelector {
    timeframe: RRDTimeframe,
    items: Rc<Vec<AttrValue>>,
}

pub enum Msg {
    SetRRDTimeframe(String),
}

fn display_value(v: &AttrValue) -> &str {
    match v.as_str() {
        "hour-avg" => "Hour (average)",
        "hour-max" => "Hour (maximum)",
        "day-avg" => "Day (average)",
        "day-max" => "Day (maximum)",
        "week-avg" => "Week (average)",
        "week-max" => "Week (maximum)",
        "month-avg" => "Month (average)",
        "month-max" => "Month (maximum)",
        "year-avg" => "Year (average)",
        "year-max" => "Year (maximum)",
        "decade-avg" => "Decade (average)",
        "decade-max" => "Decade (maximum)",
        _ => v,
    }
}

impl Component for PwtRRDTimeframeSelector {
    type Message = Msg;
    type Properties = RRDTimeframeSelector;

    fn create(_ctx: &Context<Self>) -> Self {
        let timeframe = RRDTimeframe::load();

        let values = [
            RRDTimeframe::HourAvg,
            RRDTimeframe::HourMax,
            RRDTimeframe::DayAvg,
            RRDTimeframe::DayMax,
            RRDTimeframe::WeekAvg,
            RRDTimeframe::WeekMax,
            RRDTimeframe::MonthAvg,
            RRDTimeframe::MonthMax,
            RRDTimeframe::YearAvg,
            RRDTimeframe::YearMax,
            RRDTimeframe::DecadeAvg,
            RRDTimeframe::DecadeMax,
        ]
        .iter()
        .map(|v| AttrValue::from(v.serialize()))
        .collect();

        Self {
            timeframe,
            items: Rc::new(values),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::SetRRDTimeframe(timeframe_str) => {
                if let Ok(timeframe) = RRDTimeframe::try_from(timeframe_str.as_str()) {
                    timeframe.store();
                    self.timeframe = timeframe;
                    if let Some(on_change) = &props.on_change {
                        on_change.emit(timeframe);
                    }
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Combobox::new()
            .min_width(150)
            .class(props.class.clone())
            .default(self.timeframe.serialize())
            .items(self.items.clone())
            .on_change(ctx.link().callback(Msg::SetRRDTimeframe))
            .render_value(|v: &AttrValue| {
                html! {display_value(v)}
            })
            .show_filter(false)
            // Note: This is just for completeness. Not used because we do not show the filter...
            .filter(|item: &AttrValue, query: &str| {
                display_value(item)
                    .to_lowercase()
                    .contains(&query.to_lowercase())
            })
            .into()
    }
}

impl Into<VNode> for RRDTimeframeSelector {
    fn into(self) -> VNode {
        let comp = VComp::new::<PwtRRDTimeframeSelector>(Rc::new(self), None);
        VNode::from(comp)
    }
}
