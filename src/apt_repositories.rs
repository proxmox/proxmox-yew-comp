use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use pwt::css::AlignItems;
use pwt::widget::form::{Combobox, FormContext, ValidateFn};
use serde_json::{json, Value};

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::ExtractPrimaryKey;
use pwt::state::{Selection, SlabTree, Store, TreeStore};
use pwt::widget::data_table::{
    DataTable, DataTableCellRenderArgs, DataTableColumn, DataTableHeader,
};
use pwt::widget::{Button, Column, Container, Fa, Row, Toolbar, Tooltip};

use crate::{
    EditWindow, ExistingProduct, LoadableComponent, LoadableComponentContext,
    LoadableComponentMaster, ProjectInfo, SubscriptionAlert,
};

use pwt_macros::builder;

use proxmox_apt_api_types::{
    APTRepositoriesResult, APTRepository, APTRepositoryHandle, APTRepositoryInfo,
    APTRepositoryPackageType, APTStandardRepository,
};

async fn apt_configuration(base_url: AttrValue) -> Result<APTRepositoriesResult, Error> {
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
enum Status {
    Ok,
    Error,
    Warning,
}

#[derive(Clone, PartialEq)]
struct StatusLine {
    status: Status,
    message: Key,
}

impl StatusLine {
    fn ok(msg: impl Into<String>) -> Self {
        StatusLine {
            status: Status::Ok,
            message: Key::from(msg.into()),
        }
    }
    fn warning(msg: impl Into<String>) -> Self {
        StatusLine {
            status: Status::Warning,
            message: Key::from(msg.into()),
        }
    }
    fn error(msg: impl Into<String>) -> Self {
        StatusLine {
            status: Status::Error,
            message: Key::from(msg.into()),
        }
    }
}

impl ExtractPrimaryKey for StatusLine {
    fn extract_key(&self) -> Key {
        self.message.clone()
    }
}

// Note: this should implement the same logic we have in APTRepositories.js
fn update_status_store(
    status_store: &Store<StatusLine>,
    config: &APTRepositoriesResult,
    standard_repos: &HashMap<String, APTStandardRepository>,
    active_subscription: bool,
) {
    let mut list = Vec::new();

    for error in &config.errors {
        list.push(StatusLine::error(format!(
            "{} - {}",
            error.path, error.error
        )));
    }

    let mut has_enterprise = false;
    let mut has_no_subscription = false;
    let mut has_test = false;
    let mut has_ceph_enterprise = false;
    let mut has_ceph_no_subscription = false;
    let mut has_ceph_test = false;

    for repo in standard_repos.values() {
        if repo.status != Some(true) {
            continue;
        }
        use APTRepositoryHandle::*;
        match repo.handle {
            CephQuincyEnterprise | CephReefEnterprise => has_ceph_enterprise = true,
            CephQuincyNoSubscription | CephReefNoSubscription => has_ceph_no_subscription = true,
            CephQuincyTest | CephReefTest => has_ceph_test = true,
            Enterprise => has_enterprise = true,
            NoSubscription => has_no_subscription = true,
            Test => has_test = true,
        }
    }

    let product = ExistingProduct::PBS; // fixme

    if !(has_enterprise | has_no_subscription | has_test) {
        list.push(StatusLine::error(tr!(
            "No {0} repository is enabled, you do not get any updates!",
            product.project_text()
        )));
    } else {
        if config.errors.is_empty() {
            // just avoid that we show "get updates"
            if has_test || has_no_subscription {
                list.push(StatusLine::ok(tr!(
                    "You get updates for {0}",
                    product.project_text()
                )));
            } else if has_enterprise && active_subscription {
                list.push(StatusLine::ok(tr!(
                    "You get supported updates for {0}",
                    product.project_text()
                )));
            }
        }
    }

    let mut enabled_repos: HashSet<(&str, usize)> = HashSet::new();
    for file in &config.files {
        for (index, repo) in file.repositories.iter().enumerate() {
            if repo.enabled {
                if let Some(path) = &file.path {
                    enabled_repos.insert((path, index));
                }
            }
        }
    }

    let mut mixed_suites = false;
    let mut check_mixed_suites = false;

    let mut ignore_pre_upgrade_warning: HashSet<(&str, usize)> = HashSet::new();
    let mut controlled_origin: HashSet<(&str, usize)> = HashSet::new();
    for info in &config.infos {
        if info.kind == "ignore-pre-upgrade-warning" {
            ignore_pre_upgrade_warning.insert((&info.path, info.index));
            check_mixed_suites = true;
        }
        if info.kind == "origin" {
            if info.message == "Debian" || info.message == "Proxmox" {
                controlled_origin.insert((&info.path, info.index));
            }
        }
    }

    let mut suites_warning = false;
    for info in &config.infos {
        if info.kind == "warning" && info.property.as_deref() == Some("Suites") {
            if enabled_repos.contains(&(&info.path, info.index)) {
                suites_warning = true;
                break;
            }
        }
    }

    if suites_warning {
        list.push(StatusLine::warning(tr!("Some suites are misconfigured")));
    }

    for file in &config.files {
        if let Some(path) = &file.path {
            for (index, repo) in file.repositories.iter().enumerate() {
                if check_mixed_suites
                    && repo.enabled
                    && repo.types.contains(&APTRepositoryPackageType::Deb)
                    && controlled_origin.contains(&(&path, index))
                {
                    mixed_suites = true;
                }
            }
        }
    }

    if mixed_suites {
        list.push(StatusLine::warning(tr!(
            "Detected mixed suites before upgrade"
        )));
    }

    // production ready check
    if has_enterprise && !active_subscription {
        list.push(StatusLine::warning(tr!(
            "The {0}enterprise repository is enabled, but there is no active subscription!",
            product.project_text() + " ",
        )));
    }

    if has_no_subscription {
        list.push(StatusLine::warning(tr!(
            "The {0}no-subscription{1} repository is not recommended for production use!",
            product.project_text() + " ",
            "",
        )));
    }

    if has_test {
        list.push(StatusLine::warning(tr!(
            "The {0}test repository may pull in unstable updates and is not recommended for production use!",
            product.project_text() + " ",
        )));
    }

    // check Ceph repositories
    if has_ceph_enterprise && !active_subscription {
        list.push(StatusLine::warning(tr!(
            "The {0}enterprise repository is enabled, but there is no active subscription!",
            "Ceph ",
        )));
    }

    if has_ceph_no_subscription {
        list.push(StatusLine::warning(tr!(
            "The {0}no-subscription{1} repository is not recommended for production use!",
            "Ceph ",
            "/main", // TODO drop alternate 'main' name when no longer relevant
        )));
    }

    if has_ceph_test {
        list.push(StatusLine::warning(tr!(
            "The {0}test repository may pull in unstable updates and is not recommended for production use!",
            "Ceph ",
        )));
    }

    if !config.errors.is_empty() {
        list.push(StatusLine::error(tr!(
            "Fatal parsing error for at least one repository"
        )));
    }

    if list.iter().find(|l| l.status != Status::Ok).is_none() {
        list.push(StatusLine::ok(tr!(
            "All OK, you have production-ready repositories configured!"
        )));
    }

    status_store.write().set_data(list);
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
    },
}

