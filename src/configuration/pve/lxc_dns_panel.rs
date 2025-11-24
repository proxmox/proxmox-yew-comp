use std::rc::Rc;

use pve_api_types::LxcConfig;
use serde_json::Value;

use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt_macros::builder;

use crate::configuration::{guest_config_url, guest_pending_url};
use crate::form::{pve::PveGuestType, typed_load};
use crate::pending_property_view::{pending_typed_load, PendingPropertyGrid, PendingPropertyList};
use crate::{http_put, EditableProperty};

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct LxcDnsPanel {
    vmid: u32,
    node: AttrValue,

    /// Use Proxmox Datacenter Manager API endpoints
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub remote: Option<AttrValue>,

    /// Layout for mobile devices.
    #[prop_or_default]
    #[builder]
    pub mobile: bool,
}

impl LxcDnsPanel {
    pub fn new(node: impl Into<AttrValue>, vmid: u32) -> Self {
        yew::props!(Self {
            node: node.into(),
            vmid,
        })
    }
}

fn properties(_node: &str, vmid: u32, mobile: bool) -> Vec<EditableProperty> {
    vec![
        crate::form::pve::lxc_hostname_property(vmid, mobile),
        crate::form::pve::lxc_searchdomain_property(mobile),
        crate::form::pve::lxc_nameserver_property(mobile),
    ]
}

pub struct LxcDnsComp {
    properties: Rc<Vec<EditableProperty>>,
}

impl Component for LxcDnsComp {
    type Message = ();
    type Properties = LxcDnsPanel;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        Self {
            properties: Rc::new(properties(&props.node, props.vmid, props.mobile)),
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let editor_url =
            guest_config_url(props.vmid, &props.node, &props.remote, PveGuestType::Lxc);

        let editor_loader = typed_load::<LxcConfig>(editor_url.clone());
        let pending_url =
            guest_pending_url(props.vmid, &props.node, &props.remote, PveGuestType::Lxc);

        let on_submit = move |value: Value| {
            let url = editor_url.clone();
            async move { http_put(url.clone(), Some(value.clone())).await }
        };
        if props.mobile {
            PendingPropertyList::new(Rc::clone(&self.properties))
                .class(pwt::css::FlexFit)
                .editor_loader(editor_loader)
                .pending_loader(pending_typed_load::<LxcConfig>(pending_url))
                .on_submit(on_submit)
                .into()
        } else {
            PendingPropertyGrid::new(Rc::clone(&self.properties))
                .class(pwt::css::FlexFit)
                .editor_loader(editor_loader)
                .pending_loader(pending_typed_load::<LxcConfig>(pending_url))
                .on_submit(on_submit)
                .into()
        }
    }
}

impl From<LxcDnsPanel> for VNode {
    fn from(props: LxcDnsPanel) -> Self {
        let comp = VComp::new::<LxcDnsComp>(Rc::new(props), None);
        VNode::from(comp)
    }
}
