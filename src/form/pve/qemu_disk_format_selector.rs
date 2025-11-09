use std::rc::Rc;

use anyhow::format_err;

use pve_api_types::StorageInfoFormatsDefault;

use pwt::state::Store;

use pwt::prelude::*;
use pwt::props::FieldBuilder;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::form::{Selector, SelectorRenderArgs};

use pwt::widget::GridPicker;

use pwt_macros::{builder, widget};

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::Key;

#[widget(comp=QemuDiskFormatComp, @input)]
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct QemuDiskFormatSelector {
    /// List of supported formats
    #[builder]
    #[prop_or_default]
    supported_formats: Option<Vec<StorageInfoFormatsDefault>>,

    /// Default format
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    default: Option<StorageInfoFormatsDefault>,

    /// Change callback
    #[builder_cb(IntoEventCallback, into_event_callback, Option<StorageInfoFormatsDefault>)]
    #[prop_or_default]
    pub on_change: Option<Callback<Option<StorageInfoFormatsDefault>>>,
}

impl QemuDiskFormatSelector {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct QemuDiskFormatComp {
    store: Store<Entry>,
}

#[derive(Clone, PartialEq)]
struct Entry {
    format: StorageInfoFormatsDefault,
    format_text: String,
    description: String,
}

impl QemuDiskFormatComp {
    fn populate_store(&mut self, ctx: &Context<Self>) {
        let props = ctx.props();

        let mut data = Vec::new();

        let mut cond_push = |format, description| {
            if props
                .supported_formats
                .as_ref()
                .map(|list| list.contains(&format))
                .unwrap_or(true)
            {
                data.push(Entry {
                    format,
                    format_text: format.to_string(),
                    description,
                });
            }
        };

        cond_push(StorageInfoFormatsDefault::Raw, tr!("Raw disk image"));
        cond_push(StorageInfoFormatsDefault::Qcow2, tr!("QEMU image format"));
        cond_push(StorageInfoFormatsDefault::Vmdk, tr!("VMware image format"));

        self.store.set_data(data);
    }
}

impl Component for QemuDiskFormatComp {
    type Message = ();
    type Properties = QemuDiskFormatSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let store = Store::with_extract_key(|entry: &Entry| Key::from(entry.format.to_string()));
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

        Selector::new(
            self.store.clone(),
            move |args: &SelectorRenderArgs<Store<Entry>>| {
                GridPicker::new(
                    DataTable::new(columns(), args.store.clone())
                        .min_width(300)
                        .show_header(false)
                        .header_focusable(false)
                        .class(pwt::css::FlexFit)
                        .into(),
                )
                .selection(args.selection.clone())
                .on_select(args.controller.on_select_callback())
                .into()
            },
        )
        .with_input_props(&props.input_props)
        .style("min-width", "8em")
        .default(props.default.map(|f| f.to_string()))
        .render_value(|v: &AttrValue| v.to_string().into())
        .validate(|(value, store): &(String, Store<Entry>)| {
            store
                .read()
                .iter()
                .find(|item| *item.format.to_string() == *value)
                .ok_or_else(|| format_err!("no such item"))
                .map(|_| ())
        })
        .on_change({
            let on_change = props.on_change.clone();
            let store = self.store.clone();
            move |key: Key| {
                let format = store
                    .read()
                    .iter()
                    .find(|item| *item.format_text == *key)
                    .map(|entry| entry.format.clone());

                if let Some(on_change) = &on_change {
                    on_change.emit(format);
                }
            }
        })
        .into()
    }
}

fn columns() -> Rc<Vec<DataTableHeader<Entry>>> {
    Rc::new(vec![
        DataTableColumn::new(tr!("Format"))
            .width("8em")
            .get_property(|entry: &Entry| &entry.format_text)
            .into(),
        DataTableColumn::new(tr!("Description"))
            .width("15em")
            .render(|entry: &Entry| entry.description.clone().into())
            .into(),
    ])
}
