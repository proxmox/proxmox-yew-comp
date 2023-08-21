use std::rc::Rc;
use anyhow::{bail, Error};

use yew::prelude::*;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::state::local_storage;
use pwt::widget::form::Combobox;

/// Combobox for selecting the theme density.
#[derive(Clone, PartialEq, Properties)]
pub struct RRDTimeframeSelector {
    #[prop_or_default]
    class: Classes,
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
    Hour,
    #[default]
    Day,
    Week,
    Month,
    Year,
    Decade,
}

impl std::fmt::Display for RRDTimeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str(match self {
            RRDTimeframe::Hour => "hour",
            RRDTimeframe::Day => "day",
            RRDTimeframe::Week => "week",
            RRDTimeframe::Month => "month",
            RRDTimeframe::Year => "year",
            RRDTimeframe::Decade => "decade",
         })
    }
}

impl TryFrom<&str> for RRDTimeframe {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "hour" => RRDTimeframe::Hour,
            "day" => RRDTimeframe::Day,
            "week" => RRDTimeframe::Week,
            "month" => RRDTimeframe::Month,
            "year" => RRDTimeframe::Year,
            "decade" => RRDTimeframe::Decade,
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

        if let Ok(Some(timeframe)) = store.get_item("ProxmoxRRDTimeframe") {
            if let Ok(timeframe) = RRDTimeframe::try_from(timeframe.as_str()) {
                return timeframe;
            }
        }

        timeframe
    }

    pub fn store(&self) {
        if let Some(store) = local_storage() {
            if let Err(_) = store.set_item("ProxmoxRRDTimeframe", &self.to_string()) {
                log::error!("RRDTimeframe::store - set_item failed");
            } else {
                emit_rrd_timeframe_changed_event();
            }
        } else {
            log::error!("RRDTimeframe::store  - no storage");
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
                AttrValue::from(RRDTimeframe::Hour.to_string()),
                AttrValue::from(RRDTimeframe::Day.to_string()),
                AttrValue::from(RRDTimeframe::Week.to_string()),
                AttrValue::from(RRDTimeframe::Month.to_string()),
                AttrValue::from(RRDTimeframe::Year.to_string()),
                AttrValue::from(RRDTimeframe::Decade.to_string()),
            ]),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SetRRDTimeframe(timeframe) => {
                if let Ok(timeframe) = RRDTimeframe::try_from(timeframe) {
                    timeframe.store();
                    self.timeframe = timeframe;
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
