use std::borrow::BorrowMut;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use indexmap::IndexMap;
use serde_json::json;

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::css;
use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::menu::{Menu, MenuButton, MenuItem};
use pwt::widget::Toolbar;

use pwt_macros::builder;

use proxmox_access_control::types::{AclListItem, AclUgidType};

use crate::percent_encoding::percent_encode_component;
use crate::utils::render_boolean;
use crate::{
    ConfirmButton, EditWindow, LoadableComponent, LoadableComponentContext, LoadableComponentMaster,
};

use super::acl_edit::AclEditWindow;

#[derive(PartialEq, Properties)]
#[builder]
pub struct AclView {
    /// Show the ACL entries for the specified API path and sub-paths only.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    acl_path: Option<AttrValue>,

    /// Specifies the endpoint from which to fetch the ACL entries from via GET and to update them
    /// via PUT requests.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(String::from("/access/acl"))]
    acl_api_endpoint: String,

    /// Menu entries for editing the ACL. The key is used as the menu label while the value should
    /// be a tuple containing icon class and the dialog for editing ACL entries.
    #[prop_or_default]
    // using an index map here preserves the insertion order
    edit_acl_menu: IndexMap<AttrValue, (Classes, EditWindow)>,
}

impl AclView {
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    pub fn with_acl_edit_menu_entry(
        mut self,
        lable: impl Into<AttrValue>,
        icon: impl Into<Classes>,
        dialog: impl AclEditWindow,
    ) -> Self {
        self.add_acl_edit_menu_entry(lable, icon, dialog);
        self
    }

    pub fn add_acl_edit_menu_entry(
        &mut self,
        lable: impl Into<AttrValue>,
        icon: impl Into<Classes>,
        dialog: impl AclEditWindow,
    ) {
        self.edit_acl_menu
            .borrow_mut()
            .insert(lable.into(), (icon.into(), dialog.into()));
    }
}

impl Default for AclView {
    fn default() -> Self {
        AclView::new()
    }
}

impl From<AclView> for VNode {
    fn from(value: AclView) -> Self {
        VComp::new::<LoadableComponentMaster<ProxmoxAclView>>(Rc::new(value), None).into()
    }
}

#[derive(Clone, PartialEq)]
enum ViewState {
    AddAcl(AttrValue),
}

enum Msg {
    Reload,
    Remove,
}

struct ProxmoxAclView {
    selection: Selection,
    store: Store<AclListItem>,
}

impl ProxmoxAclView {
    fn colmuns() -> Rc<Vec<DataTableHeader<AclListItem>>> {
        Rc::new(vec![
            DataTableColumn::new(tr!("Path"))
                .flex(1)
                .render(|item: &AclListItem| item.path.as_str().into())
                .sorter(|a: &AclListItem, b: &AclListItem| a.path.cmp(&b.path))
                .sort_order(true)
                .into(),
            DataTableColumn::new(tr!("User/Group/API Token"))
                .flex(1)
                .render(|item: &AclListItem| item.ugid.as_str().into())
                .sorter(|a: &AclListItem, b: &AclListItem| a.ugid.cmp(&b.ugid))
                .sort_order(true)
                .into(),
            DataTableColumn::new(tr!("Role"))
                .flex(1)
                .render(|item: &AclListItem| item.roleid.as_str().into())
                .sorter(|a: &AclListItem, b: &AclListItem| a.roleid.cmp(&b.roleid))
                .into(),
            DataTableColumn::new(tr!("Propagate"))
                .render(|item: &AclListItem| render_boolean(item.propagate).as_str().into())
                .sorter(|a: &AclListItem, b: &AclListItem| a.propagate.cmp(&b.propagate))
                .into(),
        ])
    }
}

impl LoadableComponent for ProxmoxAclView {
    type Properties = AclView;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let link = ctx.link();
        link.repeated_load(5000);

        let selection = Selection::new().on_select(link.callback(|_| Msg::Reload));

        let store = Store::with_extract_key(|record: &AclListItem| {
            let acl_id = format!("{} for {} - {}", record.path, record.ugid, record.roleid);
            Key::from(acl_id)
        });

        Self { selection, store }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let store = self.store.clone();
        let url = &ctx.props().acl_api_endpoint;

        let path = if let Some(path) = &ctx.props().acl_path {
            format!("{url}&path={}", percent_encode_component(path))
        } else {
            url.to_owned()
        };

        Box::pin(async move {
            let data = crate::http_get(&path, None).await?;
            store.write().set_data(data);
            Ok(())
        })
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let selected_id = self.selection.selected_key().map(|k| k.to_string());
        let disabled = selected_id.is_none();

        let mut toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .border_bottom(true);

        if !ctx.props().edit_acl_menu.is_empty() {
            let add_menu = ctx.props().edit_acl_menu.iter().fold(
                Menu::new(),
                |add_menu, (label, (icon, _))| {
                    let msg = label.to_owned();

                    add_menu.with_item(
                        MenuItem::new(label.to_owned())
                            .icon_class(icon.to_owned())
                            .on_select(ctx.link().change_view_callback(move |_| {
                                Some(ViewState::AddAcl(msg.clone()))
                            })),
                    )
                },
            );

            toolbar.add_child(MenuButton::new(tr!("Add")).show_arrow(true).menu(add_menu));
        }

        toolbar.add_child(
            ConfirmButton::new(tr!("Remove ACL Entry"))
                .confirm_message(tr!("Are you sure you want to remove this ACL entry?"))
                .disabled(disabled)
                .on_activate(ctx.link().callback(|_| Msg::Remove)),
        );

        Some(toolbar.into())
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Reload => true,
            Msg::Remove => {
                if let Some(key) = self.selection.selected_key() {
                    if let Some(record) = self.store.read().lookup_record(&key).cloned() {
                        let link = ctx.link();
                        let url = ctx.props().acl_api_endpoint.to_owned();

                        link.clone().spawn(async move {
                            let data = match record.ugid_type {
                                AclUgidType::User => json!({
                                    "delete": true,
                                    "path": record.path,
                                    "role": record.roleid,
                                    "auth-id": record.ugid,
                                }),
                                AclUgidType::Group => json!({
                                    "delete": true,
                                    "path": record.path,
                                    "role": record.roleid,
                                    "group": record.ugid,
                                }),
                            };

                            match crate::http_put(url, Some(data)).await {
                                Ok(()) => link.send_reload(),
                                Err(err) => link.show_error("Removing ACL failed", err, true),
                            }
                        });
                    }
                }
                false
            }
        }
    }

    fn main_view(&self, _ctx: &LoadableComponentContext<Self>) -> Html {
        DataTable::new(Self::colmuns(), self.store.clone())
            .class(css::FlexFit)
            .selection(self.selection.clone())
            .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        match view_state {
            ViewState::AddAcl(key) => ctx.props().edit_acl_menu.get(key).map(|(_, dialog)| {
                dialog
                    .clone()
                    .on_done(ctx.link().change_view_callback(|_| None))
                    .into()
            }),
        }
    }
}
