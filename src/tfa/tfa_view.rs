use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use pwt::props::ExtractPrimaryKey;

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::menu::{Menu, MenuButton, MenuItem};
use pwt::widget::{Button, Mask, Toolbar};

use pwt_macros::builder;

use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster};

use proxmox_tfa::{TfaType, TfaUser};

use crate::percent_encoding::percent_encode_component;
use crate::tfa::TfaEdit;

use super::tfa_confirm_remove::TfaConfirmRemove;
use super::{TfaAddRecovery, TfaAddTotp, TfaAddWebauthn};

async fn delete_item(
    base_url: AttrValue,
    user_id: String,
    entry_id: String,
    password: Option<String>,
) -> Result<(), Error> {
    let url = format!(
        "{base_url}/{}/{}",
        percent_encode_component(&user_id),
        percent_encode_component(&entry_id),
    );
    let password = password.map(|password| {
        serde_json::json!({
            "password": password
        })
    });
    crate::http_delete(&url, password).await?;
    Ok(())
}

#[derive(Clone, PartialEq)]
pub(super) struct TfaEntry {
    pub full_id: String,
    pub user_id: String,
    pub entry_id: String,
    pub tfa_type: TfaType,
    pub description: String,
    pub created: i64,
    pub enable: bool,
    pub locked: bool,
}

impl ExtractPrimaryKey for TfaEntry {
    fn extract_key(&self) -> yew::virtual_dom::Key {
        Key::from(self.full_id.clone())
    }
}

#[derive(PartialEq, Properties)]
#[builder]
pub struct TfaView {
    /// Base API path.
    #[prop_or("/access/tfa".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,
}

impl Default for TfaView {
    fn default() -> Self {
        Self::new()
    }
}

