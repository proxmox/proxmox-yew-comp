use std::collections::HashSet;
use std::rc::Rc;

use anyhow::{bail, Error};

use pwt::widget::form::{Number, ValidateFn};
use serde_json::Value;
use yew::virtual_dom::VComp;

use pve_api_types::{ClusterResource, LxcConfigMpArray, LxcConfigUnusedArray};

use pwt::prelude::*;
use pwt::widget::InputPanel;
use pwt::AsyncAbortGuard;

const TARGET_MOUNT_POINT_ID: &'static str = "_target_mount_point_id_";

use crate::configuration::guest_config_url;
use crate::form::pve::{
    extract_unused_keys, extract_used_mount_points, first_unused_mount_point, PveGuestSelector,
    PveGuestType,
};
use crate::http_get;
use crate::{PropertyEditDialog, PropertyEditorState};

#[derive(PartialEq, Properties, Clone)]
struct LxcReassignVolumePanel {
    node: Option<AttrValue>,
    vmid: u32,
    state: PropertyEditorState,
    remote: Option<AttrValue>,

    mobile: bool,
    unused: bool,
}

enum Msg {
    Target(Option<ClusterResource>),
    LoadResult(Result<Value, Error>),
}

struct LxcReassignVolumeComp {
    target: Option<ClusterResource>,
    load_guard: Option<AsyncAbortGuard>,
    used_mount_points: HashSet<String>,
    validate_id: Option<ValidateFn<u16>>,
}

impl Component for LxcReassignVolumeComp {
    type Message = Msg;
    type Properties = LxcReassignVolumePanel;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            target: None,
            load_guard: None,
            used_mount_points: HashSet::new(),
            validate_id: None,
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
                    let url = guest_config_url(
                        *vmid,
                        &node.clone().into(),
                        &props.remote,
                        PveGuestType::Lxc,
                    );
                    let link = ctx.link().clone();
                    self.load_guard = Some(AsyncAbortGuard::spawn(async move {
                        let result = http_get(&url, None).await;
                        link.send_message(Msg::LoadResult(result));
                    }));
                }
            }
            Msg::LoadResult(result) => {
                let form_ctx = &props.state.form_ctx;

                match result {
                    Ok(data) => {
                        if props.unused {
                            self.used_mount_points = extract_unused_keys(&data);
                            if let Some(first) = first_unused_unused_key(&self.used_mount_points) {
                                form_ctx
                                    .write()
                                    .set_field_value(TARGET_MOUNT_POINT_ID, first.into())
                            }
                        } else {
                            self.used_mount_points = extract_used_mount_points(&data);
                            if let Some(first) = first_unused_mount_point(&self.used_mount_points) {
                                form_ctx
                                    .write()
                                    .set_field_value(TARGET_MOUNT_POINT_ID, first.into())
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("LxcReassignVolumePanel: load target config failed - {err}");
                        self.used_mount_points = HashSet::new();
                    }
                };

                self.validate_id = Some(ValidateFn::from({
                    let used_mount_points = self.used_mount_points.clone();
                    move |id: &u16| {
                        if used_mount_points.contains(&format!("mp{id}")) {
                            bail!(tr!("Mount point is already in use."));
                        }
                        Ok(())
                    }
                }))
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let max_id = if props.unused {
            LxcConfigUnusedArray::MAX
        } else {
            LxcConfigMpArray::MAX
        };

        let target_vmid_label = tr!("Target Guest");
        let target_vmid_field = PveGuestSelector::new()
            .remote(props.remote.clone())
            .name("target-vmid")
            .required(true)
            .guest_type(PveGuestType::Lxc)
            .exclude_guest(props.vmid)
            .on_change(ctx.link().callback(Msg::Target))
            .mobile(props.mobile);

        let target_mount_point_label = if props.unused {
            tr!("Add as unused volume")
        } else {
            tr!("Add as mount point")
        };

        let target_mount_point_field = Number::<u16>::new()
            .name(TARGET_MOUNT_POINT_ID)
            .submit(false)
            .required(true)
            .min(0)
            .max((max_id - 1) as u16)
            .disabled(self.validate_id.is_none())
            .validate(self.validate_id.clone());

        InputPanel::new()
            .mobile(props.mobile)
            .field_width((!props.mobile).then(|| "300px"))
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .with_field(target_vmid_label, target_vmid_field)
            .with_field(target_mount_point_label, target_mount_point_field)
            .into()
    }
}

pub fn lxc_reassign_volume_dialog(
    name: &str,
    node: Option<AttrValue>,
    vmid: u32,
    remote: Option<AttrValue>,
    mobile: bool,
) -> PropertyEditDialog {
    let title = tr!("Reassign Volume");

    let unused = name.starts_with("unused");

    PropertyEditDialog::new(title.clone() + " (" + name + ")")
        .mobile(mobile)
        .edit(false)
        .submit_text(title.clone())
        .submit_hook({
            let disk = name.to_string();
            move |state: PropertyEditorState| {
                let form_ctx = &state.form_ctx;
                let mut data = form_ctx.get_submit_data();
                let id = form_ctx.read().get_field_text(TARGET_MOUNT_POINT_ID);

                data["volume"] = disk.clone().into();

                data["target-volume"] = if unused {
                    format!("unused{id}").into()
                } else {
                    format!("mp{id}").into()
                };

                Ok(data)
            }
        })
        .renderer({
            let node = node.clone();
            move |state| {
                let props = LxcReassignVolumePanel {
                    state,
                    node: node.clone(),
                    vmid,
                    remote: remote.clone(),
                    mobile,
                    unused,
                };
                VComp::new::<LxcReassignVolumeComp>(Rc::new(props), None).into()
            }
        })
}

fn first_unused_unused_key(prop_names: &HashSet<String>) -> Option<usize> {
    for n in 0..LxcConfigUnusedArray::MAX {
        let name = format!("unused{n}");
        if !prop_names.contains(&name) {
            return Some(n);
        }
    }
    None
}
