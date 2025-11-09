use std::rc::Rc;

use pve_api_types::{QemuConfigTpmstate0, StorageContent, StorageInfo};
use yew::virtual_dom::VComp;

use pwt::prelude::*;
use pwt::widget::form::{Combobox, FormContextObserver};
use pwt::widget::InputPanel;

const IMAGE_STORAGE: &'static str = "_storage_";
const FILE_PN: &'static str = "_file";

use crate::form::property_string_from_parts;
use crate::form::pve::pve_storage_content_selector::PveStorageContentSelector;
use crate::form::pve::PveStorageSelector;
use crate::{EditableProperty, PropertyEditorState};

#[derive(PartialEq, Properties)]
struct QemuTpmStatePanel {
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    mobile: bool,
    state: PropertyEditorState,
}

enum Msg {
    FormUpdate,
    StorageInfo(Option<StorageInfo>),
}
struct QemuTpmStatePanelComp {
    storage_info: Option<StorageInfo>,
    _observer: FormContextObserver,
}

impl Component for QemuTpmStatePanelComp {
    type Message = Msg;
    type Properties = QemuTpmStatePanel;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        let _observer = props
            .state
            .form_ctx
            .add_listener(ctx.link().callback(|_| Msg::FormUpdate));

        Self {
            storage_info: None,
            _observer,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::StorageInfo(info) => self.storage_info = info,
            Msg::FormUpdate => { /* redraw */ }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let mobile = props.mobile;

        let select_existing = match &self.storage_info {
            Some(StorageInfo {
                select_existing, ..
            }) => select_existing.unwrap_or(false),
            _ => false,
        };

        let disk_image_label = tr!("Disk image");
        let disk_image_field = PveStorageContentSelector::new()
            .mobile(mobile)
            .name(FILE_PN)
            .node(props.node.clone())
            .required(true)
            .storage(self.storage_info.as_ref().map(|info| info.storage.clone()));

        let mut panel = InputPanel::new()
            .mobile(mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .with_field(
                tr!("Storage"),
                PveStorageSelector::new(props.node.clone())
                    .remote(props.remote.clone())
                    .name(IMAGE_STORAGE)
                    .submit(false)
                    .required(true)
                    .autoselect(true)
                    .content_types(Some(vec![StorageContent::Images]))
                    .on_change(ctx.link().callback(Msg::StorageInfo))
                    .mobile(true),
            );

        if select_existing {
            panel.add_field(disk_image_label, disk_image_field);
        }

        panel
            .with_field(
                tr!("Version"),
                Combobox::new()
                    .with_item("v1.2")
                    .with_item("v2.0")
                    .default("v2.0")
                    .name("_version")
                    .required(true),
            )
            .into()
    }
}

pub fn qemu_tpmstate_property(
    name: Option<String>,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    mobile: bool,
) -> EditableProperty {
    let title = tr!("TPM State");
    EditableProperty::new(name.clone(), title)
        .render_input_panel(move |state| {
            let props = QemuTpmStatePanel {
                state,
                node: node.clone(),
                remote: remote.clone(),
                mobile,
            };
            VComp::new::<QemuTpmStatePanelComp>(Rc::new(props), None).into()
        })
        .submit_hook(move |state: PropertyEditorState| {
            let form_ctx = &state.form_ctx;
            let mut data = form_ctx.get_submit_data();

            let storage = form_ctx.read().get_field_text(IMAGE_STORAGE);

            if data[FILE_PN].is_null() {
                // we use 1 here, because for tpmstate the size gets overridden from the backend
                data[FILE_PN] = format!("{}:1", storage).into();
            }

            property_string_from_parts::<QemuConfigTpmstate0>(&mut data, "tpmstate0", true)?;
            Ok(data)
        })
}
