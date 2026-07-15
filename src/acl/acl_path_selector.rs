use std::rc::Rc;

use serde_json::Value;

use yew::html::IntoPropValue;
use yew::virtual_dom::Key;

use pwt::prelude::*;
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn};
use pwt::widget::form::{Selector, SelectorRenderArgs};
use pwt::widget::GridPicker;

use pwt_macros::{builder, widget};

/// Selector for ACL paths, offering the paths on which permissions can be granted.
///
/// The candidates are the object keys of the configured permissions listing endpoint. The field
/// stays editable, so a path missing from the listing, or one that could not be loaded, can
/// still be entered by hand.
#[widget(comp=ProxmoxAclPathSelector, @input, @element)]
#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct AclPathSelector {
    /// Endpoint returning the grantable paths as object keys.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(AttrValue::Static("/access/permissions"))]
    pub permissions_api_endpoint: AttrValue,
}

impl AclPathSelector {
    /// Creates a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

impl Default for AclPathSelector {
    fn default() -> Self {
        Self::new()
    }
}

#[doc(hidden)]
pub enum Msg {
    Loaded(Vec<String>),
}

#[doc(hidden)]
pub struct ProxmoxAclPathSelector {
    store: Store<String>,
}

impl Component for ProxmoxAclPathSelector {
    type Message = Msg;
    type Properties = AclPathSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let endpoint = ctx.props().permissions_api_endpoint.to_string();
        ctx.link().send_future(async move {
            let mut paths: Vec<String> = match crate::http_get::<Value>(&endpoint, None).await {
                Ok(Value::Object(map)) => map.keys().cloned().collect(),
                Ok(_) => Vec::new(),
                Err(err) => {
                    // manual path entry still works, so just log the miss
                    log::error!("loading ACL paths from {endpoint} failed: {err}");
                    Vec::new()
                }
            };
            paths.sort();
            Msg::Loaded(paths)
        });

        Self {
            store: Store::with_extract_key(|item: &String| Key::from(item.as_str())),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded(paths) => {
                self.store.write().set_data(paths);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        Selector::new(
            self.store.clone(),
            |args: &SelectorRenderArgs<Store<String>>| {
                let column = Rc::new(vec![
                    DataTableColumn::new("Path")
                        .show_menu(false)
                        .render(|v: &String| html! {v})
                        .into(),
                ]);

                let table = DataTable::new(column, args.store.clone())
                    .striped(true)
                    .borderless(true)
                    .bordered(false)
                    .show_header(false);

                GridPicker::new(table)
                    .selection(args.selection.clone())
                    .on_select(args.controller.on_select_callback())
                    .into()
            },
        )
        .with_std_props(&ctx.props().std_props)
        .with_input_props(&ctx.props().input_props)
        .editable(true)
        .into()
    }
}
