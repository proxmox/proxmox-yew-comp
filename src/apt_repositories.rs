use std::rc::Rc;
use std::pin::Pin;
use std::cmp::Ordering;
use std::future::Future;
use std::collections::HashMap;

use anyhow::Error;
use serde_json::json;

use yew::virtual_dom::{Key, VComp, VNode};
use yew::html::IntoPropValue;

use pwt::prelude::*;
use pwt::props::ExtractPrimaryKey;
use pwt::state::{Selection, SlabTree, TreeStore};
use pwt::widget::{Container, Button, Toolbar, Tooltip};
use pwt::widget::data_table::{DataTable, DataTableCellRenderArgs, DataTableColumn, DataTableHeader, DataTableHeaderGroup};

use crate::percent_encoding::percent_encode_component;
use crate::{DataViewWindow, LoadableComponent, LoadableComponentContext, LoadableComponentMaster};
use crate::common_api_types::APTUpdateInfo;

use pwt_macros::builder;

use super::apt_api_types::{APTConfiguration, APTRepository, APTRepositoryInfo};

async fn apt_configuration(base_url: AttrValue) -> Result<APTConfiguration, Error> {
    let url = format!("{base_url}/repositories");
    crate::http_get(url, None).await
}

#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct AptRepositories {
    #[prop_or("/nodes/localhost/apt".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,
}

impl AptRepositories {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[derive(Copy, Clone, PartialEq)]
enum Origin {
    Debian,
    Proxmox,
    Other,
}

#[derive(Clone, PartialEq)]
enum TreeEntry {
    Root(Key),
    File {
        key: Key,
        path: String,
        repo_count: usize,
    },
    Repository {
        key: Key,
        path: String,
        index: usize,
        repo: APTRepository,
        origin: Origin,
        warnings: Vec<APTRepositoryInfo>,
    }
}

impl ExtractPrimaryKey for TreeEntry {
    fn extract_key(&self) -> Key {
        match self {
            TreeEntry::Root(key) => key.clone(),
            TreeEntry::File {key, ..} => key.clone(),
            TreeEntry::Repository {key, ..} => key.clone(),
        }
    }
}

fn apt_configuration_to_tree(config: &APTConfiguration) -> SlabTree<TreeEntry> {
    let mut tree = SlabTree::new();

    let mut root = tree.set_root(TreeEntry::Root(Key::from(format!("root"))));
    root.set_expanded(true);

    let mut info_map: HashMap<String, HashMap<usize, Vec<APTRepositoryInfo>>> = HashMap::new();

    for info in &config.infos {
        let inner = info_map.entry(info.path.clone()).or_insert(HashMap::new());
        let entry = inner.entry(info.index).or_insert(Vec::new());
        entry.push(info.clone());
    }

    for file in &config.files {
        let path = match &file.path {
            None => continue, // fixme: WTF?
            Some(path) => path,
        };
        let mut file_node = root.append(TreeEntry::File {
            key: Key::from(format!("file:{path}")),
            path: path.clone(),
            repo_count: file.repositories.len(),
        });

        file_node.set_expanded(true);

        let file_infos = info_map.get(path);

        for (index, repo) in file.repositories.iter().enumerate() {
            let mut origin = Origin::Other;
            let mut warnings = Vec::new();

            if let Some(file_infos) = &file_infos {
                if let Some(list) = file_infos.get(&index) {
                    for info in list {
                        match info.kind.as_str() {
                            "origin" => {
                                origin = match info.message.as_str() {
                                    "Debian" => Origin::Debian,
                                    "Proxmox" => Origin::Proxmox,
                                    _ => Origin::Other,
                                };
                            }
                            "warning" => {
                                warnings.push(info.clone());
                            }
                            _ => {}
                        }
                    }
                }
            }

            file_node.append(TreeEntry::Repository {
                key: Key::from(format!("repo:{path}:{index}")),
                path: path.clone(),
                index,
                repo: repo.clone(),
                origin,
                warnings,
            });
        }

    }

    tree
}

pub enum Msg {
    Refresh,
    ToggleEnable,
}

#[derive(Clone, PartialEq)]
pub enum ViewState {}

pub struct ProxmoxAptRepositories {
    tree_store: TreeStore<TreeEntry>,
    selection: Selection,
    columns: Rc<Vec<DataTableHeader<TreeEntry>>>,
}

impl LoadableComponent for ProxmoxAptRepositories {
    type Properties = AptRepositories;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let tree_store = TreeStore::new().view_root(false);
        let columns = Self::columns(ctx, tree_store.clone());
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::Refresh));

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
            let config = apt_configuration(base_url.clone()).await?;
            let tree = apt_configuration_to_tree(&config);
            tree_store.write().update_root_tree(tree);
            Ok(())
        })
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::Refresh => true,
            Msg::ToggleEnable => {
                let selected_record = match self.selected_record() {
                    Some(record) => record,
                    None => return false,
                };
                match selected_record {
                    TreeEntry::Repository { path, index, repo, ..} => {
                        let param = json!({
                            "path": path,
                            "index": index,
                            "enabled": !repo.enabled,
                        });
                        // fixme: add digest to protect against concurrent changes
                        let url = format!("{}/repositories", props.base_url);
                        let link = ctx.link();
                        wasm_bindgen_futures::spawn_local(async move {
                            match crate::http_post(url, Some(param)).await {
                                 Ok(()) => {
                                    link.send_reload();
                                }
                                Err(err) => {
                                    link.show_error(tr!("API call failed"), err, true);
                                }
                            }
                        });
                    }
                    _ => {}
                }
                false
            }
        }

    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let selected_record = self.selected_record();

        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child({
                let enabled = match selected_record {
                    Some(TreeEntry::Repository {repo, ..}) => Some(repo.enabled),
                    _ => None,
                };
                Button::new(if enabled.unwrap_or(false) { tr!("Disable") } else { tr!("Enable") })
                    .disabled(enabled.is_none())
                    .onclick(ctx.link().callback(|_| Msg::ToggleEnable))
            })
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
            .into()
    }
}

