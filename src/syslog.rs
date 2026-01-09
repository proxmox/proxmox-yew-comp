use std::rc::Rc;

use pwt::widget::form::{DateField, InputType, PlainDate};
use pwt::widget::{Button, Container, Row, SegmentedButton};
use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::{form::Field, Column, Toolbar};

use crate::{JournalView, LogView};

use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct Syslog {
    /// Base URL for the syslog API
    #[prop_or("/nodes/localhost/syslog".into())]
    #[builder(IntoPropValue, into_prop_value)]
    pub base_url: AttrValue,

    /// Base URL for the journal API
    #[prop_or("/nodes/localhost/journal".into())]
    #[builder(IntoPropValue, into_prop_value)]
    pub journal_base_url: AttrValue,

    /// Show logs for specified service.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub service: Option<AttrValue>,
}

impl Default for Syslog {
    fn default() -> Self {
        Self::new()
    }
}

impl Syslog {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub enum Msg {
    ChangeMode(bool),
    LoadingChange((usize, bool)),
    SinceDate(Option<PlainDate>),
    SinceTime(String),
    UntilDate(Option<PlainDate>),
    UntilTime(String),
}

pub struct ProxmoxSyslog {
    active: bool,
    since: PlainDate,
    since_time: String,
    since_label_id: AttrValue,
    until: PlainDate,
    until_time: String,
    until_label_id: AttrValue,
    pending: bool,
}

fn date_time_to_epoch(date: &PlainDate, time: &str) -> Option<i64> {
    let (hours, minutes) = time.split_once(':')?;
    let d = date.to_date();
    d.set_hours(hours.parse().ok()?);
    d.set_minutes(minutes.parse().ok()?);
    let res = d.get_time() / 1000.0;
    res.is_finite().then_some(res.round() as i64)
}

impl ProxmoxSyslog {
    fn create_toolbar(&self, ctx: &Context<Self>) -> Html {
        Toolbar::new()
            .with_optional_child(
                self.pending.then_some(
                    Row::new()
                        .gap(2)
                        .with_child(
                            Container::from_tag("i")
                                .class("pwt-loading-icon")
                                .padding(2),
                        )
                        .with_child(tr!("Loading...")),
                ),
            )
            .with_flex_spacer()
            .with_child(
                SegmentedButton::new()
                    .with_button(
                        Button::new(tr!("Live Mode"))
                            .pressed(self.active)
                            .onclick(ctx.link().callback(|_| Msg::ChangeMode(true))),
                    )
                    .with_button(
                        Button::new(tr!("Select Timespan"))
                            .pressed(!self.active)
                            .onclick(ctx.link().callback(|_| Msg::ChangeMode(false))),
                    ),
            )
            .with_child(
                Container::from_tag("label")
                    .id(self.since_label_id.clone())
                    .padding_start(2)
                    .class("pwt-align-self-center")
                    .class(self.active.then_some("pwt-label-disabled"))
                    .with_child("Since:"),
            )
            .with_child(
                Container::new().with_child(
                    DateField::new()
                        .label_id(self.since_label_id.clone())
                        .required(true)
                        .disabled(self.active)
                        .on_change(ctx.link().callback(Msg::SinceDate))
                        .value(self.since.to_string()),
                ),
            )
            .with_child(
                Container::new().with_child(
                    Field::new()
                        .input_type(InputType::Time)
                        .required(true) // avoid clear button in firefox
                        .disabled(self.active)
                        .class("pwt-input-hide-clear-button")
                        .on_change(ctx.link().callback(Msg::SinceTime))
                        .value(self.since_time.to_string()),
                ),
            )
            .with_child(
                Container::from_tag("label")
                    .id(self.since_label_id.clone())
                    .padding_start(2)
                    .class("pwt-align-self-center")
                    .class(self.active.then_some("pwt-label-disabled"))
                    .with_child("Until:"),
            )
            .with_child(
                Container::new().with_child(
                    DateField::new()
                        .label_id(self.until_label_id.clone())
                        .required(true) // avoid clear button in firefox
                        .disabled(self.active)
                        .on_change(ctx.link().callback(Msg::UntilDate))
                        .value(self.until.to_string()),
                ),
            )
            .with_child(
                Container::new().with_child(
                    Field::new()
                        .label_id(self.until_label_id.clone())
                        .input_type(InputType::Time)
                        .required(true) // avoid clear button in firefox
                        .disabled(self.active)
                        .on_change(ctx.link().callback(Msg::UntilTime))
                        .value(self.until_time.to_string()),
                ),
            )
            .border_bottom(true)
            .into()
    }

    fn create_log_view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        if self.active {
            JournalView::new(props.journal_base_url.clone())
                .on_loading_change(ctx.link().callback(|(loading, tailview)| {
                    Msg::LoadingChange((if loading { 1 } else { 0 }, tailview))
                }))
                .into()
        } else {
            LogView::new(props.base_url.clone())
                .padding(2)
                .class("pwt-flex-fill")
                .service(props.service.clone())
                .since(date_time_to_epoch(&self.since, &self.since_time))
                .until(date_time_to_epoch(&self.until, &self.until_time))
                .active(false)
                .on_pending_change(ctx.link().callback(Msg::LoadingChange))
                .into()
        }
    }
}

impl Component for ProxmoxSyslog {
    type Message = Msg;
    type Properties = Syslog;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            active: true,
            since: PlainDate::today(),
            since_time: "00:00".to_string(),
            since_label_id: AttrValue::from(pwt::widget::get_unique_element_id()),
            until: PlainDate::today(),
            until_time: "23:59".to_string(),
            until_label_id: AttrValue::from(pwt::widget::get_unique_element_id()),
            pending: false,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SinceDate(Some(date)) => {
                self.since = date;
                true
            }
            Msg::SinceDate(None) => false,
            Msg::SinceTime(time) => {
                self.since_time = time;
                true
            }
            Msg::UntilDate(Some(date)) => {
                self.until = date;
                true
            }
            Msg::UntilDate(None) => false,
            Msg::UntilTime(time) => {
                self.until_time = time;
                true
            }
            Msg::LoadingChange((num, tail_view)) => {
                let new_pending = num > 0 && !tail_view;
                let changed = new_pending != self.pending;
                self.pending = new_pending;
                changed
            }
            Msg::ChangeMode(active) => {
                let changed = active != self.active;
                self.active = active;
                changed
            }
        }
    }
    fn view(&self, ctx: &Context<Self>) -> Html {
        Column::new()
            .class("pwt-flex-fill pwt-overflow-auto")
            .with_child(self.create_toolbar(ctx))
            .with_child(self.create_log_view(ctx))
            .into()
    }
}

impl From<Syslog> for VNode {
    fn from(val: Syslog) -> Self {
        let comp = VComp::new::<ProxmoxSyslog>(Rc::new(val), None);
        VNode::from(comp)
    }
}
