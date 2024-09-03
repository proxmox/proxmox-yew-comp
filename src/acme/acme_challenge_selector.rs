use std::rc::Rc;

use anyhow::format_err;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use yew::html::IntoEventCallback;
use yew::virtual_dom::Key;

use pwt::prelude::*;
use pwt::props::RenderFn;
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn};
use pwt::widget::form::{Selector, SelectorRenderArgs, ValidateFn};
use pwt::widget::GridPicker;

use pwt_macros::{builder, widget};

#[widget(comp=ProxmoxAcmeChallengeSelector, @input)]
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct AcmeChallengeSelector {
    #[prop_or(AttrValue::Static("/config/acme/challenge-schema"))]
    url: AttrValue,

    /// Change callback
    #[builder_cb(IntoEventCallback, into_event_callback, Option<AcmeChallengeSchemaItem>)]
    #[prop_or_default]
    pub on_change: Option<Callback<Option<AcmeChallengeSchemaItem>>>,
}

impl AcmeChallengeSelector {
    /// Create a new instance for a local datastore.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct AcmeChallengeSchemaItem {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub schema: Value,
}

pub struct ProxmoxAcmeChallengeSelector {
    store: Store<AcmeChallengeSchemaItem>,
    validate: ValidateFn<(String, Store<AcmeChallengeSchemaItem>)>,
    picker: RenderFn<SelectorRenderArgs<Store<AcmeChallengeSchemaItem>>>,
}

impl Component for ProxmoxAcmeChallengeSelector {
    type Message = ();
    type Properties = AcmeChallengeSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let columns = Rc::new(vec![DataTableColumn::new("")
            .flex(1)
            .show_menu(false)
            .render(|item: &AcmeChallengeSchemaItem| {
                item.schema["name"].as_str().unwrap_or(&item.id).into()
            })
            .into()]);

        let store =
            Store::with_extract_key(|item: &AcmeChallengeSchemaItem| Key::from(item.id.clone()))
                .on_change(ctx.link().callback(|_| ())); // trigger redraw

        let validate = ValidateFn::new(|(id, store): &(String, Store<AcmeChallengeSchemaItem>)| {
            store
                .read()
                .data()
                .iter()
                .find(|item| &item.id == id)
                .map(drop)
                .ok_or_else(|| format_err!("no such ACME challenge schema"))
        });

        let picker = RenderFn::new(
            move |args: &SelectorRenderArgs<Store<AcmeChallengeSchemaItem>>| {
                let table = DataTable::new(columns.clone(), args.store.clone())
                    .show_header(false)
                    .class("pwt-flex-fit");

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
            .render_value({
                let store = self.store.clone();
                move |value: &AttrValue| {
                    store
                        .read()
                        .data()
                        .iter()
                        .find(|item| &item.id == value)
                        .and_then(|item| item.schema["name"].as_str())
                        .unwrap_or(value)
                        .into()
                }
            })
            .loader(&*props.url)
            .validate(self.validate.clone())
            .on_change({
                let on_change = props.on_change.clone();
                let store = self.store.clone();
                move |id: Key| {
                    if let Some(on_change) = &on_change {
                        match store.read().data().iter().find(|item| &item.id == &*id) {
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
