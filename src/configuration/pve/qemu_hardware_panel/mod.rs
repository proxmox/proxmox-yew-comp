mod move_disk_dialog;
pub use move_disk_dialog::qemu_move_disk_dialog;

mod reassign_disk_dialog;
pub use reassign_disk_dialog::qemu_reassign_disk_dialog;

mod resize_disk_dialog;
pub use resize_disk_dialog::qemu_resize_disk_dialog;
use serde_json::Value;

mod desktop;
mod mobile;

use std::rc::Rc;

use pve_api_types::QemuConfig;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::props::SubmitCallback;
use pwt_macros::builder;

use crate::form::typed_load;
use crate::pending_property_view::PvePendingPropertyView;
use crate::percent_encoding::percent_encode_component;
use crate::PropertyEditDialog;
use crate::{http_post, http_put};

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct QemuHardwarePanel {
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

impl QemuHardwarePanel {
    pub fn new(node: impl Into<AttrValue>, vmid: u32) -> Self {
        yew::props!(Self {
            node: node.into(),
            vmid,
        })
    }

    pub(crate) fn editor_url(&self) -> String {
        if let Some(remote) = &self.remote {
            format!(
                "/pve/remotes/{}/qemu/{}/config?state=pending",
                percent_encode_component(remote),
                self.vmid
            )
        } else {
            format!(
                "/nodes/{}/qemu/{}/config",
                percent_encode_component(&self.node),
                self.vmid
            )
        }
    }

    pub(crate) fn pending_url(&self) -> String {
        if let Some(remote) = &self.remote {
            format!(
                "/pve/remotes/{}/qemu/{}/pending",
                percent_encode_component(remote),
                self.vmid
            )
        } else {
            format!(
                "/nodes/{}/qemu/{}/pending",
                percent_encode_component(&self.node),
                self.vmid
            )
        }
    }

    pub(crate) fn resize_disk_url(&self) -> String {
        if let Some(remote) = &self.remote {
            format!(
                "/pve/remotes/{}/qemu/{}/resize",
                percent_encode_component(remote),
                self.vmid
            )
        } else {
            format!(
                "/nodes/{}/qemu/{}/resize",
                percent_encode_component(&self.node),
                self.vmid
            )
        }
    }

    pub(crate) fn move_disk_url(&self) -> String {
        let name = if self.remote.is_some() {
            "move-disk"
        } else {
            "move_disk"
        };
        if let Some(remote) = &self.remote {
            format!(
                "/pve/remotes/{}/qemu/{}/{name}",
                percent_encode_component(remote),
                self.vmid
            )
        } else {
            format!(
                "/nodes/{}/qemu/{}/{name}",
                percent_encode_component(&self.node),
                self.vmid
            )
        }
    }

    pub(crate) fn resize_disk_dialog(&self, name: &str) -> PropertyEditDialog {
        qemu_resize_disk_dialog(
            name,
            Some(self.node.clone()),
            self.remote.clone(),
            self.mobile,
        )
        .loader(typed_load::<QemuConfig>(self.editor_url()))
        .on_submit(create_on_submit(
            self.resize_disk_url(),
            self.on_start_command.clone(),
            false,
            0,
        ))
        .into()
    }

    pub(crate) fn reassign_disk_dialog(&self, name: &str) -> PropertyEditDialog {
        qemu_reassign_disk_dialog(
            &name,
            Some(self.node.clone()),
            self.remote.clone(),
            self.mobile,
        )
        .loader(typed_load::<QemuConfig>(self.editor_url()))
        .on_submit(create_on_submit(
            self.move_disk_url(),
            self.on_start_command.clone(),
            true,
            0,
        ))
    }

    pub(crate) fn move_disk_dialog(&self, name: &str) -> PropertyEditDialog {
        qemu_move_disk_dialog(
            &name,
            Some(self.node.clone()),
            self.remote.clone(),
            self.mobile,
        )
        .loader(typed_load::<QemuConfig>(self.editor_url()))
        .on_submit(create_on_submit(
            self.move_disk_url(),
            self.on_start_command.clone(),
            true,
            0,
        ))
    }
}

#[derive(Copy, Clone, PartialEq)]
enum EditAction {
    None,
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

impl From<QemuHardwarePanel> for VNode {
    fn from(props: QemuHardwarePanel) -> Self {
        let comp = if props.mobile {
            VComp::new::<PvePendingPropertyView<mobile::PveQemuHardwarePanel>>(Rc::new(props), None)
        } else {
            VComp::new::<PvePendingPropertyView<desktop::PveQemuHardwarePanel>>(
                Rc::new(props),
                None,
            )
        };
        VNode::from(comp)
    }
}
