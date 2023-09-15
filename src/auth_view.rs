use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;

use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader,
};
use pwt::widget::menu::{Menu, MenuButton, MenuItem};

use pwt::widget::{Button, Toolbar};

use pwt_macros::builder;

use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster, AuthEditOpenID};

use crate::common_api_types::BasicRealmInfo;

#[derive(PartialEq, Properties)]
#[builder]
pub struct AuthView {
    /// Base API path.
    #[prop_or("/access/domains".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,

    /// Allow to add/edit OpenID entries
    #[builder(IntoPropValue, into_prop_value)]
    openid_base_url: Option<AttrValue>,

    /// Allow to add/edit LDAP entries
    #[builder(IntoPropValue, into_prop_value)]
    ldap_base_url: Option<AttrValue>,
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
    EditOpenID(AttrValue),
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

impl ProxmoxAuthView {
    fn get_selected_record(&self) -> Option<BasicRealmInfo> {
        let selected_key = self.selection.selected_key();
        let mut selected_record = None;
        if let Some(key) = &selected_key {
            selected_record = self.store.read().lookup_record(key).map(|r| r.clone());
        }
        selected_record
    }
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
            let mut data: Vec<BasicRealmInfo> = crate::http_get(&*base_url, None).await?;
            data.sort();
            store.set_data(data);
            Ok(())
        })
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();

        match msg {
            Msg::Redraw => { /* just redraw */ }
            Msg::Remove => {
                let _info = match self.get_selected_record() {
                    Some(info) => info,
                    None => return true,
                };

                todo!();
            }
            Msg::Edit => {
                let info = match self.get_selected_record() {
                    Some(info) => info,
                    None => return true,
                };
                if props.openid_base_url.is_some() && info.ty == "openid" {
                    ctx.link().change_view(Some(ViewState::EditOpenID(info.realm.into())));
                }
            }
            Msg::Sync => {
                // fixme: do something
            }
        }
        true
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let props = ctx.props();

         let selected_record = self.get_selected_record();

        let mut remove_disabled = selected_record.is_none();
        let mut edit_disabled = selected_record.is_none();
        let mut sync_disabled = true;

        if let Some(realm_info) = &selected_record {
            if let Some(auth_info) = crate::utils::get_auth_domain_info(&realm_info.ty) {
                sync_disabled = !auth_info.sync;
                remove_disabled = !auth_info.add;
                edit_disabled = !auth_info.add;
            }
        }

        let mut add_menu = Menu::new();

        if props.ldap_base_url.is_some() {
            add_menu.add_item(
                MenuItem::new(tr!("LDAP server"))
                    .icon_class("fa fa-fw fa-address-book-o")
                    .on_select(
                        ctx.link()
                            .change_view_callback(|_| Some(ViewState::AddLDAP)),
                    ),
            );
        }

        if props.openid_base_url.is_some() {
            add_menu.add_item(
                MenuItem::new(tr!("OpenId Connect Server"))
                //.icon_class("fa fa-fw fa-user-o")
                .on_select(
                    ctx.link()
                        .change_view_callback(|_| Some(ViewState::AddOpenID)),
                ),
            )
        }

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

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let columns = COLUMNS.with(Rc::clone);
        DataTable::new(columns, self.store.clone())
            .selection(self.selection.clone())
            .class("pwt-flex-fit")
            .on_row_dblclick({
                let link = ctx.link();
                move |_: &mut _| { link.send_message(Msg::Edit) }
            })
            .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        let props = ctx.props();

        match view_state {
            ViewState::AddLDAP => todo!(),
            ViewState::AddOpenID => Some(
                AuthEditOpenID::new()
                    .base_url(props.openid_base_url.clone().unwrap())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into()
            ),
            ViewState::EditOpenID(realm) => Some(
                AuthEditOpenID::new()
                    .base_url(props.openid_base_url.clone().unwrap())
                    .realm(realm.clone())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into()
            ),
         }
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
