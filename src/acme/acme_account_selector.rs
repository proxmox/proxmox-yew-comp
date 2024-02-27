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

use super::acme_accounts::AcmeAccountEntry;

#[widget(comp=ProxmoxAcmeAccountSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct AcmeAccountSelector {
    #[prop_or(AttrValue::Static("/config/acme/account"))]
    url: AttrValue,

    /// The default value.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default: Option<AttrValue>,

    /// Change callback
    #[builder_cb(IntoEventCallback, into_event_callback, Option<String>)]
    #[prop_or_default]
    pub on_change: Option<Callback<Option<String>>>,
}

impl AcmeAccountSelector {
    /// Create a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct ProxmoxAcmeAccountSelector {
    store: Store<AcmeAccountEntry>,
    validate: ValidateFn<(String, Store<AcmeAccountEntry>)>,
    picker: RenderFn<SelectorRenderArgs<Store<AcmeAccountEntry>>>,
}

impl Component for ProxmoxAcmeAccountSelector {
    type Message = ();
    type Properties = AcmeAccountSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let columns = Rc::new(vec![DataTableColumn::new(tr!("Name"))
            .width("200px")
            .show_menu(false)
            .render(|item: &AcmeAccountEntry| {
                html! {&item.name}
            })
            .into()]);

        let store = Store::with_extract_key(|item: &AcmeAccountEntry| Key::from(item.name.clone()))
            .on_change(ctx.link().callback(|_| ())); // trigger redraw

        let validate = ValidateFn::new(|(name, store): &(String, Store<AcmeAccountEntry>)| {
            store
                .read()
                .data()
                .iter()
                .find(|item| &item.name == name)
                .map(drop)
                .ok_or_else(|| format_err!("no such ACME account"))
        });

        let picker = RenderFn::new(move |args: &SelectorRenderArgs<Store<AcmeAccountEntry>>| {
            let table = DataTable::new(columns.clone(), args.store.clone()).class("pwt-flex-fit");

            GridPicker::new(table)
                .selection(args.selection.clone())
                .on_select(args.on_select.clone())
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
                    async move {
                        let data = crate::http_get(&*url, None).await?;
                        Ok(data)
                    }
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
