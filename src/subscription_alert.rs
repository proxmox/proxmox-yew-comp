use std::rc::Rc;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::AlertDialog;

use pwt_macros::builder;

use crate::subscription_info::{subscription_note, subscription_status_text};

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
        let msg = subscription_note(props.url.as_deref());

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
