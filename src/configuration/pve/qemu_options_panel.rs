use std::rc::Rc;

use serde_json::Value;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;

use crate::form::typed_load;
use crate::pending_property_view::{pending_typed_load, PendingPropertyGrid, PendingPropertyList};
use crate::EditableProperty;
use crate::{http_post, percent_encoding::percent_encode_component};

use pve_api_types::QemuConfig;

use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct QemuOptionsPanel {
    vmid: u32,
    node: AttrValue,

    /// This callback is called after starting a task on the backend.
    ///
    /// The UPID is passed as argument to the callback.
    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    on_start_command: Option<Callback<String>>,

    /// Use Proxmox Datacenter Manager API endpoints
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub remote: Option<AttrValue>,

    /// Layout for mobile devices.
    #[prop_or_default]
    #[builder]
    pub mobile: bool,
}

impl QemuOptionsPanel {
    pub fn new(node: impl Into<AttrValue>, vmid: u32) -> Self {
        yew::props!(Self {
            node: node.into(),
            vmid,
        })
    }
}

pub struct PveQemuOptionsPanel {
    properties: Rc<Vec<EditableProperty>>,
}

fn properties(node: &str, vmid: u32, mobile: bool) -> Vec<EditableProperty> {
    vec![
        crate::form::pve::qemu_name_property(vmid, mobile),
        crate::form::pve::qemu_onboot_property(mobile),
        crate::form::pve::qemu_startup_property(mobile),
        crate::form::pve::qemu_ostype_property(mobile),
        crate::form::pve::qemu_boot_property(mobile),
        crate::form::pve::qemu_tablet_property(mobile),
        crate::form::pve::qemu_hotplug_property(mobile),
        crate::form::pve::qemu_acpi_property(mobile),
        crate::form::pve::qemu_kvm_property(mobile),
        crate::form::pve::qemu_freeze_property(mobile),
        crate::form::pve::qemu_localtime_property(mobile),
        crate::form::pve::qemu_startdate_property(mobile),
        crate::form::pve::qemu_smbios_property(mobile),
        crate::form::pve::qemu_agent_property(mobile),
        crate::form::pve::qemu_protection_property(mobile),
        crate::form::pve::qemu_spice_enhancement_property(mobile),
        crate::form::pve::qemu_vmstatestorage_property(node, mobile),
        crate::form::pve::qemu_amd_sev_property(mobile),
    ]
}

impl Component for PveQemuOptionsPanel {
    type Message = ();
    type Properties = QemuOptionsPanel;

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
                "/pve/remotes/{}/qemu/{}/config?state=pending",
                percent_encode_component(remote),
                props.vmid
            )
        } else {
            format!(
                "/nodes/{}/qemu/{}/config",
                percent_encode_component(&props.node),
                props.vmid
            )
        };

        let pending_url = if let Some(remote) = &props.remote {
            format!(
                "/pve/remotes/{}/qemu/{}/pending",
                percent_encode_component(remote),
                props.vmid
            )
        } else {
            format!(
                "/nodes/{}/qemu/{}/pending",
                percent_encode_component(&props.node),
                props.vmid
            )
        };

        let loader = typed_load::<QemuConfig>(editor_url.clone());

        let on_start_command = props.on_start_command.clone();
        let on_submit = move |mut value: Value| {
            let editor_url = editor_url.clone();
            let on_start_command = on_start_command.clone();
            async move {
                value["background_delay"] = 10.into();
                let result: Option<String> =
                    http_post(editor_url.clone(), Some(value.clone())).await?;
                if let Some(upid) = result {
                    if let Some(on_start_command) = &on_start_command {
                        on_start_command.emit(upid.clone());
                    }
                }
                Ok(())
            }
        };
        if props.mobile {
            PendingPropertyList::new(Rc::clone(&self.properties))
                .class(pwt::css::FlexFit)
                .pending_loader(pending_typed_load::<QemuConfig>(pending_url))
                .editor_loader(loader)
                .on_submit(on_submit)
                .into()
        } else {
            PendingPropertyGrid::new(Rc::clone(&self.properties))
                .class(pwt::css::FlexFit)
                .pending_loader(pending_typed_load::<QemuConfig>(pending_url))
                .editor_loader(loader)
                .on_submit(on_submit)
                .into()
        }
    }
}

impl From<QemuOptionsPanel> for VNode {
    fn from(props: QemuOptionsPanel) -> Self {
        let comp = VComp::new::<PveQemuOptionsPanel>(Rc::new(props), None);
        VNode::from(comp)
    }
}
