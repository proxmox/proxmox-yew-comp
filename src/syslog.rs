use std::rc::Rc;

use pwt::widget::form::InputType;
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
    Since(String),
    Until(String),
}

pub struct ProxmoxSyslog {
    active: bool,
    since: js_sys::Date,
    since_label_id: AttrValue,
    until: js_sys::Date,
    until_label_id: AttrValue,
    pending: bool,
}

fn date_to_input_value(date: &js_sys::Date) -> String {
    if date.get_date() == 0 {
        // invalid data (clear field creates this)
        String::new()
    } else {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}",
            date.get_full_year(),
            date.get_month() + 1,
            date.get_date(),
            date.get_hours(),
            date.get_minutes(),
        )
    }
}

impl ProxmoxSyslog {
    fn create_toolbar(&self, ctx: &Context<Self>) -> Html {
        let since = date_to_input_value(&self.since);
        let until = date_to_input_value(&self.until);

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
                Field::new()
                    .label_id(self.since_label_id.clone())
                    .input_type(InputType::DatetimeLocal)
                    .required(true) // avoid clear button in firefox
                    .disabled(self.active)
                    .class("pwt-input-hide-clear-button")
                    .on_change(ctx.link().callback(Msg::Since))
                    .value(since),
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
                Field::new()
                    .label_id(self.until_label_id.clone())
                    .input_type(InputType::DatetimeLocal)
                    .required(true) // avoid clear button in firefox
                    .disabled(self.active)
                    .on_change(ctx.link().callback(Msg::Until))
                    .value(until),
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
            let since = if self.since.get_date() == 0 {
                // invalid data (clear field creates this)
                get_default_since()
            } else {
                self.since.clone()
            };

            let until = if self.until.get_date() == 0 {
                // invalid data (clear field creates this)
                None
            } else {
                Some((self.until.get_time() / 1000.0) as i64)
            };

            LogView::new(props.base_url.clone())
                .padding(2)
                .class("pwt-flex-fill")
                .service(props.service.clone())
                .since((since.get_time() / 1000.0) as i64)
                .until(until)
                .active(false)
                .on_pending_change(ctx.link().callback(Msg::LoadingChange))
                .into()
        }
    }
}

fn get_default_since() -> js_sys::Date {
    let since = js_sys::Date::new_0();

    since.set_hours(0);
    since.set_minutes(0);
    since.set_seconds(0);
    since.set_milliseconds(0);

    since
}

fn get_default_until() -> js_sys::Date {
    let until = js_sys::Date::new_0();

    until.set_hours(23);
    until.set_minutes(59);
    until.set_seconds(59);
    until.set_milliseconds(999);

    until
}
impl Component for ProxmoxSyslog {
    type Message = Msg;
    type Properties = Syslog;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            active: true,
            since: get_default_since(),
            since_label_id: AttrValue::from(pwt::widget::get_unique_element_id()),
            until: get_default_until(),
            until_label_id: AttrValue::from(pwt::widget::get_unique_element_id()),
            pending: false,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Since(datetime) => {
                let since = js_sys::Date::parse(&datetime);
                let since = js_sys::Date::new(&since.into());
                self.since = since;
                true
            }
            Msg::Until(datetime) => {
                let until = js_sys::Date::parse(&datetime);
                let until = js_sys::Date::new(&until.into());
                self.until = until;
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
