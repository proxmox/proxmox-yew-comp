use std::rc::Rc;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::AlertDialog;

use pwt_macros::builder;

fn subscription_status_text(status: &str) -> String {
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

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct SubscriptionAlert {
    /// Close callback, clalled when user confirms or press dialog close.
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    pub on_close: Option<Callback<()>>,

    /// Subscription status.
    pub subscription_status: AttrValue,

    /// Option URL to proxmox web site.
    #[builder(IntoPropValue, into_prop_value)]
    pub url: Option<AttrValue>,
}

impl SubscriptionAlert {
    pub fn new(status: impl IntoPropValue<AttrValue>) -> Self {
        yew::props!(Self {
            subscription_status: status.into_prop_value()
        })
    }
}

#[doc(hidden)]
pub struct ProxmoxSubscriptionAlert {}

impl Component for ProxmoxSubscriptionAlert {
    type Message = ();
    type Properties = SubscriptionAlert;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let title = subscription_status_text(props.subscription_status.as_str());

        let msg2 = String::from("<p>")
            + &tr!(
                "
        Please visit <a target=\"_blank\" href=\"{}\">www.proxmox.com</a> to get
        a list of available options.
        ",
                props.url.as_deref().unwrap_or("https://www.proxmox.com")
            )
            + "</p>";

        let msg2 = Html::from_html_unchecked(msg2.into());

        let msg = html! {<><p class="pwt-pb-2">{tr!("
        The Proxmox team works very hard to make sure you are running the best
        software and getting stable updates and security enhancements,
        as well as quick enterprise support.
        ")}</p>{msg2}</>};

        let on_close = props.on_close.clone();
        AlertDialog::new(msg).title(title).on_close(on_close).into()
    }
}

impl Into<VNode> for SubscriptionAlert {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxSubscriptionAlert>(Rc::new(self), None);
        VNode::from(comp)
    }
}
