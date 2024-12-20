use std::rc::Rc;

use pwt::css::{AlignItems, Flex, FontColor, JustifyContent};
use serde_json::Value;

use yew::html::IntoEventCallback;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::state::Loader;
use pwt::widget::{Column, Fa, Panel, Row};

use pwt_macros::builder;

use crate::{HelpButton, ProjectInfo};

pub fn subscription_status_text(status: &str) -> String {
    match status {
        "new" => tr!("Newly set subscription, not yet checked"),
        "notfound" => tr!("No valid subscription"),
        "active" => tr!("Your subscription status is valid."),
        "invalid" => tr!("Subscription set but invalid for this server."),
        "expired" => tr!("Subscription set but expired for this server."),
        "suspended" => tr!("Subscription got (recently) suspended"),
        _ => tr!("Unable to get the subscription status (API problems)."),
    }
}

pub fn subscription_note(url: Option<&str>) -> Html {
    let msg = tr!(
        "You do not have a valid subscription for this server. Please visit <a target=\"_blank\" href=\"{}\">www.proxmox.com</a> to get
a list of available options. ",
        url.unwrap_or("https://www.proxmox.com")
    );

    let msg = Html::from_html_unchecked(msg.into());

    html! {<p>{msg}</p>}
}

pub fn subscription_status_message(status: &str, url: Option<&str>) -> Html {
    let status_text = subscription_status_text(status);

    match status {
        "new" | "active" => Row::new().with_child(status_text).into(),
        _ => Column::new()
            .class(JustifyContent::Center)
            .class(AlignItems::Center)
            .class(Flex::Fill)
            .with_child(html! {<h3>{status_text}</h3>})
            .with_child(subscription_note(url))
            .into(),
    }
}

/// Returns a fitting status icon for the various states the subscription can be in
pub fn subscription_icon(status: &str) -> Fa {
    let (icon, color) = match status {
        "new" | "active" => ("check", FontColor::Success),
        "notfound" => ("times-circle", FontColor::Error),
        _ => ("exclamation-triangle", FontColor::Warning),
    };

    Fa::new(icon).class(color)
}

#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct SubscriptionInfo {
    pub project: AttrValue,
    pub short_name: AttrValue,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    pub on_status_change: Option<Callback<String>>,
}

impl SubscriptionInfo {
    pub fn new(product: &dyn ProjectInfo) -> Self {
        yew::props!(Self {
            project: product.project_text(),
            short_name: product.short_name()
        })
    }
}

pub enum Msg {
    DataChange,
}

pub struct ProxmoxSubscriptionInfo {
    loader: Loader<Value>,
}

impl Component for ProxmoxSubscriptionInfo {
    type Message = Msg;
    type Properties = SubscriptionInfo;

    fn create(ctx: &Context<Self>) -> Self {
        let loader = Loader::new()
            .loader("/nodes/localhost/subscription")
            .on_change(ctx.link().callback(|_| Msg::DataChange));

        loader.load();

        Self { loader }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::DataChange => {
                let status = match &self.loader.read().data {
                    Some(Ok(data)) => data["status"].as_str().unwrap_or("").to_owned(),
                    _ => String::from("unknown"),
                };
                if let Some(on_status_change) = &props.on_status_change {
                    on_status_change.emit(status);
                }
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let main_view = self.loader.render(|data| {
            let status = data["status"].as_str().unwrap_or("").to_owned();
            let url = data["url"].as_str();
            let msg = subscription_status_message(&status, url);
            Row::new()
                .padding(2)
                .class(Flex::Fill)
                .class(AlignItems::Center)
                .with_child(subscription_icon(&status).large_3x().padding(6))
                .with_child(msg)
        });

        Panel::new()
            .border(true)
            .min_height(200)
            .title("Subscription")
            .with_tool(HelpButton::new().section("subscription"))
            .with_child(main_view)
            .into()
    }
}

impl From<SubscriptionInfo> for VNode {
    fn from(val: SubscriptionInfo) -> Self {
        let comp = VComp::new::<ProxmoxSubscriptionInfo>(Rc::new(val), None);
        VNode::from(comp)
    }
}
