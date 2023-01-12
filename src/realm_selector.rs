use std::rc::Rc;
use anyhow::{format_err};
use serde::{Serialize, Deserialize};

use yew::prelude::*;
use yew::virtual_dom::Key;

use pwt::props::RenderFn;
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::GridPicker;
use pwt::widget::form2::{Selector, SelectorRenderArgs, ValidateFn};

#[derive(Serialize, Deserialize, PartialEq, Clone)]
struct BasicRealmInfo {
    realm: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

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

use pwt_macros::widget;
use pwt::props::{FieldBuilder, WidgetBuilder};

#[widget(comp=ProxmoxRealmSelector, @input)]
#[derive(Properties, PartialEq)]
pub struct RealmSelector {}

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
        let store = Store::with_extract_key(|item: &BasicRealmInfo| Key::from(item.realm.clone()))
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
            .default("pam")
            .loader("/access/domains")
            .validate(self.validate.clone())
            .into()
    }
}
