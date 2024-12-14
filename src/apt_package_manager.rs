use std::cmp::Ordering;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::ExtractPrimaryKey;
use pwt::state::{Selection, SlabTree, TreeStore};
use pwt::widget::data_table::{
    DataTable, DataTableCellRenderArgs, DataTableColumn, DataTableHeader, DataTableHeaderGroup,
};
use pwt::widget::{Button, Container, Toolbar, Tooltip};

use crate::common_api_types::APTUpdateInfo;
use crate::percent_encoding::percent_encode_component;
use crate::{
    DataViewWindow, LoadableComponent, LoadableComponentContext, LoadableComponentMaster, XTermJs,
};

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

    /// Enable the upgrade button
    #[prop_or_default]
    #[builder]
    pub enable_upgrade: bool,
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
                let origin_info = OriginInfo {
                    name: Key::from(info.origin.clone()),
                    count: 1,
                };
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
            origin_node.append(TreeEntry::Package(
                Key::from(package.package.clone()),
                package,
            ));
        }
    }

    tree
}

#[derive(Clone, PartialEq)]
pub enum ViewState {
    ShowChangelog(String),
}

pub struct ProxmoxAptPackageManager {
    tree_store: TreeStore<TreeEntry>,
    selection: Selection,
    columns: Rc<Vec<DataTableHeader<TreeEntry>>>,
}

impl LoadableComponent for ProxmoxAptPackageManager {
    type Properties = AptPackageManager;
    type Message = ();
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let tree_store = TreeStore::new().view_root(false);
        let columns = Self::columns(ctx, tree_store.clone());
        let selection = Selection::new().on_select(ctx.link().callback(|_| ()));

        Self {
            tree_store,
            selection,
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

        let selected_key = self.selection.selected_key();
        let selected_record = match selected_key.as_ref() {
            Some(key) => self
                .tree_store
                .read()
                .lookup_node(key)
                .map(|r| r.record().clone()),
            None => None,
        };
        let selected_package = match selected_record {
            Some(TreeEntry::Package(_, info)) => Some(info.package.clone()),
            _ => None,
        };

        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(Button::new(tr!("Refresh")).onclick({
                let link = ctx.link();
                let command = format!("{}/update", props.base_url);
                move |_| link.start_task(&command, None, false)
            }))
            .with_child(
                Button::new(tr!("Upgrade"))
                    .disabled(!props.enable_upgrade)
                    .onclick(|_| {
                        XTermJs::open_xterm_js_viewer(
                            crate::ConsoleType::UpgradeShell,
                            "localhost",
                        );
                    }),
            )
            .with_child(
                Button::new(tr!("Changelog"))
                    .disabled(selected_package.is_none())
                    .onclick({
                        let link = ctx.link();
                        let view = selected_package
                            .as_ref()
                            .map(|p| ViewState::ShowChangelog(p.clone()));
                        move |_| link.change_view(view.clone())
                    }),
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

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        match view_state {
            ViewState::ShowChangelog(package) => {
                Some(self.create_show_changelog_dialog(ctx, &package))
            }
        }
    }
}

impl From<AptPackageManager> for VNode {
    fn from(prop: AptPackageManager) -> VNode {
        let comp =
            VComp::new::<LoadableComponentMaster<ProxmoxAptPackageManager>>(Rc::new(prop), None);
        VNode::from(comp)
    }
}

impl ProxmoxAptPackageManager {
    fn create_show_changelog_dialog(
        &self,
        ctx: &LoadableComponentContext<Self>,
        package: &str,
    ) -> Html {
        let props = ctx.props().clone();
        let url = format!(
            "{}/changelog?name={}",
            props.base_url,
            percent_encode_component(package),
        );

        DataViewWindow::<String>::new(tr!("Changelog") + ": " + package)
            .width(640)
            .height(600)
            .resizable(true)
            .on_done(ctx.link().change_view_callback(|_| None))
            .loader(url)
            .renderer(|description: &String| {
                let mut panel = Container::from_tag("pre")
                    .padding(2)
                    .class("pwt-flex-fit pwt-monospace");

                if let Some((title, body)) = description.split_once("\n") {
                    panel.add_child(html! {<h6>{title}</h6>});
                    panel.add_child(body);
                } else {
                    panel.add_child(description);
                }
                panel.into()
            })
            .into()
    }

    fn columns(
        _ctx: &LoadableComponentContext<Self>,
        store: TreeStore<TreeEntry>,
    ) -> Rc<Vec<DataTableHeader<TreeEntry>>> {
        Rc::new(vec![
            DataTableColumn::new(tr!("Package"))
                .width("350px")
                .render_cell(render_tree_node)
                .tree_column(Some(store.clone()))
                .sorter(tree_entry_ordering)
                .sort_order(true)
                .into(),
            DataTableHeaderGroup::new(tr!("Version"))
                .with_child(DataTableColumn::new(tr!("current")).width("120px").render(
                    |entry: &_| match entry {
                        TreeEntry::Package(_, info) => html! {&info.old_version},
                        _ => html! {},
                    },
                ))
                .with_child(
                    DataTableColumn::new(tr!("new")).width("120px").render(
                        |entry: &_| match entry {
                            TreeEntry::Package(_, info) => html! {&info.version},
                            _ => html! {},
                        },
                    ),
                )
                .into(),
            DataTableColumn::new(tr!("Description"))
                .flex(1)
                .render(render_description)
                .into(),
        ])
    }
}

fn render_description(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Package(_, info) => {
            if let Some((title, body)) = info.description.split_once("\n") {
                let title = html! {<h3>{title}</h3>};
                Tooltip::new(html! {&info.title})
                    .rich_tip(html! {<pre class="pwt-monospace">{title}{body}</pre>})
                    .into()
            } else {
                html! {<pre class="pwt-monospace">{&info.description}</pre>}
            }
        }
        _ => html! {},
    }
}

fn render_tree_node(args: &mut DataTableCellRenderArgs<TreeEntry>) -> Html {
    let record = args.record();
    match record {
        TreeEntry::Root(_) => html! {"Packages"}, // not visible
        TreeEntry::Origin(info) => {
            let text = tr!("Origin")
                + ": "
                + &*info.name
                + " ("
                + &tr!("One item" | "{} items" % info.count)
                + ")";
            args.add_class("pwt-bg-color-surface");
            args.set_attribute("colspan", "20");
            html! {<span class="pwt-text-truncate">{text}</span>}
        }
        TreeEntry::Package(_, info) => {
            html! {<span class="pwt-text-truncate">{&info.package}</span>}
        }
    }
}
