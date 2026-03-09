use std::collections::HashSet;
use std::rc::Rc;

use serde_json::{json, Value};
use yew::virtual_dom::VComp;

use pve_api_types::LxcConfigFeatures;

use crate::form::{
    delete_default_values, delete_empty_values, flatten_property_string, property_string_from_parts,
};

use pwt::prelude::*;
use pwt::widget::form::{Checkbox, FormContextObserver};
use pwt::widget::InputPanel;

use crate::{EditableProperty, PropertyEditorState};

const NFS_CHECKBOX_NAME: &str = "_nfs_";
const CIFS_CHECKBOX_NAME: &str = "_cifs_";

const FEATURES_PN: &str = "features";
const MOUNT_PN: &str = "_mount";
const NESTING_PN: &str = "_nesting";
const KEYCTL_PN: &str = "_keyctl";
const FUSE_PN: &str = "_fuse";
const MKNOD_PN: &str = "_mknod";

const FSTYPE_NFS: &str = "nfs";
const FSTYPE_CIFS: &str = "cifs";

#[derive(PartialEq, Properties)]
struct LxcFeaturesPanel {
    mobile: bool,
    state: PropertyEditorState,
}

enum Msg {
    FormUpdate,
}
struct LxcFeaturesComp {
    _observer: FormContextObserver,
}

impl Component for LxcFeaturesComp {
    type Message = Msg;
    type Properties = LxcFeaturesPanel;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        let _observer = props
            .state
            .form_ctx
            .add_listener(ctx.link().callback(|_| Msg::FormUpdate));

        Self { _observer }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let unpriviledged = match props.state.record["unprivileged"] {
            Value::Bool(true) => true,
            _ => false,
        };

        let unprivileged_hint = |msg: String| {
            if !unpriviledged {
                format!("{msg} ({})", tr!("unprivileged only"))
            } else {
                msg
            }
        };

        let privileged_hint = |msg: String| {
            if unpriviledged {
                format!("{msg} ({})", tr!("privileged only"))
            } else {
                msg
            }
        };

        let panel = InputPanel::new()
            .mobile(props.mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .padding_bottom(1) // avoid scrollbar
            .label_width((!props.mobile).then_some("max-content"))
            .with_single_line_field(
                false,
                false,
                unprivileged_hint(tr!("keyctl")),
                Checkbox::new().name(KEYCTL_PN).disabled(!unpriviledged),
            )
            .with_single_line_field(
                false,
                false,
                tr!("Nesting"),
                Checkbox::new().name(NESTING_PN),
            )
            .with_single_line_field(
                false,
                false,
                privileged_hint(String::from("NFS")),
                Checkbox::new()
                    .name(NFS_CHECKBOX_NAME)
                    .disabled(unpriviledged)
                    .submit(false),
            )
            .with_single_line_field(
                false,
                false,
                privileged_hint(String::from("CIFS")),
                Checkbox::new()
                    .name(CIFS_CHECKBOX_NAME)
                    .disabled(unpriviledged)
                    .submit(false),
            )
            .with_single_line_field(false, false, "FUSE", Checkbox::new().name(FUSE_PN))
            .with_single_line_field(
                false,
                false,
                tr!("Create Device Nodes") + " (" + &tr!("experimental") + ")",
                Checkbox::new().name(MKNOD_PN),
            );

        panel.into()
    }
}

pub fn lxc_features_property(mobile: bool) -> EditableProperty {
    let title = tr!("Features");
    EditableProperty::new(FEATURES_PN, title)
        .required(true)
        .placeholder(tr!("None"))
        .render_input_panel(move |state| {
            let props = LxcFeaturesPanel { state, mobile };
            VComp::new::<LxcFeaturesComp>(Rc::new(props), None).into()
        })
        .load_hook(move |mut record: Value| {
            flatten_property_string::<LxcConfigFeatures>(&mut record, FEATURES_PN)?;

            if let Some(mount) = record[MOUNT_PN].as_str() {
                let map = mount
                    .split([';', ' '])
                    .map(String::from)
                    .collect::<HashSet<String>>();
                record[NFS_CHECKBOX_NAME] = map.contains(FSTYPE_NFS).into();
                record[CIFS_CHECKBOX_NAME] = map.contains(FSTYPE_CIFS).into();
            }

            Ok(record)
        })
        .submit_hook(move |state: PropertyEditorState| {
            let form_ctx = &state.form_ctx;
            let mut data = form_ctx.get_submit_data();

            let mut mount_map: HashSet<String> =
                if let Some(mount) = state.record[MOUNT_PN].as_str() {
                    mount.split([';', ' ']).map(String::from).collect()
                } else {
                    HashSet::new()
                };

            if form_ctx.read().get_field_checked(NFS_CHECKBOX_NAME) {
                mount_map.insert(String::from(FSTYPE_NFS));
            } else {
                mount_map.remove(FSTYPE_NFS);
            }
            if form_ctx.read().get_field_checked(CIFS_CHECKBOX_NAME) {
                mount_map.insert(String::from(FSTYPE_CIFS));
            } else {
                mount_map.remove(FSTYPE_CIFS);
            }

            if !mount_map.is_empty() {
                data[MOUNT_PN] = mount_map
                    .into_iter()
                    .collect::<Vec<String>>()
                    .join(";")
                    .into();
            }

            let defaults = json!({
                KEYCTL_PN: false,
                FUSE_PN: false,
                MKNOD_PN: false,
                NESTING_PN: false,
            });
            delete_default_values(&mut data, &defaults);

            property_string_from_parts::<LxcConfigFeatures>(&mut data, FEATURES_PN, true)?;
            let data = delete_empty_values(&data, &[FEATURES_PN], false);
            Ok(data)
        })
}
