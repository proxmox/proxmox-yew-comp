use std::rc::Rc;

use pve_api_types::StorageInfoFormatsDefault;

use pwt::state::Store;

use pwt::prelude::*;
use pwt::widget::form::Number;

use pwt::widget::{Labelable, Row};

use pwt_macros::builder;

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use super::QemuDiskFormatSelector;

//#[widget(comp=QemuDiskSizeFormatComp, @input)]
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct QemuDiskSizeFormatSelector {
    /// Field name used by disk size input ([f64])
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(QemuDiskSizeFormatSelector::DISK_SIZE)]
    pub disk_size_name: AttrValue,

    /// Field name used by disk format input ([String])
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(QemuDiskSizeFormatSelector::DISK_FORMAT)]
    pub disk_format_name: AttrValue,

    /// Field disabled flag.
    #[builder]
    #[prop_or_default]
    pub disabled: bool,

    #[prop_or_default]
    label_id: Option<AttrValue>,

    /// List of supported formats
    #[builder]
    #[prop_or_default]
    supported_formats: Option<Vec<StorageInfoFormatsDefault>>,

    /// Default format
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    default_format: Option<StorageInfoFormatsDefault>,
}

impl QemuDiskSizeFormatSelector {
    pub const DISK_SIZE: AttrValue = AttrValue::Static("_disk_size_");
    pub const DISK_FORMAT: AttrValue = AttrValue::Static("_disk_format_");

    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

// impl Labelable, so that we can use it with InputPanel
impl Labelable for QemuDiskSizeFormatSelector {
    fn name(&self) -> Option<AttrValue> {
        Some(self.disk_size_name.clone())
    }
    fn set_label_id(&mut self, label_id: AttrValue) {
        self.label_id = Some(label_id);
    }
    fn disabled(&self) -> bool {
        self.disabled
    }
}

pub struct QemuDiskSizeFormatComp {
    store: Store<Entry>,
}

#[derive(Clone, PartialEq)]
struct Entry {
    format: String,
    description: String,
}

impl QemuDiskSizeFormatComp {
    fn populate_store(&mut self, ctx: &Context<Self>) {
        let props = ctx.props();

        let mut data = Vec::new();

        if props
            .supported_formats
            .as_ref()
            .map(|list| list.contains(&StorageInfoFormatsDefault::Raw))
            .unwrap_or(true)
        {
            data.push(Entry {
                format: String::from("raw"),
                description: tr!("Raw disk image"),
            });
        }
        if props
            .supported_formats
            .as_ref()
            .map(|list| list.contains(&StorageInfoFormatsDefault::Qcow2))
            .unwrap_or(true)
        {
            data.push(Entry {
                format: String::from("qcow2"),
                description: tr!("QEMU image format"),
            });
        }
        if props
            .supported_formats
            .as_ref()
            .map(|list| list.contains(&StorageInfoFormatsDefault::Vmdk))
            .unwrap_or(true)
        {
            data.push(Entry {
                format: String::from("vmdk"),
                description: tr!("VMware image format"),
            });
        }
        self.store.set_data(data);
    }
}

impl Component for QemuDiskSizeFormatComp {
    type Message = ();
    type Properties = QemuDiskSizeFormatSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let store = Store::with_extract_key(|entry: &Entry| Key::from(entry.format.clone()));
        let mut me = Self { store };
        me.populate_store(ctx);
        me
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if props.supported_formats != old_props.supported_formats {
            self.populate_store(ctx);
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        Row::new()
            .gap(1)
            .with_child(
                Number::<f64>::new()
                    .style("min-width", "0")
                    .name(&props.disk_size_name)
                    .label_id(props.label_id.clone())
                    .disabled(props.disabled)
                    .submit(false)
                    .required(true)
                    .min(0.001)
                    .max(128.0 * 1024.0)
                    .default(32.0),
            )
            .with_child(
                QemuDiskFormatSelector::new()
                    .name(&props.disk_format_name)
                    .required(true)
                    .disabled(props.disabled)
                    .supported_formats(props.supported_formats.clone())
                    .default(props.default_format)
                    .submit(false),
            )
            .into()
    }
}

impl Into<VNode> for QemuDiskSizeFormatSelector {
    fn into(self) -> VNode {
        let comp = VComp::new::<QemuDiskSizeFormatComp>(Rc::new(self), None);
        VNode::from(comp)
    }
}
