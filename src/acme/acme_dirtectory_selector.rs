use std::rc::Rc;

use anyhow::format_err;
use serde::{Deserialize, Serialize};

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::Key;

use pwt::prelude::*;
use pwt::props::{FieldBuilder, RenderFn, WidgetBuilder};
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn};
use pwt::widget::form::{Selector, SelectorRenderArgs, ValidateFn};
use pwt::widget::GridPicker;

use pwt_macros::{builder, widget};

#[widget(comp=ProxmoxAcmeDirectorySelector, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct AcmeDirectorySelector {
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(AttrValue::Static("/config/acme/directories"))]
    pub url: AttrValue,

    /// The default value.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default: Option<AttrValue>,

    /// Change callback
    #[builder_cb(IntoEventCallback, into_event_callback, Option<AcmeDirectoryListItem>)]
    #[prop_or_default]
    pub on_change: Option<Callback<Option<AcmeDirectoryListItem>>>,
}

impl AcmeDirectorySelector {
    /// Create a new instance for a local datastore.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub struct AcmeDirectoryListItem {
    pub name: String,
    pub url: String,
}

pub struct ProxmoxAcmeDirectorySelector {
    store: Store<AcmeDirectoryListItem>,
    validate: ValidateFn<(String, Store<AcmeDirectoryListItem>)>,
    picker: RenderFn<SelectorRenderArgs<Store<AcmeDirectoryListItem>>>,
}

impl Component for ProxmoxAcmeDirectorySelector {
    type Message = ();
    type Properties = AcmeDirectorySelector;

    fn create(ctx: &Context<Self>) -> Self {
        let columns = Rc::new(vec![
            DataTableColumn::new(tr!("Name"))
                .width("200px")
                .show_menu(false)
                .render(|item: &AcmeDirectoryListItem| {
                    html! {&item.name}
                })
                .into(),
            DataTableColumn::new(tr!("URL"))
                .width("400px")
                .show_menu(false)
                .render(|item: &AcmeDirectoryListItem| {
                    html! {&item.url}
                })
                .into(),
        ]);

        let store =
            Store::with_extract_key(|item: &AcmeDirectoryListItem| Key::from(item.url.clone()))
                .on_change(ctx.link().callback(|_| ())); // trigger redraw

        let validate = ValidateFn::new(|(url, store): &(String, Store<AcmeDirectoryListItem>)| {
            store
                .read()
                .data()
                .iter()
                .find(|item| &item.url == url)
                .map(drop)
                .ok_or_else(|| format_err!("no such ACME directory"))
        });

        let picker = RenderFn::new(
            move |args: &SelectorRenderArgs<Store<AcmeDirectoryListItem>>| {
                let table =
                    DataTable::new(columns.clone(), args.store.clone()).class("pwt-flex-fit");

                GridPicker::new(table)
                    .selection(args.selection.clone())
                    .on_select(args.controller.on_select_callback())
                    .into()
            },
        );

        Self {
            store,
            validate,
            picker,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Selector::new(self.store.clone(), self.picker.clone())
            .with_std_props(&props.std_props)
            .with_input_props(&props.input_props)
            .default(props.default.clone())
            .autoselect(true)
            .loader(&*props.url)
            .validate(self.validate.clone())
            .render_value({
                let store = self.store.clone();
                move |url: &AttrValue| {
                    let text = match store
                        .read()
                        .data()
                        .iter()
                        .find(|item| &item.url == url.as_str())
                    {
                        Some(entry) => entry.name.clone(),
                        None => url.to_string(),
                    };
                    html! {text}
                }
            })
            .on_change({
                let on_change = props.on_change.clone();
                let store = self.store.clone();
                move |url: Key| {
                    if let Some(on_change) = &on_change {
                        match store.read().data().iter().find(|item| &item.url == &*url) {
                            Some(entry) => {
                                on_change.emit(Some(entry.clone()));
                            }
                            None => {
                                on_change.emit(None);
                            }
                        }
                    }
                }
            })
            .into()
    }
}
