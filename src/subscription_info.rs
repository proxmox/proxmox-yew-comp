use std::rc::Rc;

use serde_json::Value;

use yew::virtual_dom::{VComp, VNode};
use yew::html::IntoEventCallback;

use pwt::prelude::*;
use pwt::state::Loader;
use pwt::widget::Panel;

use pwt_macros::builder;

use crate::{HelpButton, ProxmoxProduct};

pub fn subscription_status_text(status: &str) -> String {
    match status {
        "new" => tr!("Newly set subscription, not yet checked"),
        "notfound" => tr!("You do not have a valid subscription."),
        "active" => tr!("Subscription set and active."),
        "invalid" => tr!("Subscription set but invalid for this server."),
        "expired" => tr!("Subscription set but expired for this server."),
        "suspended" => tr!("Subscription got (recently) suspended"),
        _ => tr!("Unable to get the subscription status (API problems)."),
    }
}

pub fn subscription_note(url: Option<&str>) -> Html {
    let msg2 = String::from("<p>")
        + &tr!(
            "
Please visit <a target=\"_blank\" href=\"{}\">www.proxmox.com</a> to get
a list of available options.
",
            url.unwrap_or("https://www.proxmox.com")
        )
        + "</p>";

    let msg2 = Html::from_html_unchecked(msg2.into());

    let msg = html! {<><p class="pwt-pb-2">{tr!("
The Proxmox team works very hard to make sure you are running the best
software and getting stable updates and security enhancements,
as well as quick enterprise support.
")}</p>{msg2}</>};

    msg
}

fn subscription_status_message(status: &str, url: Option<&str>) -> Html {
    let status_text = subscription_status_text(status);
    if matches!(status, "new" | "active") {
        return html! {<p>{status_text}</p>};
    }

    let msg = html!{
        <>
            <h1>{status_text}</h1>
            {subscription_note(url)}
        </>
    };

    msg
}

#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct SubscriptionInfo {
    pub product: ProxmoxProduct,

    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    pub on_status_change: Option<Callback<String>>,
}

impl SubscriptionInfo {
    pub fn new(product: ProxmoxProduct) -> Self {
        yew::props!(Self { product })
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
        let loader = Loader::new(ctx.link().callback(|_| Msg::DataChange))
            .loader("/nodes/localhost/subscription");

        loader.load();

        Self { loader }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::DataChange => {
                self.loader.with_state(|state| {
                    let status = match &state.data {
                        Some(Ok(data)) => {
                            data["status"].as_str().unwrap_or("").to_owned()
                        }
                        _ => String::from("unknown"),
                    };
                    if let Some(on_status_change) = &props.on_status_change {
                        on_status_change.emit(status);
                    }
                });
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let main_view = self.loader.render(|data| {
            let status = data["status"].as_str().unwrap_or("").to_owned();
            let url = data["url"].as_str();
            let msg = subscription_status_message(&status, url);
            html! {<div class="pwt-p-2">{msg}</div>}
        });


        Panel::new()
            .border(true)
            .title("Subscription")
            .with_tool(HelpButton::new().section("subscription"))
            .with_child(main_view)
            .into()
    }
}

impl Into<VNode> for SubscriptionInfo {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxSubscriptionInfo>(Rc::new(self), None);
        VNode::from(comp)
    }
}