impl ExtractPrimaryKey for TreeEntry {
    fn extract_key(&self) -> Key {
        match self {
            TreeEntry::Root(key) => key.clone(),
            TreeEntry::File { key, .. } => key.clone(),
            TreeEntry::Repository { key, .. } => key.clone(),
        }
    }
}

fn apt_configuration_to_tree(config: &APTRepositoriesResult) -> SlabTree<TreeEntry> {
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
    UpdateStatus(APTRepositoriesResult),
    SubscriptionInfo(Result<Value, Error>),
}

#[derive(Clone, PartialEq)]
pub enum ViewState {
    AddRespository,
    ShowSubscription, // show subscription dialog
}

pub struct ProxmoxAptRepositories {
    tree_store: TreeStore<TreeEntry>,
    selection: Selection,
    columns: Rc<Vec<DataTableHeader<TreeEntry>>>,
    config: Option<APTRepositoriesResult>,
    standard_repos: HashMap<String, APTStandardRepository>,
    validate_standard_repo: ValidateFn<(String, Store<AttrValue>)>,
    subscription_status: Option<Result<Value, Error>>,
    status_store: Store<StatusLine>,
    status_columns: Rc<Vec<DataTableHeader<StatusLine>>>,
}

impl LoadableComponent for ProxmoxAptRepositories {
    type Properties = AptRepositories;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let tree_store = TreeStore::new().view_root(false);
        let columns = Self::columns(ctx, tree_store.clone());
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::Refresh));
        let status_columns = Self::status_columns(ctx);

        let link = ctx.link();
        wasm_bindgen_futures::spawn_local(async move {
            let data = crate::http_get("/nodes/localhost/subscription", None).await;
            link.send_message(Msg::SubscriptionInfo(data));
        });

        Self {
            tree_store,
            selection,
            columns,
            config: None,
            standard_repos: HashMap::new(),
            validate_standard_repo: ValidateFn::new(|(_, _): &_| Ok(())),
            subscription_status: None,
            status_store: Store::new(),
            status_columns,
        }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let props = ctx.props();
        let base_url = props.base_url.clone();
        let tree_store = self.tree_store.clone();
        let link = ctx.link();

        Box::pin(async move {
            let config = apt_configuration(base_url.clone()).await?;
            let tree = apt_configuration_to_tree(&config);
            tree_store.write().update_root_tree(tree);
            link.send_message(Msg::UpdateStatus(config));
            Ok(())
        })
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::Refresh => true,
            Msg::SubscriptionInfo(status) => {
                self.subscription_status = Some(status);
                if let Some(config) = &self.config {
                    let active_subscription = self.active_subscription();
                    update_status_store(
                        &self.status_store,
                        &config,
                        &self.standard_repos,
                        active_subscription,
                    );
                } else {
                    self.status_store.clear();
                }
                true
            }
            Msg::UpdateStatus(config) => {
                let standard_repos: HashMap<String, APTStandardRepository> = config
                    .standard_repos
                    .iter()
                    .map(|item| (serde_plain::to_string(&item.handle).unwrap(), item.clone()))
                    .collect();

                let active_subscription = self.active_subscription();
                update_status_store(
                    &self.status_store,
                    &config,
                    &standard_repos,
                    active_subscription,
                );

                self.config = Some(config);
                self.standard_repos = standard_repos.clone();

                self.validate_standard_repo = ValidateFn::new(move |(repo, _): &(String, _)| {
                    let (_, _, enabled) = standard_repo_info(&standard_repos, &repo);
                    if enabled {
                        Err(Error::msg(tr!("Already configured")))
                    } else {
                        Ok(())
                    }
                });

                true
            }
            Msg::ToggleEnable => {
                let selected_record = match self.selected_record() {
                    Some(record) => record,
                    None => return false,
                };
                match selected_record {
                    TreeEntry::Repository {
                        path, index, repo, ..
                    } => {
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
            .with_child(
                Button::new(tr!("Add")).onclick(
                    ctx.link()
                        .change_view_callback(|_| Some(ViewState::ShowSubscription)),
                ),
            )
            .with_child({
                let enabled = match selected_record {
                    Some(TreeEntry::Repository { repo, .. }) => Some(repo.enabled),
                    _ => None,
                };
                Button::new(if enabled.unwrap_or(false) {
                    tr!("Disable")
                } else {
                    tr!("Enable")
                })
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
        let table = DataTable::new(self.columns.clone(), self.tree_store.clone())
            .selection(self.selection.clone())
            .class("pwt-flex-fit pwt-border-top")
            .striped(false);

        let mut panel = Column::new().class("pwt-flex-fit");

        let status = DataTable::new(self.status_columns.clone(), self.status_store.clone())
            .class("pwt-flex-fit")
            .show_header(false)
            .striped(false)
            .borderless(true);

        panel.add_child(Row::new().padding(4).with_child(status));

        panel.with_child(table).into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        match view_state {
            ViewState::AddRespository => Some(self.create_add_dialog(ctx)),
            ViewState::ShowSubscription => {
                let (status, url) = match &self.subscription_status {
                    Some(Ok(data)) => (
                        data["status"].as_str().unwrap_or("unknown").to_string(),
                        data["url"].as_str().map(|s| s.to_string()),
                    ),
                    _ => (String::from("unknown"), None),
                };
                if status == "new" || status == "active" {
                    Some(self.create_add_dialog(ctx))
                } else {
                    Some(self.create_show_subscription_dialog(ctx, &status, url))
                }
            }
        }
    }
}

impl From<AptRepositories> for VNode {
    fn from(prop: AptRepositories) -> VNode {
        let comp =
            VComp::new::<LoadableComponentMaster<ProxmoxAptRepositories>>(Rc::new(prop), None);
        VNode::from(comp)
    }
}

fn standard_repo_info(
    repos: &HashMap<String, APTStandardRepository>,
    name: &str,
) -> (String, String, bool) {
    let info = repos.get(name);

    let (status, enabled) = match info {
        Some(APTStandardRepository {
            status: Some(status),
            ..
        }) => {
            let text = if *status {
                tr!("enabled")
            } else {
                tr!("disabled")
            };
            (tr!("Configured") + ": " + &text, *status)
        }
        _ => (tr!("Not yet configured"), false),
    };

    let description = match info {
        Some(APTStandardRepository { description, .. }) => description.clone(),
        _ => tr!("No description available"),
    };

    (status, description, enabled)
}

impl ProxmoxAptRepositories {
    fn active_subscription(&self) -> bool {
        match &self.subscription_status {
            Some(Ok(data)) => {
                data["status"].as_str().map(|s| s.to_lowercase()).as_deref() == Some("active")
            }
            _ => false,
        }
    }

    fn create_show_subscription_dialog(
        &self,
        ctx: &LoadableComponentContext<Self>,
        status: &str,
        url: Option<String>,
    ) -> Html {
        SubscriptionAlert::new(status.to_string())
            .on_close(
                ctx.link()
                    .change_view_callback(|_| Some(ViewState::AddRespository)),
            )
            .url(url.clone().map(|s| s.to_string()))
            .into()
    }

    fn create_add_dialog(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let props = ctx.props();
        let standard_repos = self.standard_repos.clone();
        let validate_standard_repo = self.validate_standard_repo.clone();

        let url = format!("{}/repositories", props.base_url);

        EditWindow::new(tr!("Add") + ": " + &tr!("Respository"))
            .on_done(ctx.link().change_view_callback(|_| None))
            .renderer(move |form_ctx: &FormContext| {
                let repo = form_ctx.read().get_field_text("handle");
                let (status, description, _enabled) = standard_repo_info(&standard_repos, &repo);

                let repository_selector = Combobox::new()
                    .name("handle")
                    .with_item("enterprise")
                    .with_item("no-subscription")
                    .with_item("test")
                    .default("enterprise")
                    .validate(validate_standard_repo.clone())
                    .render_value(|value: &AttrValue| {
                        html! {
                            match value.as_str() {
                                "enterprise" => "Enterprise",
                                "no-subscription" => "No-Subscription",
                                "test" => "Test",
                                v => v,
                            }
                        }
                    });

                Container::new()
                    .class("pwt-d-grid pwt-gap-4")
                    .class(AlignItems::Baseline)
                    .padding(4)
                    .style("grid-template-columns", "minmax(130px, auto) 400px")
                    .with_child(tr!("Repository"))
                    .with_child(repository_selector)
                    .with_child(tr!("Description"))
                    .with_child(html! {<p>{description}</p>})
                    .with_child(tr!("Status"))
                    .with_child(html! {<span>{status}</span>})
                    .into()
            })
            .on_submit(move |form_ctx: FormContext| {
                let param = form_ctx.get_submit_data();
                let url = url.clone();
                async move { crate::http_put(&url, Some(param.clone())).await }
            })
            .into()
    }

    fn selected_record(&self) -> Option<TreeEntry> {
        let selected_key = self.selection.selected_key();
        match selected_key.as_ref() {
            Some(key) => self
                .tree_store
                .read()
                .lookup_node(key)
                .map(|r| r.record().clone()),
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

    fn status_columns(
        _ctx: &LoadableComponentContext<Self>,
    ) -> Rc<Vec<DataTableHeader<StatusLine>>> {
        Rc::new(vec![DataTableColumn::new("Status") // not visible
            .flex(1)
            .show_menu(false)
            .render(|record: &StatusLine| {
                let (icon, color_class) = match record.status {
                    Status::Ok => ("check", ""),
                    Status::Warning => ("exclamation", "pwt-color-warning"),
                    Status::Error => ("times", "pwt-color-error"),
                };
                let icon = Fa::new(icon).fixed_width().padding_end(2);
                html! {<span class={color_class}>{icon}{&record.message}</span>}
            })
            .into()])
    }
}

fn render_enabled_or_group(args: &mut DataTableCellRenderArgs<TreeEntry>) -> Html {
    match args.record() {
        TreeEntry::File {
            path, repo_count, ..
        } => {
            let text = path.clone()
                + " ("
                + &tr!("One repository" | "{n} repositories" % *repo_count as u64)
                + ")";

            args.set_attribute("colspan", "20");
            args.add_class("pwt-bg-color-surface");
            html! {text}
        }
        TreeEntry::Repository { repo, .. } => {
            let icon_class = match repo.enabled {
                true => "fa fa-check",
                false => "fa fa-minus",
            };
            html! {<i class={icon_class}/>}
        }
        _ => html! {},
    }
}

fn render_origin(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { origin, .. } => {
            let (classes, text) = match origin {
                Origin::Debian => ("pmx-icon-debian-swirl", "Debian"),
                Origin::Proxmox => ("pmx-icon-proxmox-x", "Proxmox"),
                Origin::Other => ("fa fa-question-circle-o", "Other"),
            };

            Container::from_tag("span")
                .with_child(Container::from_tag("i").class(classes).padding_end(2))
                .with_child(text)
                .into()
        }
        _ => html! {},
    }
}

fn render_comment(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { repo, .. } => {
            html! {&repo.comment}
        }
        _ => html! {},
    }
}

fn render_text_with_warnings(text: &str, warnings: &[String]) -> Html {
    if warnings.is_empty() {
        html! {text}
    } else {
        let content = html! {
            <span class="pwt-color-warning">
                {text}
                <i class="fa fa-fw fa-exclamation-circle"/>
            </span>
        };
        let title = tr!("Warning" | "Warnings" % warnings.len());
        let mut tip = Container::new().with_child(html! {<h4>{title}</h4>});
        for message in warnings {
            tip.add_child(html! {<p>{message}</p>});
        }
        Tooltip::new(content).rich_tip(tip).into()
    }
}
fn render_components(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { origin, repo, .. } => Row::new()
            .gap(2)
            .children(repo.components.iter().map(|comp| {
                if *origin == Origin::Proxmox {
                    if comp.ends_with("-no-subscription") {
                        let warn = tr!("The no-subscription repository is NOT production-ready");
                        return render_text_with_warnings(comp, &[warn]);
                    }
                    if comp.ends_with("test") {
                        let warn = tr!("The test repository may contain unstable updates");
                        return render_text_with_warnings(comp, &[warn]);
                    }
                }
                html! {<span>{comp.to_string()}</span>}
            }))
            .into(),
        _ => html! {},
    }
}

fn render_suites(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { repo, warnings, .. } => {
            let warnings: Vec<String> = warnings
                .iter()
                .filter(|info| matches!(info.property.as_deref(), Some("Suites")))
                .map(|info| info.message.clone())
                .collect();
            render_text_with_warnings(&repo.suites.join(" "), &warnings)
        }
        _ => html! {},
    }
}

fn render_uris(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { repo, .. } => {
            html! {repo.uris.join(" ")}
        }
        _ => html! {},
    }
}

fn render_types(record: &TreeEntry) -> Html {
    match record {
        TreeEntry::Repository { repo, .. } => {
            let text: String = repo
                .types
                .iter()
                .map(|t| serde_plain::to_string(t).unwrap())
                .collect();
            html! {text}
        }
        _ => html! {},
    }
}
