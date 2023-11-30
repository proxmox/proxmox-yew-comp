use std::rc::Rc;
use std::pin::Pin;
use std::cmp::Ordering;
use std::future::Future;
use std::collections::HashMap;

use anyhow::Error;

use yew::virtual_dom::{Key, VComp, VNode};
use yew::html::IntoPropValue;

use pwt::prelude::*;
use pwt::props::ExtractPrimaryKey;
use pwt::state::{Selection, SlabTree, TreeStore};
use pwt::widget::{Button, Toolbar};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader, DataTableHeaderGroup};

use crate::percent_encoding::percent_encode_component;
use crate::{
    EditWindow, LoadableComponent, LoadableComponentContext, LoadableComponentLink,
    LoadableComponentMaster,
};

use crate::common_api_types::APTUpdateInfo;

use pwt_macros::builder;

// fixme: add Upgrade button (opens xtermjs)


async fn list_updates(base_url: AttrValue) -> Result<Vec<APTUpdateInfo>, Error> {
    let url = format!("{base_url}/update");
    crate::http_get(url, None).await
}

#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct AptPackageManager {
    #[prop_or("/nodes/localhost/apt".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,
}

impl AptPackageManager {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[derive(Clone, PartialEq)]
struct OriginInfo {
    name: Key,
    count: usize,
}

#[derive(Clone, PartialEq)]
enum TreeEntry {
    Root(Key),
    Origin(OriginInfo),
    Package(Key, APTUpdateInfo),
}

impl ExtractPrimaryKey for TreeEntry {
    fn extract_key(&self) -> Key {
        match self {
            TreeEntry::Root(key) => key.clone(),
            TreeEntry::Origin(info) => info.name.clone(),
            TreeEntry::Package(key, _) => key.clone(),
        }
    }
}
fn tree_entry_ordering(a: &TreeEntry, b: &TreeEntry) -> Ordering {
    match (a, b) {
        (TreeEntry::Origin(a), TreeEntry::Origin(b)) => a.name.cmp(&b.name),
        (TreeEntry::Package(_, a), TreeEntry::Package(_, b)) => a.package.cmp(&b.package),
        (_, _) => Ordering::Equal,
    }
}

fn update_list_to_tree(updates: &[APTUpdateInfo]) -> SlabTree<TreeEntry> {
    let mut tree = SlabTree::new();

    let mut root = tree.set_root(TreeEntry::Root(Key::from(format!("root"))));
    root.set_expanded(true);

    let mut origin_map = HashMap::new();

    for info in updates {
        match origin_map.get_mut(&info.origin) {
            None => {
                let origin_info = OriginInfo { name: Key::from(info.origin.clone()), count: 1};
                let package_list = vec![info.clone()];
                origin_map.insert(info.origin.clone(), (origin_info, package_list));
            }
            Some((origin_info, package_list)) => {
                origin_info.count += 1;
                package_list.push(info.clone());
            }
        }
    }

    for (_origin, (origin_info, package_list)) in origin_map.into_iter() {
        let mut origin_node = root.append(TreeEntry::Origin(origin_info));
        origin_node.set_expanded(true);
        for package in package_list.into_iter() {
            origin_node.append(TreeEntry::Package(Key::from(package.package.clone()), package));
        }
    }

    tree
}

pub enum Msg {}

pub struct ProxmoxAptPackageManager {
    tree_store: TreeStore<TreeEntry>,
    selection: Selection,
    columns: Rc<Vec<DataTableHeader<TreeEntry>>>,
}

impl LoadableComponent for ProxmoxAptPackageManager {
    type Properties = AptPackageManager;
    type Message = Msg;
    type ViewState = ();

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let props = ctx.props();
        let tree_store = TreeStore::new().view_root(false);

        let columns = Self::columns(ctx, tree_store.clone());

        Self {
            tree_store,
            selection: Selection::new(),
            columns,
        }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let props = ctx.props();
        let base_url = props.base_url.clone();
        let tree_store = self.tree_store.clone();
        Box::pin(async move {
            let updates = list_updates(base_url.clone()).await?;
            let tree = update_list_to_tree(&updates);
            tree_store.write().update_root_tree(tree);
            Ok(())
        })
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let props = ctx.props();
        let link = ctx.link();

        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                Button::new(tr!("Refresh"))
                    .onclick({
                        let link = ctx.link();
                        let command = format!("{}/update", props.base_url);
                        move |_| link.start_task(&command, None, false)
                    })
            )
            .with_flex_spacer()
            .with_child({
                let loading = ctx.loading();
                let link = ctx.link();
                Button::refresh(loading).onclick(move |_| link.send_reload())
            });

        Some(toolbar.into())
    }

    fn main_view(&self, _ctx: &LoadableComponentContext<Self>) -> Html {
        DataTable::new(self.columns.clone(), self.tree_store.clone())
            .selection(self.selection.clone())
            .class("pwt-flex-fit")
            .striped(false)
            .borderless(true)
            .into()
    }
}

impl From<AptPackageManager> for VNode {
    fn from(prop: AptPackageManager) -> VNode {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxAptPackageManager>>(Rc::new(prop), None);
        VNode::from(comp)
    }
}

impl ProxmoxAptPackageManager {
    fn columns(
        ctx: &LoadableComponentContext<Self>,
        store: TreeStore<TreeEntry>,
    ) -> Rc<Vec<DataTableHeader<TreeEntry>>> {
        Rc::new(vec![
            DataTableColumn::new(tr!("Package"))
                .width("350px")
                .render(render_tree_node)
                .tree_column(Some(store.clone()))
                .sorter(tree_entry_ordering)
                .sort_order(true)
                .into(),
            DataTableHeaderGroup::new(tr!("Version"))
                .with_child(
                    DataTableColumn::new(tr!("current"))
                        .width("120px")
                        .render(|entry: &_| match entry {
                            TreeEntry::Package(_, info) => html!{&info.old_version},
                            _ => html!{},
                        })
                )
                .with_child(
                    DataTableColumn::new(tr!("new"))
                        .width("120px")
                        .render(|entry: &_| match entry {
                            TreeEntry::Package(_, info) => html!{&info.version},
                            _ => html!{},
                        })
                )
                .into(),
            DataTableColumn::new(tr!("Description"))
                .flex(1)
                .render(render_desdcription)
                .into(),
        ])
    }
}

fn render_desdcription(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Package(_, info) => html!{&info.title},
        _ => html!{},
    }
}

fn render_tree_node(record: &TreeEntry) -> Html {
    let (class, content): (Option<String>, String) = match record {
        TreeEntry::Root(_) => (None, String::from("Root")), // not visible
        TreeEntry::Origin(info) => {
            (
                None,
                tr!("Origin") + ": " + &*info.name + " " + &tr!("One item" | "{} items" % info.count)
            )
        }
        TreeEntry::Package(_, info) => {
            (
                None,
                info.package.clone(),
            )
        }
    };
    html! {<><i {class}></i><span class="pwt-text-truncate">{content}</span></>}
}