impl TfaView {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub enum Msg {
    Redraw,
    Edit,
    Remove(Option<String>),
    RemoveResult(Result<(), Error>),
}

#[derive(PartialEq)]
pub enum ViewState {
    AddTotp,
    AddWebAuthn,
    AddRecoveryKeys,
    Edit(AttrValue, AttrValue),
    Remove,
}

#[doc(hidden)]
pub struct ProxmoxTfaView {
    selection: Selection,
    store: Store<TfaEntry>,
    removing: bool,
}

impl ProxmoxTfaView {
    fn get_selected_record(&self) -> Option<TfaEntry> {
        let selected_key = self.selection.selected_key();
        let mut selected_record = None;
        if let Some(key) = &selected_key {
            selected_record = self.store.read().lookup_record(key).cloned();
        }
        selected_record
    }
}

impl LoadableComponent for ProxmoxTfaView {
    type Properties = TfaView;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let store = Store::new();
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::Redraw));
        Self {
            store,
            selection,
            removing: false,
        }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let props = ctx.props();
        let base_url = props.base_url.clone();
        let store = self.store.clone();
        Box::pin(async move {
            let data: Vec<TfaUser> = crate::http_get(&*base_url, None).await?;
            let now = proxmox_time::epoch_i64();

            let mut flat_list = Vec::new();
            for tfa_user in data {
                let tfa_locked = tfa_user.tfa_locked_until.is_some_and(|t| t > now);
                let totp_locked = tfa_user.totp_locked;
                for typed_tfa_info in tfa_user.entries {
                    flat_list.push(TfaEntry {
                        full_id: format!("{}/{}", tfa_user.userid, typed_tfa_info.info.id),
                        user_id: tfa_user.userid.clone(),
                        entry_id: typed_tfa_info.info.id,
                        tfa_type: typed_tfa_info.ty,
                        description: typed_tfa_info.info.description,
                        created: typed_tfa_info.info.created,
                        enable: typed_tfa_info.info.enable,
                        locked: tfa_locked || typed_tfa_info.ty == TfaType::Totp && totp_locked,
                    });
                }
            }

            flat_list.sort_by(|a: &TfaEntry, b: &TfaEntry| {
                a.user_id
                    .cmp(&b.user_id)
                    .then_with(|| format_tfa_type(a.tfa_type).cmp(&format_tfa_type(b.tfa_type)))
            });
            store.set_data(flat_list);
            Ok(())
        })
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::Redraw => true,
            Msg::Edit => {
                let info = match self.get_selected_record() {
                    Some(info) => info,
                    None => return true,
                };

                if info.tfa_type == TfaType::Recovery {
                    return false;
                }

                ctx.link().change_view(Some(ViewState::Edit(
                    info.user_id.clone().into(),
                    info.entry_id.clone().into(),
                )));

                false
            }
            Msg::Remove(password) => {
                self.removing = true;
                let info = match self.get_selected_record() {
                    Some(info) => info,
                    None => return true,
                };
                // fixme: ask use if he really wants to remove
                let link = ctx.link().clone();
                let base_url = props.base_url.clone();
                link.send_future(async move {
                    Msg::RemoveResult(
                        delete_item(
                            base_url,
                            info.user_id.clone(),
                            info.entry_id.clone(),
                            password,
                        )
                        .await,
                    )
                });

                false
            }
            Msg::RemoveResult(res) => {
                self.removing = false;
                if let Err(err) = res {
                    ctx.link()
                        .show_error(tr!("Unable to delete item"), err, true);
                }
                ctx.link().send_reload();
                false
            }
        }
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let selected_record = self.get_selected_record();
        let remove_disabled = selected_record.is_none();
        let edit_disabled = selected_record
            .as_ref()
            .map(|item| item.tfa_type == TfaType::Recovery)
            .unwrap_or(true);

        let add_menu = Menu::new()
            .with_item(
                MenuItem::new(tr!("TOTP"))
                    .icon_class("fa fa-fw fa-clock-o")
                    .on_select(
                        ctx.link()
                            .change_view_callback(|_| Some(ViewState::AddTotp)),
                    ),
            )
            .with_item(
                MenuItem::new(tr!("WebAuthn"))
                    .icon_class("fa fa-fw fa-shield")
                    .on_select(
                        ctx.link()
                            .change_view_callback(|_| Some(ViewState::AddWebAuthn)),
                    ),
            )
            .with_item(
                MenuItem::new(tr!("Recovery Keys"))
                    .icon_class("fa fa-fw fa-file-text-o")
                    .on_select(
                        ctx.link()
                            .change_view_callback(|_| Some(ViewState::AddRecoveryKeys)),
                    ),
            );

        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(MenuButton::new("Add").show_arrow(true).menu(add_menu))
            .with_spacer()
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(edit_disabled)
                    .onclick(ctx.link().callback(|_| Msg::Edit)),
            )
            .with_child(
                Button::new(tr!("Remove"))
                    .disabled(remove_disabled)
                    .onclick(ctx.link().change_view_callback(|_| Some(ViewState::Remove))),
            )
            .with_flex_spacer()
            .with_child({
                let loading = ctx.loading();
                let link = ctx.link();
                Button::refresh(loading).onclick(move |_| link.send_reload())
            });

        Some(toolbar.into())
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let columns = COLUMNS.with(Rc::clone);
        let view = DataTable::new(columns, self.store.clone())
            .selection(self.selection.clone())
            .class("pwt-flex-fit")
            .on_row_dblclick({
                let link = ctx.link();
                move |_: &mut _| link.send_message(Msg::Edit)
            });
        Mask::new(view).visible(self.removing).into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        let props = ctx.props();
        match view_state {
            ViewState::Remove => Some({
                let info = self.get_selected_record()?;
                TfaConfirmRemove::new(info)
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .on_confirm({
                        let link = ctx.link();
                        move |password| link.send_message(Msg::Remove(password))
                    })
                    .into()
            }),
            ViewState::AddTotp => Some(
                TfaAddTotp::new()
                    .base_url(props.base_url.clone())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::AddWebAuthn => Some(
                TfaAddWebauthn::new()
                    .base_url(props.base_url.clone())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::AddRecoveryKeys => Some(
                TfaAddRecovery::new()
                    .base_url(props.base_url.clone())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::Edit(user_id, entry_id) => Some(
                TfaEdit::new(user_id.clone(), entry_id.clone())
                    .base_url(props.base_url.clone())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
        }
    }
}

fn format_tfa_type(tfa_type: TfaType) -> String {
    serde_plain::to_string(&tfa_type).unwrap()
}

thread_local! {
    static COLUMNS: Rc<Vec<DataTableHeader<TfaEntry>>> = Rc::new(vec![
        DataTableColumn::new(tr!("User"))
            .width("200px")
            .render(|item: &TfaEntry| {
                html!{item.user_id.clone()}
            })
            .sorter(|a: &TfaEntry, b: &TfaEntry| {
                a.user_id.cmp(&b.user_id)
            })
            .into(),
        DataTableColumn::new(tr!("Enabled"))
            .width("100px")
            .justify("center")
            .render({
                let yes_text = tr!("Yes");
                let no_text = tr!("No");
                let locked_text = tr!("Locked");

                move |item: &TfaEntry| html!{
                    {
                        match (item.locked, item.enable) {
                            (true, _) => &locked_text,
                            (_, true) => &yes_text,
                            (_, false) => &no_text,
                        }
                    }
                }
            })
            .sorter(|a: &TfaEntry, b: &TfaEntry| {
                a.enable.cmp(&b.enable)
            })
            .into(),
        DataTableColumn::new(tr!("TFA Type"))
            .width("100px")
            .render(|item: &TfaEntry| html!{
                format_tfa_type(item.tfa_type)
            })
            .sorter(|a: &TfaEntry, b: &TfaEntry| {
                let a = format_tfa_type(a.tfa_type);
                let b = format_tfa_type(b.tfa_type);
                a.cmp(&b)
            })
        .into(),
        DataTableColumn::new(tr!("Created"))
            .width("170px")
            .render(|item: &TfaEntry| html!{
                crate::utils::render_epoch(item.created)
            })
            .sorter(|a: &TfaEntry, b: &TfaEntry| {
                a.created.cmp(&b.created)
            })
            .into(),
        DataTableColumn::new(tr!("Description"))
            .flex(1)
            .render(|item: &TfaEntry| html! {
                item.description.clone()
            })
            .into(),
    ]);
}

impl From<TfaView> for VNode {
    fn from(val: TfaView) -> Self {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxTfaView>>(Rc::new(val), None);
        VNode::from(comp)
    }
}
