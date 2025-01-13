use anyhow::{format_err, Error};
use serde_json::{json, Value};
use std::rc::Rc;

use yew::html::IntoEventCallback;
use yew::prelude::*;
use yew::virtual_dom::{VComp, VNode};

use proxmox_rrd_api_types as rrd_types;
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

#[derive(Copy, Clone, PartialEq)]
pub struct RRDTimeframe {
    pub timeframe: rrd_types::RrdTimeframe,
    pub mode: rrd_types::RrdMode,
}

impl RRDTimeframe {
    pub const fn new(timeframe: rrd_types::RrdTimeframe, mode: rrd_types::RrdMode) -> Self {
        Self { timeframe, mode }
    }
}

impl Default for RRDTimeframe {
    fn default() -> Self {
        Self::new(rrd_types::RrdTimeframe::Day, rrd_types::RrdMode::Average)
    }
}

impl std::str::FromStr for RRDTimeframe {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Error> {
        value
            .split_once('-')
            .and_then(|(timeframe, mode)| {
                Some(Self {
                    timeframe: timeframe.parse().ok()?,
                    mode: mode.parse().ok()?,
                })
            })
            .ok_or_else(|| format_err!("{value:?} is not a valid RRD timeframe"))
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
            if let Ok(timeframe) = timeframe_str.as_str().parse() {
                return timeframe;
            }
        }

        timeframe
    }

    fn serialize(&self) -> String {
        format!("{}-{}", self.timeframe, self.mode)
    }

    pub fn store(&self) {
        if let Some(store) = local_storage() {
            let timeframe_str = self.serialize();
            if store
                .set_item("ProxmoxRRDTimeframe", &timeframe_str)
                .is_err()
            {
                log::error!("RRDTimeframe::store - set_item failed");
            } else {
                emit_rrd_timeframe_changed_event();
            }
        } else {
            log::error!("RRDTimeframe::store  - no storage");
        }
    }

    pub fn api_params(&self) -> Value {
        json!({ "cf": self.mode, "timeframe": self.timeframe })
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
        "hour-AVERAGE" => "Hour (average)",
        "hour-MAX" => "Hour (maximum)",
        "day-AVERAGE" => "Day (average)",
        "day-MAX" => "Day (maximum)",
        "week-AVERAGE" => "Week (average)",
        "week-MAX" => "Week (maximum)",
        "month-AVERAGE" => "Month (average)",
        "month-MAX" => "Month (maximum)",
        "year-AVERAGE" => "Year (average)",
        "year-MAX" => "Year (maximum)",
        "decade-AVERAGE" => "Decade (average)",
        "decade-MAX" => "Decade (maximum)",
        _ => v,
    }
}

impl Component for PwtRRDTimeframeSelector {
    type Message = Msg;
    type Properties = RRDTimeframeSelector;

    fn create(_ctx: &Context<Self>) -> Self {
        use rrd_types::RrdMode::*;
        use rrd_types::RrdTimeframe::*;

        let timeframe = RRDTimeframe::load();

        let values = [
            RRDTimeframe::new(Hour, Average),
            RRDTimeframe::new(Hour, Max),
            RRDTimeframe::new(Day, Average),
            RRDTimeframe::new(Day, Max),
            RRDTimeframe::new(Week, Average),
            RRDTimeframe::new(Week, Max),
            RRDTimeframe::new(Month, Average),
            RRDTimeframe::new(Month, Max),
            RRDTimeframe::new(Year, Average),
            RRDTimeframe::new(Year, Max),
            RRDTimeframe::new(Decade, Average),
            RRDTimeframe::new(Decade, Max),
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
                if let Ok(timeframe) = timeframe_str.as_str().parse::<RRDTimeframe>() {
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
            .required(true)
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
