use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;


use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{PersistentState, Selection, Store};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader,
};
use pwt::widget::menu::{Menu, MenuButton, MenuItem};
use pwt::widget::form::{Field, Form, FormContext};

use pwt::widget::{Button, Column, Toolbar};

use crate::utils::{render_epoch_short, render_upid};

use pwt_macros::builder;

use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster};

use crate::common_api_types::BasicRealmInfo;

#[derive(PartialEq, Properties)]
#[builder]
pub struct AuthView {
    #[prop_or("/access/domains".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,
}

impl AuthView {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[derive(PartialEq)]
pub enum ViewState {
    AddLDAP,
    AddOpenID,
}

pub enum Msg {
    Redraw,
    Edit,
    Remove,
    Sync,
}
#[doc(hidden)]
pub struct ProxmoxAuthView {
    selection: Selection,
    store: Store<BasicRealmInfo>,
}

impl LoadableComponent for ProxmoxAuthView {
    type Properties = AuthView;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let store = Store::new();
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::Redraw));
        Self { store, selection }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let props = ctx.props();
        let base_url = props.base_url.clone();
        let store = self.store.clone();
        Box::pin(async move {
            let data = crate::http_get(&*base_url, None).await?;
            store.set_data(data);
            Ok(())
        })
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let selected_key = self.selection.selected_key();
        let mut selected_record = None;
        if let Some(key) = &selected_key {
            selected_record = self.store.read().lookup_record(key).map(|r| r.clone());
        }

        let mut remove_disabled = selected_key.is_none();
        let mut edit_disabled = selected_key.is_none();
        let mut sync_disabled = true;

        if let Some(realm_info) = &selected_record {
            if let Some(auth_info) = crate::utils::get_auth_domain_info(&realm_info.ty) {
                sync_disabled = !auth_info.sync;
                remove_disabled = !auth_info.add;
                edit_disabled = !auth_info.add;
            }
        }

        let add_menu = Menu::new()
            .with_item(
                MenuItem::new(tr!("LDAP server"))
                    .icon_class("fa fa-fw fa-address-book-o")
                    .on_select(
                        ctx.link()
                            .change_view_callback(|_| Some(ViewState::AddLDAP)),
                    ),
            )
        .with_item(
            MenuItem::new(tr!("OpenId Connect Server"))
                //.icon_class("fa fa-fw fa-user-o")
                .on_select(
                    ctx.link()
                        .change_view_callback(|_| Some(ViewState::AddOpenID)),
                ),
        );

        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(MenuButton::new("Add").show_arrow(true).menu(add_menu))
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(edit_disabled)
                    .onclick(ctx.link().callback(|_| Msg::Edit)),
            )
            .with_child(
                Button::new(tr!("Remove"))
                    .disabled(remove_disabled)
                    .onclick(ctx.link().callback(|_| Msg::Remove)),
            )
            .with_child(
                Button::new(tr!("Sync"))
                    .disabled(sync_disabled)
                    .onclick(ctx.link().callback(|_| Msg::Sync)),
            );

        Some(toolbar.into())
    }

    fn main_view(&self, _ctx: &LoadableComponentContext<Self>) -> Html {
        let columns = COLUMNS.with(Rc::clone);
        DataTable::new(columns, self.store.clone())
            .selection(self.selection.clone())
            .class("pwt-flex-fit")
            .into()
    }
}

thread_local! {
    static COLUMNS: Rc<Vec<DataTableHeader<BasicRealmInfo>>> = Rc::new(vec![
        DataTableColumn::new(tr!("Realm"))
            .width("100px")
            .render(|item: &BasicRealmInfo| {
                html!{item.realm.clone()}
            })
            .sorter(|a: &BasicRealmInfo, b: &BasicRealmInfo| {
                a.realm.cmp(&b.realm)
            })
            .into(),
        DataTableColumn::new(tr!("Type"))
            .width("100px")
            .render(|item: &BasicRealmInfo| {
                html!{item.ty.clone()}
            })
            .sorter(|a: &BasicRealmInfo, b: &BasicRealmInfo| {
                a.ty.cmp(&b.ty)
            })
            .into(),
        DataTableColumn::new("Comment")
            .flex(1)
            .show_menu(false)
            .render(|record: &BasicRealmInfo| {
                html!{record.comment.clone().unwrap_or(String::new())}
            })
            .into(),
    ]);
}

impl Into<VNode> for AuthView {
    fn into(self) -> VNode {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxAuthView>>(Rc::new(self), None);
        VNode::from(comp)
    }
}
