use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use pwt::props::ExtractPrimaryKey;
use pwt::widget::Fa;
use serde_json::{json, Value};

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{SlabTree, SlabTreeNodeMut, TreeStore};
use pwt::widget::data_table::{
    DataTable, DataTableCellRenderArgs, DataTableColumn, DataTableHeader,
};

use pwt_macros::builder;

use crate::{http_get, LoadableComponent, LoadableComponentMaster};

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct PermissionPanel {
    #[prop_or("/access/permissions".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,

    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    #[prop_or_default]
    pub auth_id: Option<AttrValue>,
}

impl Default for PermissionPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionPanel {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}
pub struct ProxmoxPermissionPanel {
    store: TreeStore<PermissionInfo>,
    columns: Rc<Vec<DataTableHeader<PermissionInfo>>>,
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum PermissionInfo {
    Permission(String, String, bool),
    Path(String, String),
}

impl ExtractPrimaryKey for PermissionInfo {
    fn extract_key(&self) -> Key {
        Key::from(match self {
            PermissionInfo::Path(path, _) => path.clone(),
            PermissionInfo::Permission(path, _, _) => path.clone(),
        })
    }
}

fn insert_node(
    mut node: SlabTreeNodeMut<'_, PermissionInfo>,
    components: &[&str],
    perm_map: HashMap<String, bool>,
) {
    let path = match node.record() {
        PermissionInfo::Path(path, _) => path.clone(),
        _ => unreachable!(),
    };

    if components.is_empty() {
        for (perm, propagate) in perm_map {
            node.append(PermissionInfo::Permission(
                format!("{perm}|{path}"),
                perm.clone(),
                propagate,
            ));
        }
    } else {
        let component = components[0];
        let components = &components[1..];

        if let Some(child) = node.children_mut().find(|c| match c.record() {
            PermissionInfo::Path(_, name) if name == component => true,
            _ => false,
        }) {
            insert_node(child, components, perm_map);
        } else {
            let child_path = if path == "/" {
                format!("/{component}")
            } else {
                format!("{path}/{component}")
            };
            let record = PermissionInfo::Path(child_path, component.to_owned());
            let child = node.append(record);
            insert_node(child, components, perm_map);
        }
    }
}

impl LoadableComponent for ProxmoxPermissionPanel {
    type Properties = PermissionPanel;
    type Message = ();
    type ViewState = ();

    fn create(_ctx: &crate::LoadableComponentContext<Self>) -> Self {
        let store = TreeStore::new();
        let columns = Rc::new(columns(&store));
        Self { store, columns }
    }

    fn load(
        &self,
        ctx: &crate::LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let props = ctx.props();
        let base_url = props.base_url.clone();
        let args: Option<Value> = props
            .auth_id
            .as_ref()
            .map(|auth_id| json!({ "auth-id": **auth_id}));
        let store = self.store.clone();

        Box::pin(async move {
            let data: HashMap<String, HashMap<String, bool>> = http_get(&*base_url, args).await?;
            let mut tree: SlabTree<PermissionInfo> = SlabTree::new();
            tree.set_root(PermissionInfo::Path(String::from("/"), String::from("/")));
            let mut root = tree.root_mut().unwrap();
            root.set_expanded(true);

            for (path, perm_map) in data {
                let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();
                let root = tree.root_mut().unwrap();
                insert_node(root, &components, perm_map);
            }

            tree.sort(true);

            store.set_data(tree);
            Ok(())
        })
    }

    fn main_view(&self, _ctx: &crate::LoadableComponentContext<Self>) -> Html {
        DataTable::new(Rc::clone(&self.columns), self.store.clone())
            .class("pwt-flex-fit")
            .into()
    }
}

fn columns(store: &TreeStore<PermissionInfo>) -> Vec<DataTableHeader<PermissionInfo>> {
    vec![
        DataTableColumn::new(tr!("Path") + "/" + &tr!("Permission"))
            .flex(1)
            .tree_column(store.clone())
            .render_cell(move |args: &mut DataTableCellRenderArgs<PermissionInfo>| {
                let (icon_class, text) = match args.record() {
                    PermissionInfo::Path(path, _name) => ("folder-o", path.clone()),
                    PermissionInfo::Permission(_path, perm, _propagate) => ("unlock", perm.clone()),
                };
                let icon = Fa::new(icon_class).fixed_width().padding_end(2);
                html! {<>{icon} {text}</>}
            })
            .into(),
        DataTableColumn::new(tr!("Propagate"))
            .render({
                let yes_text = tr!("Yes");
                let no_text = tr!("No");
                move |record: &PermissionInfo| {
                    let text = match record {
                        PermissionInfo::Path(_path, _name) => "",
                        PermissionInfo::Permission(_path, _perm, propagate) => {
                            if *propagate {
                                &yes_text
                            } else {
                                &no_text
                            }
                        }
                    };
                    html! {text}
                }
            })
            .into(),
    ]
}

impl From<PermissionPanel> for VNode {
    fn from(val: PermissionPanel) -> Self {
        let comp =
            VComp::new::<LoadableComponentMaster<ProxmoxPermissionPanel>>(Rc::new(val), None);
        VNode::from(comp)
    }
}
