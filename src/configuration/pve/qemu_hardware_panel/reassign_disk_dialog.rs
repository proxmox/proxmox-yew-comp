use std::collections::HashSet;
use std::rc::Rc;

use anyhow::Error;
use serde_json::Value;
use yew::virtual_dom::VComp;

use pve_api_types::ClusterResource;

use pwt::widget::{Column, InputPanel};
use pwt::{prelude::*, AsyncAbortGuard};

use crate::form::pve::{
    extract_used_devices, PveGuestSelector, PveGuestType, QemuControllerSelector,
};
use crate::http_get;
use crate::layout::mobile_form::label_field;
use crate::{PropertyEditDialog, PropertyEditorState};

#[derive(PartialEq, Properties, Clone)]
struct QemuReassignDiskPanel {
    node: Option<AttrValue>,
    state: PropertyEditorState,
    remote: Option<AttrValue>,
    mobile: bool,
}

enum Msg {
    Target(Option<ClusterResource>),
    LoadResult(Result<Value, Error>),
}

struct QemuReassignDiskPanelComp {
    target: Option<ClusterResource>,
    load_guard: Option<AsyncAbortGuard>,
    used_devices: Option<HashSet<String>>,
}

impl Component for QemuReassignDiskPanelComp {
    type Message = Msg;
    type Properties = QemuReassignDiskPanel;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            target: None,
            load_guard: None,
            used_devices: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::Target(target) => {
                self.target = target;
                if let Some(ClusterResource {
                    node: Some(node),
                    vmid: Some(vmid),
                    ..
                }) = &self.target
                {
                    let url = super::QemuHardwarePanel::new(node.clone(), *vmid)
                        .remote(props.remote.clone())
                        .editor_url();
                    let link = ctx.link().clone();
                    self.load_guard = Some(AsyncAbortGuard::spawn(async move {
                        let result = http_get(&url, None).await;
                        link.send_message(Msg::LoadResult(result));
                    }));
                }
            }
            Msg::LoadResult(result) => {
                match result {
                    Ok(data) => self.used_devices = Some(extract_used_devices(&data)),
                    Err(err) => {
                        log::error!("QemuReassignDiskPanel: load target config failed - {err}");
                        self.used_devices = None;
                    }
                };
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        // let state = &props.state;

        let target_vmid_label = tr!("Target Guest");
        let target_vmid_field = PveGuestSelector::new()
            .remote(props.remote.clone())
            .name("target-vmid")
            .required(true)
            .guest_type(PveGuestType::Qemu)
            .on_change(ctx.link().callback(Msg::Target))
            .mobile(props.mobile);

        let target_disk_label = tr!("Bus/Device");
        let target_disk_field = QemuControllerSelector::new()
            .name("target-disk")
            .exclude_devices(self.used_devices.clone());

        if props.mobile {
            Column::new()
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .gap(2)
                .with_child(label_field(target_vmid_label, target_vmid_field, true))
                .with_child(label_field(target_disk_label, target_disk_field, true))
                .into()
        } else {
            InputPanel::new()
                .min_width(400)
                .with_field(target_vmid_label, target_vmid_field)
                .with_field(target_disk_label, target_disk_field)
                .into()
        }
    }
}

pub fn qemu_reassign_disk_dialog(
    name: &str,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    mobile: bool,
) -> PropertyEditDialog {
    let title = tr!("Reassign Disk");

    PropertyEditDialog::new(title.clone() + " (" + name + ")")
        .mobile(mobile)
        .edit(false)
        .submit_text(title.clone())
        .submit_hook({
            let disk = name.to_string();
            move |state: PropertyEditorState| {
                let mut data = state.form_ctx.get_submit_data();

                data["disk"] = disk.clone().into();
                Ok(data)
            }
        })
        .renderer({
            let node = node.clone();
            move |state| {
                let props = QemuReassignDiskPanel {
                    state,
                    node: node.clone(),
                    remote: remote.clone(),
                    mobile,
                };
                VComp::new::<QemuReassignDiskPanelComp>(Rc::new(props), None).into()
            }
        })
}
