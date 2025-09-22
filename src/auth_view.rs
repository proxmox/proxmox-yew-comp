use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;

use proxmox_client::ApiResponseData;
use pwt::widget::form::{Checkbox, FormContext, TristateBoolean};
use serde_json::Value;
use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::menu::{Menu, MenuButton, MenuItem};

use pwt::widget::{Button, Container, Fa, InputPanel, Toolbar};

use pwt_macros::builder;

use crate::{
    AuthEditLDAP, AuthEditOpenID, EditWindow, LoadableComponent, LoadableComponentContext,
    LoadableComponentLink, LoadableComponentMaster,
};

use crate::common_api_types::BasicRealmInfo;
use crate::percent_encoding::percent_encode_component;

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
    #[prop_or_default]
    openid_base_url: Option<AttrValue>,

    /// Allow to add/edit LDAP entries
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    ldap_base_url: Option<AttrValue>,

    /// Allow to add/edit LDAP entries
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    ad_base_url: Option<AttrValue>,
}

impl Default for AuthView {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthView {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[derive(PartialEq)]
pub enum ViewState {
    AddAd,
    AddLDAP,
    AddOpenID,
    EditOpenID(AttrValue),
    EditLDAP(AttrValue),
    EditAd(AttrValue),
    Sync(BasicRealmInfo),
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

async fn delete_item(base_url: AttrValue, realm: AttrValue) -> Result<(), Error> {
    let url = format!("{base_url}/{}", percent_encode_component(&realm));
    crate::http_delete(&url, None).await?;
    Ok(())
}

async fn sync_realm(
    form_ctx: FormContext,
    link: LoadableComponentLink<ProxmoxAuthView>,
    url: impl Into<String>,
) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();

    let mut remove_vanished = Vec::new();

    for prop in ["acl", "entry", "properties"] {
        let prop_name = format!("remove-vanished-{prop}");
        if data[&prop_name] == Value::Bool(true) {
            remove_vanished.push(prop);
        }

        data[&prop_name] = Value::Null;
    }

    if !remove_vanished.is_empty() {
        data["remove-vanished"] = Value::String(remove_vanished.join(";"));
    }

    let mut new = serde_json::json!({});

    for (param, v) in data.as_object().unwrap().iter() {
        if !v.is_null() {
            new[param] = v.clone();
        }
    }

    match crate::http_post::<String>(url, Some(new)).await {
        Ok(upid) => link.show_task_log(upid, None),
        Err(err) => link.show_error(tr!("Sync Failed"), err, true),
    };

