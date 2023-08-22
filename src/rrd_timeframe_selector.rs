use anyhow::{bail, Error};
use serde_json::{json, Value};
use std::rc::Rc;

use serde::{Deserialize, Serialize};

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

#[derive(Copy, Clone, PartialEq, Default, Serialize, Deserialize)]
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

impl std::fmt::Display for RRDTimeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str(match self {
            RRDTimeframe::HourAvg => "Hour (average)",
            RRDTimeframe::HourMax => "Hour (maximum)",
            RRDTimeframe::DayAvg => "Day (average)",
            RRDTimeframe::DayMax => "Day (maximum)",
            RRDTimeframe::WeekAvg => "Week (average)",
            RRDTimeframe::WeekMax => "Week (maximum)",
            RRDTimeframe::MonthAvg => "Month (average)",
            RRDTimeframe::MonthMax => "Month (maximum)",
            RRDTimeframe::YearAvg => "Year (average)",
            RRDTimeframe::YearMax => "Year (maximum)",
            RRDTimeframe::DecadeAvg => "Decade (average)",
            RRDTimeframe::DecadeMax => "Decade (maximum)",
        })
    }
}

impl TryFrom<&str> for RRDTimeframe {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "Hour (average)" => RRDTimeframe::HourAvg,
            "Hour (maximum)" => RRDTimeframe::HourMax,
            "Day (average)" => RRDTimeframe::DayAvg,
            "Day (maximum)" => RRDTimeframe::DayMax,
            "Week (average)" => RRDTimeframe::WeekAvg,
            "Week (maximum)" => RRDTimeframe::WeekMax,
            "Month (average)" => RRDTimeframe::MonthAvg,
            "Month (maximum)" => RRDTimeframe::MonthMax,
            "Year (average)" => RRDTimeframe::YearAvg,
            "Year (maximum)" => RRDTimeframe::YearMax,
            "Decade (average)" => RRDTimeframe::DecadeAvg,
            "Decade (maximum)" => RRDTimeframe::DecadeMax,
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
            if let Ok(timeframe) = serde_json::from_str(&timeframe_str) {
                return timeframe;
            }
        }

        timeframe
    }

    pub fn store(&self) {
        if let Some(store) = local_storage() {
            let timeframe_str = serde_json::to_string(self).unwrap();
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
    SetRRDTimeframe(RRDTimeframe),
}

impl Component for PwtRRDTimeframeSelector {
    type Message = Msg;
    type Properties = RRDTimeframeSelector;

    fn create(_ctx: &Context<Self>) -> Self {
        let timeframe = RRDTimeframe::load();

        Self {
            timeframe,
            items: Rc::new(vec![
                AttrValue::from(RRDTimeframe::HourAvg.to_string()),
                AttrValue::from(RRDTimeframe::HourMax.to_string()),
                AttrValue::from(RRDTimeframe::DayAvg.to_string()),
                AttrValue::from(RRDTimeframe::DayMax.to_string()),
                AttrValue::from(RRDTimeframe::WeekAvg.to_string()),
                AttrValue::from(RRDTimeframe::WeekMax.to_string()),
                AttrValue::from(RRDTimeframe::MonthAvg.to_string()),
                AttrValue::from(RRDTimeframe::MonthMax.to_string()),
                AttrValue::from(RRDTimeframe::YearAvg.to_string()),
                AttrValue::from(RRDTimeframe::YearMax.to_string()),
                AttrValue::from(RRDTimeframe::DecadeAvg.to_string()),
                AttrValue::from(RRDTimeframe::DecadeMax.to_string()),
            ]),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::SetRRDTimeframe(timeframe) => {
                if let Ok(timeframe) = RRDTimeframe::try_from(timeframe) {
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
            .class(props.class.clone())
            .default(self.timeframe.to_string())
            .show_filter(false)
            .items(self.items.clone())
            .on_change(ctx.link().callback(|timeframe: String| {
                let timeframe = RRDTimeframe::try_from(timeframe.as_str()).unwrap_or_default();
                Msg::SetRRDTimeframe(timeframe)
            }))
            .into()
    }
}

impl Into<VNode> for RRDTimeframeSelector {
    fn into(self) -> VNode {
        let comp = VComp::new::<PwtRRDTimeframeSelector>(Rc::new(self), None);
        VNode::from(comp)
    }
}
