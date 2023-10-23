use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use pwt::props::ExtractPrimaryKey;
use serde::{Serialize, Deserialize};

use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode, Key};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::menu::{Menu, MenuButton, MenuItem};

use pwt::widget::{Button, Toolbar, form};

use pwt_macros::builder;

use crate::{
    LoadableComponent, LoadableComponentContext,
    LoadableComponentMaster,
};

use proxmox_tfa::{TfaType, TypedTfaInfo};

use crate::percent_encoding::percent_encode_component;

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
struct TfaEntry  {
    full_id: String,
    userid: String,
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
}

#[derive(PartialEq)]
pub enum ViewState {
    AddTotp,
    EditTotp(AttrValue),
}

#[doc(hidden)]
pub struct ProxmoxTfaView {
    selection: Selection,
    store: Store<TfaEntry>,
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
            for tfa_user in data  {
                for typed_tfa_info in tfa_user.entries {
                    flat_list.push(TfaEntry {
                        full_id: format!("{}/{}", tfa_user.userid, typed_tfa_info.info.id),
                        userid: tfa_user.userid.clone(),
                        tfa_type: typed_tfa_info.ty,
                        description: typed_tfa_info.info.description,
                        created: typed_tfa_info.info.created,
                        enable: typed_tfa_info.info.enable,
                    });
                }
            }

            flat_list.sort_by(|a: &TfaEntry, b: &TfaEntry| {
                a.userid.cmp(&b.userid).then_with(|| format_tfa_type(a.tfa_type).cmp(&format_tfa_type(b.tfa_type)))
            });
            store.set_data(flat_list);
            Ok(())
        })
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();

        match msg {
            Msg::Redraw => { true }
            Msg::Edit => { true }
        }
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

}


fn format_tfa_type(tfa_type: TfaType) -> String {
    serde_plain::to_string(&tfa_type).unwrap()
}

thread_local! {
    static COLUMNS: Rc<Vec<DataTableHeader<TfaEntry>>> = Rc::new(vec![
        DataTableColumn::new(tr!("User"))
            .width("200px")
            .render(|item: &TfaEntry| {
                html!{item.userid.clone()}
            })
            .sorter(|a: &TfaEntry, b: &TfaEntry| {
                a.userid.cmp(&b.userid)
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
        DataTableColumn::new(tr!("TfaType"))
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
