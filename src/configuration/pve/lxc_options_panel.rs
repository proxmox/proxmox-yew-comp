use std::rc::Rc;

use pwt::props::SubmitCallback;
use serde_json::Value;

use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;

use crate::configuration::{guest_config_url, guest_pending_url};
use crate::form::pve::PveGuestType;
use crate::form::typed_load;
use crate::http_put;
use crate::pending_property_view::{pending_typed_load, PendingPropertyGrid, PendingPropertyList};
use crate::EditableProperty;

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

    /// Read-only view - hide toolbar and all buttons/menus to edit content.
    #[prop_or_default]
    #[builder]
    pub readonly: bool,
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

        let editor_url =
            guest_config_url(props.vmid, &props.node, &props.remote, PveGuestType::Lxc);

        let pending_url =
            guest_pending_url(props.vmid, &props.node, &props.remote, PveGuestType::Lxc);

        let loader = typed_load::<LxcConfig>(editor_url.clone());

        let on_submit = (!props.readonly).then(|| {
            SubmitCallback::new(move |value: Value| {
                let editor_url = editor_url.clone();
                async move { http_put(editor_url.clone(), Some(value.clone())).await }
            })
        });
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
