use std::rc::Rc;
use anyhow::{format_err};

use yew::prelude::*;
use yew::html::IntoPropValue;

use pwt::props::RenderFn;
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::GridPicker;
use pwt::widget::form::{Selector, SelectorRenderArgs, ValidateFn};

use crate::common_api_types::BasicRealmInfo;

thread_local!{
    static COLUMNS: Rc<Vec<DataTableHeader<BasicRealmInfo>>> = Rc::new(vec![
        DataTableColumn::new("Realm")
            .width("100px")
            .show_menu(false)
            .render(|record: &BasicRealmInfo| {
                html!{record.realm.clone()}
            })
            .into(),
        DataTableColumn::new("Comment")
            .width("300px")
            .show_menu(false)
            .render(|record: &BasicRealmInfo| {
                html!{record.comment.clone().unwrap_or(String::new())}
            })
            .into(),
    ]);
}

use pwt_macros::{builder, widget};
use pwt::props::{FieldBuilder, WidgetBuilder};

#[widget(comp=ProxmoxRealmSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct RealmSelector {
    /// The default value.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default: Option<AttrValue>,
}

impl RealmSelector {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct ProxmoxRealmSelector{
    store: Store<BasicRealmInfo>,
    validate: ValidateFn<(String, Store<BasicRealmInfo>)>,
    picker: RenderFn<SelectorRenderArgs<Store<BasicRealmInfo>>>,
}

impl Component for  ProxmoxRealmSelector {
    type Message = ();
    type Properties =  RealmSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let store = Store::new()
            .on_change(ctx.link().callback(|_| ())); // trigger redraw

        let validate = ValidateFn::new(|(realm, store): &(String, Store<BasicRealmInfo>)| {
            store.read().data().iter()
                .find(|item| &item.realm == realm)
                .map(drop)
                .ok_or_else(|| format_err!("no such realm"))
        });

        let picker = RenderFn::new(|args: &SelectorRenderArgs<Store<BasicRealmInfo>>| {
            let table = DataTable::new(COLUMNS.with(Rc::clone), args.store.clone())
                .class("pwt-fit");

            GridPicker::new(table)
                .selection(args.selection.clone())
                .on_select(args.on_select.clone())
                .into()
        });

        Self { store, validate, picker }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Selector::new(self.store.clone(), self.picker.clone())
            .with_std_props(&props.std_props)
            .with_input_props(&props.input_props)
            .required(true)
            .default(props.default.as_deref().unwrap_or("pam").to_string())
            .loader("/access/domains")
            .validate(self.validate.clone())
            .into()
    }
}