impl From<AptRepositories> for VNode {
    fn from(prop: AptRepositories) -> VNode {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxAptRepositories>>(Rc::new(prop), None);
        VNode::from(comp)
    }
}

impl ProxmoxAptRepositories {

    fn selected_record(&self) -> Option<TreeEntry> {
        let selected_key = self.selection.selected_key();
        match selected_key.as_ref() {
            Some(key) => self.tree_store.read().lookup_node(key).map(|r| r.record().clone()),
            None => None,
        }
    }

    fn columns(
        _ctx: &LoadableComponentContext<Self>,
        store: TreeStore<TreeEntry>,
    ) -> Rc<Vec<DataTableHeader<TreeEntry>>> {
        Rc::new(vec![
            DataTableColumn::new(tr!("Enabled"))
                .render_cell(render_enabled_or_group)
                .tree_column(store.clone())
                .into(),
            DataTableColumn::new(tr!("Types"))
                .width("100px")
                .render(render_types)
                .into(),
            DataTableColumn::new(tr!("URIs"))
                .width("400px")
                .render(render_uris)
                .into(),
            DataTableColumn::new(tr!("Suites"))
                .width("150px")
                .render(render_suites)
                .into(),
            DataTableColumn::new(tr!("Components"))
                .width("200px")
                .render(render_components)
                .into(),
            DataTableColumn::new(tr!("Origin"))
                .width("140px")
                .render(render_origin)
                .into(),
            DataTableColumn::new(tr!("Comment"))
                .flex(1)
                .render(render_comment)
                .into(),
        ])
    }
}

fn render_enabled_or_group(args: &mut DataTableCellRenderArgs<TreeEntry>) -> Html {
    match args.record() {
        TreeEntry::File { path, repo_count, ..} => {
            let text = path.clone() + " (" +
                &tr!("One repository" | "{n} repositories" % *repo_count as u64) + ")";

            args.set_attribute("colspan", "20");
            args.add_class("pwt-bg-color-surface");
            html!{text}
        }
        TreeEntry::Repository { repo, ..} => {
            let icon_class = match repo.enabled {
                true => "fa fa-check",
                false => "fa fa-minus"
            };
            html!{<i class={icon_class}/>}
        }
        _ => html!{},
    }
}

fn render_origin(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { origin, ..} => {
            match origin {
                Origin::Debian => html!{"Debian"},
                Origin::Proxmox => html!{"Proxmox"},
                Origin::Other => html!{"Other"},
            }
        }
        _ => html!{}
    }
}

fn render_comment(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { repo, ..} => {
            html!{&repo.comment}
        }
        _ => html!{}
    }
}

fn render_components(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { repo, ..} => {
            html!{repo.components.join(" ")}
        }
        _ => html!{}
    }
}

fn render_suites(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { repo, warnings, ..} => {
            let mut warn: Vec<String> = warnings
                .iter()
                .filter(|info| matches!(info.property.as_deref(), Some("Suites")))
                .map(|info| info.message.clone())
                .collect();
            if warn.is_empty() {
                html!{repo.suites.join(" ")}
            } else {
                let content = html!{
                    <span class="pwt-color-warning">
                        {repo.suites.join(" ")}
                        <i class="fa fa-fw fa-exclamation-circle"/>
                    </span>
                };
                let title = tr!("Warning" | "Warnings" % warn.len());
                let mut tip = Container::new().with_child(html!{<h4>{title}</h4>});
                for message in warn {
                    tip.add_child(html!{<p>{message}</p>});
                }
                Tooltip::new(content).rich_tip(tip).into()
            }
        }
        _ => html!{}
    }
}

fn render_uris(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { repo, ..} => {
            html!{repo.uris.join(" ")}
        }
        _ => html!{}
    }
}

fn render_types(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { repo, ..} => {
            let text: String = repo.types.iter()
                .map(|t| serde_plain::to_string(t).unwrap()).collect();
            html!{text}
        }
        _ => html!{}
    }
}
