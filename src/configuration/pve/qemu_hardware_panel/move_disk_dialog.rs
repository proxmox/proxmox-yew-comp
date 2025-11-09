use std::rc::Rc;

use pve_api_types::{StorageContent, StorageInfo, StorageInfoFormatsDefault};
use yew::virtual_dom::VComp;

use pwt::prelude::*;
use pwt::widget::form::{Checkbox, FormContextObserver};
use pwt::widget::InputPanel;

use crate::form::pve::{PveStorageSelector, QemuDiskFormatSelector};
use crate::{PropertyEditDialog, PropertyEditorState};

#[derive(PartialEq, Properties, Clone)]
struct QemuMoveDiskPanel {
    node: Option<AttrValue>,
    state: PropertyEditorState,
    remote: Option<AttrValue>,
    mobile: bool,
}

enum Msg {
    FormUpdate,
    StorageInfo(Option<StorageInfo>),
}

struct QemuMoveDiskPanelComp {
    storage_info: Option<StorageInfo>,
    _observer: FormContextObserver,
}

impl Component for QemuMoveDiskPanelComp {
    type Message = Msg;
    type Properties = QemuMoveDiskPanel;

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
        // let state = &props.state;

        let (supported_formats, default_format) = match &self.storage_info {
            Some(StorageInfo {
                formats: Some(formats),
                ..
            }) => (formats.supported.clone(), formats.default),
            _ => (
                vec![StorageInfoFormatsDefault::Raw],
                StorageInfoFormatsDefault::Raw,
            ),
        };

        // disable selector if there is no real choice
        let disable_format_selector = supported_formats.len() <= 1;

        let storage_label = tr!("Storage");
        let storage_field = PveStorageSelector::new(props.node.clone())
            .remote(props.remote.clone())
            .name("storage")
            .required(true)
            .include_select_existing(false)
            .autoselect(true)
            .content_types(Some(vec![StorageContent::Images]))
            .on_change(ctx.link().callback(Msg::StorageInfo))
            .mobile(props.mobile);

        let format_label = tr!("Format");
        let format_field = QemuDiskFormatSelector::new()
            .name("format")
            .supported_formats(Some(supported_formats))
            .default(default_format)
            .disabled(disable_format_selector);

        let delete_source_label = tr!("Delete source");
        let delete_source_field = Checkbox::new()
            .name("delete")
            .disabled(disable_format_selector);

        InputPanel::new()
            .mobile(props.mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .with_field(storage_label, storage_field)
            .with_field(format_label, format_field)
            .with_field(delete_source_label, delete_source_field)
            .into()
    }
}

pub fn qemu_move_disk_dialog(
    name: &str,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    mobile: bool,
) -> PropertyEditDialog {
    let title = tr!("Move Disk");

    let renderer = {
        let node = node.clone();
        move |state| {
            let props = QemuMoveDiskPanel {
                state,
                node: node.clone(),
                remote: remote.clone(),
                mobile: mobile,
            };
            VComp::new::<QemuMoveDiskPanelComp>(Rc::new(props), None).into()
        }
    };

    let submit_hook = {
        let disk = name.to_string();
        move |state: PropertyEditorState| {
            let mut data = state.form_ctx.get_submit_data();
            data["disk"] = disk.clone().into();
            Ok(data)
        }
    };

    PropertyEditDialog::new(title.clone() + " (" + name + ")")
        .mobile(mobile)
        .edit(false)
        .submit_text(title.clone())
        .renderer(renderer)
        .submit_hook(submit_hook)
}
