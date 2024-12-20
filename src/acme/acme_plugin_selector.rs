use std::rc::Rc;

use anyhow::format_err;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::Key;

use pwt::prelude::*;
use pwt::props::{FieldBuilder, RenderFn, WidgetBuilder};
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn};
use pwt::widget::form::{Selector, SelectorRenderArgs, ValidateFn};
use pwt::widget::GridPicker;

use pwt_macros::{builder, widget};

use super::acme_plugins::{load_acme_plugin_list, PluginConfig};

#[widget(comp=ProxmoxAcmePluginSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct AcmePluginSelector {
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(AttrValue::Static("/config/acme/plugins"))]
    pub url: AttrValue,

    /// The default value.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default: Option<AttrValue>,

    /// Change callback
    #[builder_cb(IntoEventCallback, into_event_callback, Option<String>)]
    #[prop_or_default]
    pub on_change: Option<Callback<Option<String>>>,
}

impl Default for AcmePluginSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl AcmePluginSelector {
    /// Create a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct ProxmoxAcmePluginSelector {
    store: Store<PluginConfig>,
    validate: ValidateFn<(String, Store<PluginConfig>)>,
    picker: RenderFn<SelectorRenderArgs<Store<PluginConfig>>>,
}

impl Component for ProxmoxAcmePluginSelector {
    type Message = ();
    type Properties = AcmePluginSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let columns = Rc::new(vec![DataTableColumn::new(tr!("Name"))
            .width("200px")
            .show_menu(false)
            .render(|item: &PluginConfig| {
                html! {&item.plugin}
            })
            .into()]);

        let store = Store::with_extract_key(|item: &PluginConfig| Key::from(item.plugin.clone()))
            .on_change(ctx.link().callback(|_| ())); // trigger redraw

        let validate = ValidateFn::new(|(plugin, store): &(String, Store<PluginConfig>)| {
            store
                .read()
                .data()
                .iter()
                .find(|item| &item.plugin == plugin)
                .map(drop)
                .ok_or_else(|| format_err!("no such ACME plugin"))
        });

        let picker = RenderFn::new(move |args: &SelectorRenderArgs<Store<PluginConfig>>| {
            let table = DataTable::new(columns.clone(), args.store.clone()).class("pwt-flex-fit");

            GridPicker::new(table)
                .selection(args.selection.clone())
                .on_select(args.controller.on_select_callback())
                .into()
        });

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
            .loader((
                |url: AttrValue| {
                    let url = url.clone();
                    async move { load_acme_plugin_list(url.clone()).await }
                },
                props.url.clone(),
            ))
            .validate(self.validate.clone())
            .on_change({
                let on_change = props.on_change.clone();
                move |plugin: Key| {
                    if let Some(on_change) = &on_change {
                        on_change.emit(Some(plugin.to_string()));
                    }
                }
            })
            .into()
    }
}
