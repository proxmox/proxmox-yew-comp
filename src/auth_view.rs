use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;

use pwt::widget::form::{Field, Form, FormContext};

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{PersistentState, Selection, Store};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader,
};
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

#[doc(hidden)]
pub struct ProxmoxAuthView {
    selection: Selection,
    store: Store<BasicRealmInfo>,
}

impl LoadableComponent for ProxmoxAuthView {
    type Properties = AuthView;
    type Message = ();
    type ViewState = ();

    fn create(_ctx: &LoadableComponentContext<Self>) -> Self {
        let store = Store::new();
        let selection = Selection::new();
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
