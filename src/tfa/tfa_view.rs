use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use pwt::props::ExtractPrimaryKey;
use serde::{Deserialize, Serialize};

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::menu::{Menu, MenuButton, MenuItem};
use pwt::widget::{Button, Toolbar};

use pwt_macros::builder;

use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster};

use proxmox_tfa::{TfaType, TypedTfaInfo};

use crate::percent_encoding::percent_encode_component;
use crate::tfa::TfaEdit;

use super::{TfaAddRecovery, TfaAddTotp};

async fn delete_item(base_url: AttrValue, user_id: String, entry_id: String) -> Result<(), Error> {
    let url = format!(
        "{base_url}/{}/{}",
        percent_encode_component(&user_id),
        percent_encode_component(&entry_id),
    );
    let _ = crate::http_delete(&url, None).await?;
    Ok(())
}

// fixme: use proxmox_tfa::api::methods::TfaUser;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct TfaUser {
    /// The user this entry belongs to.
    pub userid: String,

    /// TFA entries.
    pub entries: Vec<TypedTfaInfo>,
}

#[derive(Clone, PartialEq)]
struct TfaEntry {
    full_id: String,
    user_id: String,
    entry_id: String,
    tfa_type: TfaType,
    description: String,
    created: i64,
    enable: bool,
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

impl TfaView {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub enum Msg {
    Redraw,
    Edit,
    Remove,
}

#[derive(PartialEq)]
pub enum ViewState {
    AddTotp,
    AddWebAuthn,
    AddRecoveryKeys,
    Edit(AttrValue, AttrValue),
}

#[doc(hidden)]
pub struct ProxmoxTfaView {
    selection: Selection,
    store: Store<TfaEntry>,
}

impl ProxmoxTfaView {
    fn get_selected_record(&self) -> Option<TfaEntry> {
        let selected_key = self.selection.selected_key();
        let mut selected_record = None;
        if let Some(key) = &selected_key {
            selected_record = self.store.read().lookup_record(key).map(|r| r.clone());
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
            let data: Vec<TfaUser> = crate::http_get(&*base_url, None).await?;

            let mut flat_list = Vec::new();
            for tfa_user in data {
                for typed_tfa_info in tfa_user.entries {
                    flat_list.push(TfaEntry {
                        full_id: format!("{}/{}", tfa_user.userid, typed_tfa_info.info.id),
                        user_id: tfa_user.userid.clone(),
                        entry_id: typed_tfa_info.info.id,
                        tfa_type: typed_tfa_info.ty,
                        description: typed_tfa_info.info.description,
                        created: typed_tfa_info.info.created,
                        enable: typed_tfa_info.info.enable,
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
            Msg::Remove => {
                let info = match self.get_selected_record() {
                    Some(info) => info,
                    None => return true,
                };
                // fixme: ask use if he really wants to remove
                let link = ctx.link().clone();
                let base_url = props.base_url.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(err) =
                        delete_item(base_url, info.user_id.clone(), info.entry_id.clone()).await
                    {
                        link.show_error(tr!("Unable to delete item"), err, true);
                    }
                    link.send_reload();
                });

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
                    .onclick(ctx.link().callback(|_| Msg::Remove)),
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
            ViewState::AddTotp => Some(
                TfaAddTotp::new()
                    .base_url(props.base_url.clone())
                    .on_close(ctx.link().change_view_callback(|_| None))
                    .into(),
            ),
            ViewState::AddWebAuthn => None,
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

                move |item: &TfaEntry| html!{
                    {
                        match item.enable {
                            true => &yes_text,
                            false => &no_text,
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

impl Into<VNode> for TfaView {
    fn into(self) -> VNode {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxTfaView>>(Rc::new(self), None);
        VNode::from(comp)
    }
}
