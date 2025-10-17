use anyhow::{format_err, Error};
use std::rc::Rc;

use yew::html::IntoPropValue;
use yew::prelude::*;

use pwt::props::RenderFn;
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::form::{Selector, SelectorRenderArgs, ValidateFn};
use pwt::widget::GridPicker;

use crate::common_api_types::BasicRealmInfo;

thread_local! {
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
                html!{record.comment.clone().unwrap_or_default()}
            })
            .into(),
    ]);
}

use pwt::props::{FieldBuilder, WidgetBuilder};
use pwt_macros::{builder, widget};

#[widget(comp=ProxmoxRealmSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct RealmSelector {
    /// The default value.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default: Option<AttrValue>,

    /// The path for getting the realm list
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or("/access/domains".into())]
    pub path: AttrValue,
}

impl Default for RealmSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl RealmSelector {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

struct ProxmoxRealmSelector {
    store: Store<BasicRealmInfo>,
    validate: ValidateFn<(String, Store<BasicRealmInfo>)>,
    picker: RenderFn<SelectorRenderArgs<Store<BasicRealmInfo>>>,
    loaded_default_realm: Option<AttrValue>,
}

impl ProxmoxRealmSelector {
    async fn load_realms(url: AttrValue) -> Msg {
        let response: Result<_, Error> = crate::http_get_full(url.to_string(), None).await;

        match response {
            Ok(data) => Msg::LoadComplete(data.data),
            Err(_) => Msg::LoadFailed,
        }
    }
}

enum Msg {
    LoadComplete(Vec<BasicRealmInfo>),
    LoadFailed,
}

impl Component for ProxmoxRealmSelector {
    type Message = Msg;
    type Properties = RealmSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let store = Store::new();
        let url = ctx.props().path.clone();

        let validate = ValidateFn::new(|(realm, store): &(String, Store<BasicRealmInfo>)| {
            store
                .read()
                .data()
                .iter()
                .find(|item| &item.realm == realm)
                .map(drop)
                .ok_or_else(|| format_err!("no such realm"))
        });

        let picker = RenderFn::new(|args: &SelectorRenderArgs<Store<BasicRealmInfo>>| {
            let table =
                DataTable::new(COLUMNS.with(Rc::clone), args.store.clone()).class("pwt-fit");

            GridPicker::new(table)
                .selection(args.selection.clone())
                .on_select(args.controller.on_select_callback())
                .into()
        });

        ctx.link().send_future(Self::load_realms(url));

        Self {
            store,
            validate,
            picker,
            loaded_default_realm: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::LoadComplete(data) => {
                let realm = ctx
                    .props()
                    .default
                    .as_ref()
                    .and_then(|d| data.iter().find(|r| &r.realm == d))
                    .or_else(|| data.iter().find(|r| r.default.unwrap_or_default()))
                    .or_else(|| data.iter().find(|r| r.ty == "pam"))
                    .map(|r| AttrValue::from(r.realm.clone()));

                self.loaded_default_realm = realm;
                self.store.set_data(data);
                true
            }
            // not much we can do here, so just don't re-render
            Msg::LoadFailed => false,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let store = self.store.clone();

        let default = props
            .default
            .clone()
            .or_else(|| self.loaded_default_realm.clone())
            .unwrap_or(AttrValue::from("pam"));

        Selector::new(self.store.clone(), self.picker.clone())
            .with_std_props(&props.std_props)
            .with_input_props(&props.input_props)
            .required(true)
            .default(&default)
            .validate(self.validate.clone())
            // force re-render of the selector after load; returning `true` in update does not
            // re-render the selector by itself
            .key(format!("realm-selector-{default}"))
            .into()
    }
}
