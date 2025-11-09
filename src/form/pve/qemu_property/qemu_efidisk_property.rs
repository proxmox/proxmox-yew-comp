use std::rc::Rc;

use pwt::widget::form::{Checkbox, FormContextObserver};
use pwt::widget::Container;

use pwt::prelude::*;
use pwt::widget::{FieldPosition, InputPanel};

use pve_api_types::{
    QemuConfigBios, QemuConfigEfidisk0, QemuConfigEfidisk0Efitype, StorageContent, StorageInfo,
    StorageInfoFormatsDefault,
};
use yew::virtual_dom::VComp;

const IMAGE_STORAGE: &'static str = "_storage_";

const FILE_PN: &'static str = "_file";
const EFITYPE_PN: &'static str = "_efitype";

use crate::form::property_string_from_parts;
use crate::form::pve::{PveStorageContentSelector, PveStorageSelector, QemuDiskFormatSelector};

use crate::{EditableProperty, PropertyEditorState};

#[derive(PartialEq, Properties)]
struct QemuEfidiskPanel {
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    mobile: bool,
    state: PropertyEditorState,
}

enum Msg {
    FormUpdate,
    StorageInfo(Option<StorageInfo>),
}
struct QemuEfidiskPanelComp {
    storage_info: Option<StorageInfo>,
    _observer: FormContextObserver,
}

impl Component for QemuEfidiskPanelComp {
    type Message = Msg;
    type Properties = QemuEfidiskPanel;

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
        let state = &props.state;

        let (supported_formats, default_format, select_existing) = match &self.storage_info {
            Some(StorageInfo {
                formats: Some(formats),
                select_existing,
                ..
            }) => (
                formats.supported.clone(),
                formats.default,
                select_existing.unwrap_or(false),
            ),
            _ => (
                vec![StorageInfoFormatsDefault::Raw],
                StorageInfoFormatsDefault::Raw,
                false,
            ),
        };

        let hint = |msg: String| Container::new().class("pwt-color-warning").with_child(msg);

        let bios = serde_json::from_value::<Option<QemuConfigBios>>(state.record["bios"].clone());
        let show_bios_hint = match bios {
            Ok(Some(QemuConfigBios::Ovmf)) => false,
            _ => true,
        };

        // disable selector if there is no real choice
        let disable_format_selector = supported_formats.len() <= 1;
        let hide_format_selector = select_existing;

        let storage_label = tr!("Storage");
        let storage_field = PveStorageSelector::new(props.node.clone())
            .remote(props.remote.clone())
            .name(IMAGE_STORAGE)
            .submit(false)
            .required(true)
            .autoselect(true)
            .content_types(Some(vec![StorageContent::Images]))
            .on_change(ctx.link().callback(Msg::StorageInfo))
            .mobile(true);

        let format_label = tr!("Format");
        let format_field = QemuDiskFormatSelector::new()
            .name("_format")
            .supported_formats(Some(supported_formats))
            .default(default_format)
            .disabled(disable_format_selector);

        let disk_image_label = tr!("Disk image");
        let disk_image_field = PveStorageContentSelector::new()
            .mobile(mobile)
            .name(FILE_PN)
            .node(props.node.clone())
            .required(true)
            .storage(self.storage_info.as_ref().map(|info| info.storage.clone()));

        let bios_hint = hint(tr!(
            "Warning: The VM currently does not uses 'OVMF (UEFI)' as BIOS."
        ))
        .key("bios_hint");

        let mut panel = InputPanel::new()
            .mobile(mobile)
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .padding_bottom(1) // avoid scrollbar
            .with_field(storage_label, storage_field);

        if select_existing {
            panel.add_field(disk_image_label, disk_image_field);
        } else {
            panel.add_field_with_options(
                FieldPosition::Left,
                false,
                hide_format_selector,
                format_label,
                format_field,
            );
        }

        panel
            .with_field(
                tr!("Pre-Enroll keys"),
                Checkbox::new().name("_pre-enrolled-keys").submit(false),
            )
            .with_custom_child_and_options(FieldPosition::Left, false, !show_bios_hint, bios_hint)
            .into()
    }
}

pub fn qemu_efidisk_property(
    name: Option<AttrValue>,
    node: Option<AttrValue>,
    remote: Option<AttrValue>,
    mobile: bool,
) -> EditableProperty {
    let title = tr!("EFI Disk");
    EditableProperty::new(name.clone(), title)
        .render_input_panel(move |state| {
            let props = QemuEfidiskPanel {
                state,
                node: node.clone(),
                remote: remote.clone(),
                mobile,
            };
            VComp::new::<QemuEfidiskPanelComp>(Rc::new(props), None).into()
        })
        .submit_hook(move |state: PropertyEditorState| {
            let form_ctx = &state.form_ctx;
            let mut data = form_ctx.get_submit_data();

            let storage = form_ctx.read().get_field_text(IMAGE_STORAGE);

            if data[FILE_PN].is_null() {
                // we use 1 here, because for efi the size gets overridden from the backend
                data[FILE_PN] = format!("{}:1", storage).into();
            }

            // always default to newer 4m type with secure boot support, if we're
            // adding a new EFI disk there can't be any old state anyway
            data[EFITYPE_PN] = QemuConfigEfidisk0Efitype::Mb4.to_string().into();

            property_string_from_parts::<QemuConfigEfidisk0>(&mut data, "efidisk0", true)?;
            Ok(data)
        })
}