    Ok(())
}

async fn load_realm(url: impl Into<String>) -> Result<ApiResponseData<Value>, Error> {
    let mut response: ApiResponseData<Value> = crate::http_get_full(url, None).await?;

    if let Value::String(sync_default_options) = response.data["sync-defaults-options"].take() {
        let split = sync_default_options.split(",");

        for part in split {
            let mut part = part.split("=");

            match part.next() {
                Some("enable-new") => {
                    response.data["enable-new"] = Value::Bool(part.next() == Some("true"))
                }
                Some("remove-vanished") => {
                    if let Some(part) = part.next() {
                        for vanished_opt in part.split(";") {
                            response.data[&format!("remove-vanished-{vanished_opt}")] =
                                Value::Bool(true)
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(response)
}

impl ProxmoxAuthView {
    fn get_selected_record(&self) -> Option<BasicRealmInfo> {
        let selected_key = self.selection.selected_key();
        let mut selected_record = None;
        if let Some(key) = &selected_key {
            selected_record = self.store.read().lookup_record(key).cloned();
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
            Msg::Redraw => true,
            Msg::Remove => {
                let info = match self.get_selected_record() {
                    Some(info) => info,
                    None => return true,
                };

                let link = ctx.link();
                let base_url = props.base_url.clone();
                link.clone().spawn(async move {
                    if let Err(err) = delete_item(base_url, info.realm.into()).await {
                        link.show_error(tr!("Unable to delete item"), err, true);
                    }
                    link.send_reload();
                });

                false
            }
            Msg::Edit => {
                let info = match self.get_selected_record() {
                    Some(info) => info,
                    None => return true,
                };

                let view = match info.ty.as_str() {
                    "openid" if props.openid_base_url.is_some() => {
                        Some(ViewState::EditOpenID(info.realm.into()))
                    }
                    "ldap" if props.ldap_base_url.is_some() => {
                        Some(ViewState::EditLDAP(info.realm.into()))
                    }
                    "ad" if props.ad_base_url.is_some() => {
                        Some(ViewState::EditAd(info.realm.into()))
                    }
                    _ => return true,
                };

                ctx.link().change_view(view);
                true
            }
            Msg::Sync => {
                let info = match self.get_selected_record() {
                    Some(info) => info,
                    None => return true,
                };

                ctx.link().change_view(Some(ViewState::Sync(info)));
                true
            }
        }
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

        if props.ad_base_url.is_some() {
            add_menu.add_item(
                MenuItem::new(tr!("Active Directory Server"))
                    .icon_class("fa fa-fw fa-address-book-o")
                    .on_select(ctx.link().change_view_callback(|_| Some(ViewState::AddAd))),
            );
        }

        if props.ldap_base_url.is_some() {
            add_menu.add_item(
                MenuItem::new(tr!("LDAP Server"))
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
                move |_: &mut _| link.send_message(Msg::Edit)
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
            ViewState::AddAd => Some(
                AuthEditLDAP::new()
                    .base_url(props.ad_base_url.clone().unwrap())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .ad_realm(true)
                    .into(),
            ),
            ViewState::EditAd(realm) => Some(
                AuthEditLDAP::new()
                    .base_url(props.ad_base_url.clone().unwrap())
                    .realm(realm.clone())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .ad_realm(true)
                    .into(),
            ),

            ViewState::AddLDAP => Some(
                AuthEditLDAP::new()
                    .base_url(props.ldap_base_url.clone().unwrap())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::EditLDAP(realm) => Some(
                AuthEditLDAP::new()
                    .base_url(props.ldap_base_url.clone().unwrap())
                    .realm(realm.clone())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::AddOpenID => Some(
                AuthEditOpenID::new()
                    .base_url(props.openid_base_url.clone().unwrap())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::EditOpenID(realm) => Some(
                AuthEditOpenID::new()
                    .base_url(props.openid_base_url.clone().unwrap())
                    .realm(realm.clone())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::Sync(realm) => {
                let link = ctx.link();
                let url = format!(
                    "{}/{}/sync",
                    ctx.props().base_url,
                    percent_encode_component(&realm.realm)
                );

                let base_url = match realm.ty.as_str() {
                    // unwraps here are safe as the guards ensure the Option is a Some
                    "ldap" if props.ldap_base_url.is_some() => {
                        props.ldap_base_url.as_ref().unwrap()
                    }
                    "ad" if props.ad_base_url.is_some() => props.ad_base_url.as_ref().unwrap(),
                    _ => return None,
                };

                Some(
                    EditWindow::new(tr!("Realm Sync"))
                        .renderer(|_form_ctx| {
                            InputPanel::new()
                                .padding(4)
                                .with_field(tr!("Preview Only"), Checkbox::new().name("dry-run"))
                                .with_field(
                                    tr!("Enable new users"),
                                    TristateBoolean::new()
                                        .name("enable-new")
                                        .null_text(tr!("Default") + " (" + &tr!("Yes") + ")"),
                                )
                                .with_large_custom_child(
                                    Container::new()
                                        .key("remove-vanished-options")
                                        .class("pwt-font-title-medium")
                                        .padding_top(2)
                                        .with_child(tr!("Remove Vanished Options")),
                                )
                                .with_large_field(
                                    tr!("ACLs"),
                                    Checkbox::new()
                                        .name("remove-vanished-acl")
                                        .box_label(tr!("Remove ACLs of vanished users.")),
                                )
                                .with_large_field(
                                    tr!("Entries"),
                                    Checkbox::new()
                                        .name("remove-vanished-entry")
                                        .box_label(tr!("Remove vanished user")),
                                )
                                .with_large_field(
                                    tr!("Properties"),
                                    Checkbox::new()
                                        .name("remove-vanished-properties")
                                        .box_label(tr!("Remove vanished properties")),
                                )
                                .into()
                        })
                        .loader({
                            let url =
                                format!("{base_url}/{}", percent_encode_component(&realm.realm));
                            move || load_realm(url.clone())
                        })
                        .submit_digest(false)
                        .on_close(link.change_view_callback(|_| None))
                        .on_submit(move |form_context| {
                            let link = link.clone();
                            let url = url.clone();
                            sync_realm(form_context, link, url)
                        })
                        .into(),
                )
            }
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
        DataTableColumn::new(tr!("Default"))
            .width("100px")
            .render(|item: &BasicRealmInfo| {
                if item.default.unwrap_or_default() {
                    Fa::new("check").into()
                } else {
                    Fa::new("times").into()
                }
            })
            .justify("center")
            .sorter(|a: &BasicRealmInfo, b: &BasicRealmInfo| {
                a.default.unwrap_or_default().cmp(&b.default.unwrap_or_default())
            })
            .into(),
        DataTableColumn::new("Comment")
            .flex(1)
            .render(|record: &BasicRealmInfo| {
                html!{record.comment.clone().unwrap_or_default()}
            })
            .into(),
    ]);
}

impl From<AuthView> for VNode {
    fn from(val: AuthView) -> Self {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxAuthView>>(Rc::new(val), None);
        VNode::from(comp)
    }
}
