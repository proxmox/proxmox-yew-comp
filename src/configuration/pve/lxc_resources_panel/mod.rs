mod desktop;
mod mobile;

mod reassign_volume_dialog;
pub use reassign_volume_dialog::lxc_reassign_volume_dialog;

use std::rc::Rc;

use serde_json::Value;
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pve_api_types::LxcConfig;

use pwt::prelude::*;
use pwt::props::SubmitCallback;
use pwt_macros::builder;

use crate::configuration::pve::{move_disk_dialog, resize_disk_dialog};
use crate::configuration::{guest_config_url, guest_move_volume_url, guest_resize_disk_url};
use crate::form::pve::PveGuestType;
use crate::form::typed_load;
use crate::pending_property_view::{PvePendingConfiguration, PvePendingPropertyView};
use crate::{http_post, http_put, PropertyEditDialog};

pub enum Msg {
    ResizeDisk(String),
    ReassignDisk(String),
    MoveDisk(String),
}

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct LxcResourcesPanel {
    vmid: u32,
    node: AttrValue,

    /// Use Proxmox Datacenter Manager API endpoints
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub remote: Option<AttrValue>,

    /// This callback is called after starting a task on the backend.
    ///
    /// The UPID is passed as argument to the callback.
    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    #[prop_or_default]
    on_start_command: Option<Callback<String>>,

    /// Layout for mobile devices.
    #[prop_or_default]
    #[builder]
    pub mobile: bool,

    /// Read-only view - hide toolbar and all buttons/menus to edit content.
    #[prop_or_default]
    #[builder]
    pub readonly: bool,
}

impl LxcResourcesPanel {
    pub fn new(node: impl Into<AttrValue>, vmid: u32) -> Self {
        yew::props!(Self {
            node: node.into(),
            vmid,
        })
    }

    pub(crate) fn move_volume_dialog(&self, name: &str) -> PropertyEditDialog {
        let editor_url = guest_config_url(self.vmid, &self.node, &self.remote, PveGuestType::Lxc);
        let move_volume_url =
            guest_move_volume_url(self.vmid, &self.node, &self.remote, PveGuestType::Lxc);
        move_disk_dialog(
            name,
            Some(self.node.clone()),
            self.remote.clone(),
            PveGuestType::Lxc,
            self.mobile,
        )
        .loader(typed_load::<LxcConfig>(editor_url))
        .on_submit(create_on_submit(
            move_volume_url,
            self.on_start_command.clone(),
            true,
            0,
        ))
    }

    pub(crate) fn resize_disk_dialog(&self, name: &str) -> PropertyEditDialog {
        let editor_url = guest_config_url(self.vmid, &self.node, &self.remote, PveGuestType::Lxc);
        let resize_disk_url =
            guest_resize_disk_url(self.vmid, &self.node, &self.remote, PveGuestType::Lxc);
        resize_disk_dialog(
            name,
            Some(self.node.clone()),
            self.remote.clone(),
            self.mobile,
        )
        .loader(typed_load::<LxcConfig>(editor_url))
        .on_submit(create_on_submit(
            resize_disk_url,
            self.on_start_command.clone(),
            false,
            0,
        ))
    }

    pub(crate) fn reassign_volume_dialog(&self, name: &str) -> PropertyEditDialog {
        let editor_url = guest_config_url(self.vmid, &self.node, &self.remote, PveGuestType::Lxc);
        let move_volume_url =
            guest_move_volume_url(self.vmid, &self.node, &self.remote, PveGuestType::Lxc);
        lxc_reassign_volume_dialog(
            name,
            Some(self.node.clone()),
            self.vmid,
            self.remote.clone(),
            self.mobile,
        )
        .loader(typed_load::<LxcConfig>(editor_url))
        .on_submit(create_on_submit(
            move_volume_url,
            self.on_start_command.clone(),
            true,
            0,
        ))
    }
}

#[derive(Copy, Clone, PartialEq)]
enum EditAction {
    //    None,
    Edit,
    Add,
}

fn create_on_submit(
    submit_url: String,
    on_start_command: Option<Callback<String>>,
    post: bool,              // PUT or POST
    background_delay: usize, // add background_delay parameter
) -> SubmitCallback<Value> {
    SubmitCallback::new(move |mut data: Value| {
        let submit_url = submit_url.clone();
        let on_start_command = on_start_command.clone();
        if background_delay > 0 {
            data["background_delay"] = background_delay.into();
        }
        async move {
            let result: Option<String> = if post {
                http_post(&submit_url, Some(data)).await?
            } else {
                http_put(&submit_url, Some(data)).await?
            };
            if let Some(upid) = result {
                if let Some(on_start_command) = &on_start_command {
                    on_start_command.emit(upid.clone());
                }
            }
            Ok(())
        }
    })
}

fn is_unprivileged(data: &PvePendingConfiguration) -> bool {
    let PvePendingConfiguration {
        current: _,
        pending,
        keys: _,
    } = data;

    match pending["unprivileged"] {
        Value::Bool(unprivileged) => unprivileged,
        _ => false,
    }
}

impl From<LxcResourcesPanel> for VNode {
    fn from(props: LxcResourcesPanel) -> Self {
        let comp = if props.mobile {
            VComp::new::<PvePendingPropertyView<mobile::PveLxcResourcesPanel>>(Rc::new(props), None)
        } else {
            VComp::new::<PvePendingPropertyView<desktop::PveLxcResourcesPanel>>(
                Rc::new(props),
                None,
            )
        };
        VNode::from(comp)
    }
}
