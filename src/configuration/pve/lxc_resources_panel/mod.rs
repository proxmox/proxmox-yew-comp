//mod desktop;
mod mobile;

use std::rc::Rc;

use serde_json::Value;
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::props::SubmitCallback;
use pwt_macros::builder;

use crate::pending_property_view::PvePendingPropertyView;
use crate::percent_encoding::percent_encode_component;
use crate::{http_post, http_put};

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

    pub(crate) fn editor_url(&self) -> String {
        if let Some(remote) = &self.remote {
            format!(
                "/pve/remotes/{}/lxc/{}/config?state=pending",
                percent_encode_component(remote),
                self.vmid
            )
        } else {
            format!(
                "/nodes/{}/lxc/{}/config",
                percent_encode_component(&self.node),
                self.vmid
            )
        }
    }

    pub(crate) fn pending_url(&self) -> String {
        if let Some(remote) = &self.remote {
            format!(
                "/pve/remotes/{}/lxc/{}/pending",
                percent_encode_component(remote),
                self.vmid
            )
        } else {
            format!(
                "/nodes/{}/lxc/{}/pending",
                percent_encode_component(&self.node),
                self.vmid
            )
        }
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

impl From<LxcResourcesPanel> for VNode {
    fn from(props: LxcResourcesPanel) -> Self {
        let comp = if props.mobile {
            VComp::new::<PvePendingPropertyView<mobile::PveLxcResourcesPanel>>(Rc::new(props), None)
        } else {
            todo!();
            /*
            VComp::new::<PvePendingPropertyView<desktop::PveLxcResourcesPanel>>(
                Rc::new(props),
                None,
            )
            */
        };
        VNode::from(comp)
    }
}
