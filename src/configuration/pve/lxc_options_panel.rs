use std::rc::Rc;

use serde_json::Value;

use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;

use crate::form::typed_load;
use crate::pending_property_view::{pending_typed_load, PendingPropertyGrid, PendingPropertyList};
use crate::EditableProperty;
use crate::{http_put, percent_encoding::percent_encode_component};

use pve_api_types::LxcConfig;

use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct LxcOptionsPanel {
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

impl LxcOptionsPanel {
    pub fn new(node: impl Into<AttrValue>, vmid: u32) -> Self {
        yew::props!(Self {
            node: node.into(),
            vmid,
        })
    }
}

pub struct PveLxcOptionsPanel {
    properties: Rc<Vec<EditableProperty>>,
}

fn properties(_node: &str, _vmid: u32, mobile: bool) -> Vec<EditableProperty> {
    vec![
        //crate::form::pve::Lxc_name_property(vmid, mobile),
        crate::form::pve::qemu_onboot_property(mobile),
        crate::form::pve::qemu_startup_property(mobile),
        crate::form::pve::lxc_ostype_property(),
        crate::form::pve::lxc_architecture_property(),
        crate::form::pve::lxc_console_property(mobile),
        crate::form::pve::lxc_tty_count_property(mobile),
        crate::form::pve::lxc_console_mode_property(mobile),
        crate::form::pve::qemu_protection_property(mobile),
        crate::form::pve::lxc_unpriviledged_property(),
        crate::form::pve::lxc_features_property(mobile),
        crate::form::pve::lxc_hookscript_property(),
    ]
}

impl Component for PveLxcOptionsPanel {
    type Message = ();
    type Properties = LxcOptionsPanel;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        Self {
            properties: Rc::new(properties(&props.node, props.vmid, props.mobile)),
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let editor_url = if let Some(remote) = &props.remote {
            format!(
                "/pve/remotes/{}/lxc/{}/config?state=pending",
                percent_encode_component(remote),
                props.vmid
            )
        } else {
            format!(
                "/nodes/{}/lxc/{}/config",
                percent_encode_component(&props.node),
                props.vmid
            )
        };

        let pending_url = if let Some(remote) = &props.remote {
            format!(
                "/pve/remotes/{}/lxc/{}/pending",
                percent_encode_component(remote),
                props.vmid
            )
        } else {
            format!(
                "/nodes/{}/lxc/{}/pending",
                percent_encode_component(&props.node),
                props.vmid
            )
        };

        let loader = typed_load::<LxcConfig>(editor_url.clone());

        let on_submit = move |value: Value| {
            let editor_url = editor_url.clone();
            async move { http_put(editor_url.clone(), Some(value.clone())).await }
        };
        if props.mobile {
            PendingPropertyList::new(Rc::clone(&self.properties))
                .class(pwt::css::FlexFit)
                .pending_loader(pending_typed_load::<LxcConfig>(pending_url))
                .editor_loader(loader)
                .on_submit(on_submit)
                .into()
        } else {
            PendingPropertyGrid::new(Rc::clone(&self.properties))
                .class(pwt::css::FlexFit)
                .pending_loader(pending_typed_load::<LxcConfig>(pending_url))
                .editor_loader(loader)
                .on_submit(on_submit)
                .into()
        }
    }
}

impl From<LxcOptionsPanel> for VNode {
    fn from(props: LxcOptionsPanel) -> Self {
        let comp = VComp::new::<PveLxcOptionsPanel>(Rc::new(props), None);
        VNode::from(comp)
    }
}
