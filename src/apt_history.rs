use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::css::{
    AlignItems, FlexWrap, FontColor, JustifyContent, Opacity, OverflowY, UserSelect, WhiteSpace,
};
use pwt::prelude::*;
use pwt::props::{FilterFn, RenderFn};
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader, DataTableHeaderGroup, DataTableMouseEvent,
};
use pwt::widget::form::{DateField, Field, PlainDate};
use pwt::widget::{
    get_unique_element_id, Button, Column, Container, Fa, Pane, Row, SegmentedButton,
    SelectionViewRenderInfo, SizeObserver, SplitPane, TabBarItem, TabPanel, Toolbar, Tooltip,
};

use proxmox_apt_history::{
    styled_segments, HistoryEntry, PackageAction, PackageChange, TextColor, TextStyle,
    TransactionChangelog,
};

use crate::package_changelog::PackageChangelog;
use crate::transaction_changelogs::TransactionChangelogs;
use crate::transaction_detail::ProxmoxTransactionDetail;
use crate::utils::{format_duration_human, render_epoch};
use crate::{
    LoadableComponent, LoadableComponentContext, LoadableComponentMaster,
    LoadableComponentScopeExt, LoadableComponentState, Status,
};

use pwt_macros::builder;

async fn list_history(base_url: AttrValue) -> Result<Vec<HistoryEntry>, Error> {
    let url = format!("{base_url}/history");
    crate::http_get(url, None).await.map_err(api_error_message)
}

/// Turns an API error into its message, without the status code that [`proxmox_client::Error`]
/// adds, so that the views show the reason as the API phrased it, for example for a remote running
/// a version without these endpoints.
pub(crate) fn api_error_message(err: Error) -> Error {
    match err.downcast::<proxmox_client::Error>() {
        Ok(proxmox_client::Error::Api(_, message)) => Error::msg(message),
        Ok(err) => err.into(),
        Err(err) => err,
    }
}

/// Shows past APT transactions of a node, their package changes with their changelogs and their
/// terminal output.
#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct AptHistory {
    #[prop_or("/nodes/localhost/apt".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url of the APT API of the node
    pub base_url: AttrValue,

    /// Allow showing the terminal output of the transactions, which needs the privilege to read
    /// the system log.
    #[prop_or(true)]
    #[builder]
    pub enable_term_log: bool,
}

impl Default for AptHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl AptHistory {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

/// Which transactions to show by their status.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum StatusFilter {
    #[default]
    All,
    Ok,
    Failed,
    Interrupted,
}

/// The filters of the transaction list, all applied to the loaded list, which covers all rotated
/// history logs.
#[derive(Clone, Default, PartialEq)]
struct Filters {
    status: StatusFilter,
    major_upgrades: bool,
    since: Option<PlainDate>,
    until: Option<PlainDate>,
    /// Terms of which a package name must contain one.
    packages: Vec<String>,
    search: String,
}

impl Filters {
    fn matches(&self, entry: &HistoryEntry) -> bool {
        let status_matches = match self.status {
            StatusFilter::All => true,
            StatusFilter::Ok => entry.error.is_none() && entry.end_time.is_some(),
            StatusFilter::Failed => entry.error.is_some(),
            StatusFilter::Interrupted => entry.error.is_none() && entry.end_time.is_none(),
        };
        // the timestamps are milliseconds, the until date includes the whole day
        let start = entry.start_time as f64 * 1000.0;
        status_matches
            && (!self.major_upgrades || entry.release_changes().next().is_some())
            && self
                .since
                .is_none_or(|since| start >= since.to_timestamp_start())
            && self
                .until
                .is_none_or(|until| start <= until.to_timestamp_end())
            && (self.packages.is_empty()
                || entry
                    .packages
                    .iter()
                    .any(|change| matches_package(&self.packages, change)))
            && (self.search.is_empty() || self.matches_search(entry))
    }

    fn matches_search(&self, entry: &HistoryEntry) -> bool {
        let contains = |value: &Option<String>| {
            value
                .as_ref()
                .is_some_and(|value| value.to_lowercase().contains(&self.search))
        };
        contains(&entry.command_line)
            || contains(&entry.requested_by)
            || contains(&entry.comment)
            || contains(&entry.error)
    }
}

fn matches_package(terms: &[String], change: &PackageChange) -> bool {
    terms
        .iter()
        .any(|term| change.package.contains(term.as_str()))
}

#[derive(Clone, PartialEq)]
pub enum ViewState {
    Changelog(PackageChange),
}

pub enum Msg {
    SelectionChange,
    PackageSelectionChange,
    Filter(String),
    StatusFilter(StatusFilter),
    ToggleMajorUpgrades,
    Packages(String),
    Since(Option<PlainDate>),
    Until(Option<PlainDate>),
    ShowTermLog,
    HistoryLoaded,
    Resize(f64),
    ExpandCommand(bool),
    SummaryResize,
}

const PACKAGES_TAB: &str = "packages";
const TERM_LOG_TAB: &str = "termlog";
const CHANGELOGS_TAB: &str = "changelogs";

/// Below this width the details go below the list, as each needs about half of it for its columns.
const STACK_BELOW_WIDTH: f64 = 1200.0;

pub struct ProxmoxAptHistory {
    state: LoadableComponentState<ViewState>,
    store: Store<HistoryEntry>,
    selection: Selection,
    package_store: Store<PackageChange>,
    package_selection: Selection,
    columns: Rc<Vec<DataTableHeader<HistoryEntry>>>,
    package_columns: Rc<Vec<DataTableHeader<PackageChange>>>,
    filters: Filters,
    tab_selection: Selection,
    generation: u64,
    stacked: bool,
    /// The transaction whose command line is expanded, any other one shows it collapsed.
    expanded_command: Option<Key>,
    /// Whether the collapsed command line does not fit into its line.
    command_long: bool,
    command_id: AttrValue,
    command_ref: NodeRef,
    command_toggle_ref: NodeRef,
}

pwt::impl_deref_mut_property!(ProxmoxAptHistory, state, LoadableComponentState<ViewState>);

impl LoadableComponent for ProxmoxAptHistory {
    type Properties = AptHistory;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        // transactions can start in the same second, see `HistoryEntry::start_index`
        let store = Store::with_extract_key(|entry: &HistoryEntry| {
            Key::from(format!("{}.{}", entry.start_time, entry.start_index))
        });
        let package_store = Store::with_extract_key(|change: &PackageChange| {
            Key::from(format!(
                "{:?}/{}/{}",
                change.action,
                change.package,
                change.arch.as_deref().unwrap_or_default(),
            ))
        });
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::SelectionChange));
        let package_selection =
            Selection::new().on_select(ctx.link().callback(|_| Msg::PackageSelectionChange));
        let tab_selection = Selection::new();

        Self {
            state: LoadableComponentState::new(),
            store,
            selection,
            package_store,
            package_selection,
            columns: Self::columns(&[]),
            package_columns: Self::package_columns(),
            filters: Filters::default(),
            tab_selection,
            generation: 0,
            stacked: false,
            expanded_command: None,
            command_long: false,
            command_id: AttrValue::from(get_unique_element_id()),
            command_ref: NodeRef::default(),
            command_toggle_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SelectionChange => {
                if self.expanded_command != self.selection.selected_key() {
                    self.expanded_command = None;
                }
                self.show_details();
                true
            }
            Msg::PackageSelectionChange => true,
            Msg::Filter(search) => {
                self.filters.search = search.trim().to_lowercase();
                self.apply_filters();
                true
            }
            Msg::StatusFilter(status) => {
                self.filters.status = status;
                self.apply_filters();
                true
            }
            Msg::ToggleMajorUpgrades => {
                self.filters.major_upgrades = !self.filters.major_upgrades;
                self.apply_filters();
                true
            }
            Msg::Packages(packages) => {
                let packages: Vec<String> = packages
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|term| !term.is_empty())
                    .map(str::to_lowercase)
                    .collect();
                if packages == self.filters.packages {
                    return false;
                }
                self.filters.packages = packages;
                self.columns = Self::columns(&self.filters.packages);
                self.apply_filters();
                true
            }
            Msg::Since(since) => {
                self.filters.since = since;
                self.apply_filters();
                true
            }
            Msg::Until(until) => {
                self.filters.until = until;
                self.apply_filters();
                true
            }
            Msg::ShowTermLog => {
                if ctx.props().enable_term_log {
                    self.tab_selection.select(Key::from(TERM_LOG_TAB));
                }
                false
            }
            // a reload keeps the selection, but the transaction can have changed, e.g. finished,
            // and then also be hidden by the filters
            Msg::HistoryLoaded => {
                self.clear_hidden_selection();
                self.generation += 1;
                self.show_details();
                true
            }
            Msg::Resize(width) => {
                let stacked = width < STACK_BELOW_WIDTH;
                let changed = stacked != self.stacked;
                self.stacked = stacked;
                changed
            }
            Msg::ExpandCommand(expand) => {
                self.expanded_command = expand.then(|| self.selection.selected_key()).flatten();
                true
            }
            Msg::SummaryResize => self.update_command_long(),
        }
    }

    fn rendered(&mut self, ctx: &LoadableComponentContext<Self>, _first_render: bool) {
        if self.update_command_long() {
            ctx.link().send_redraw();
        }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let base_url = ctx.props().base_url.clone();
        let store = self.store.clone();
        let link = ctx.link().clone();
        Box::pin(async move {
            let entries = list_history(base_url).await?;
            store.set_data(entries);
            link.send_message(Msg::HistoryLoaded);
            Ok(())
        })
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let pressed_scheme = "pwt-scheme-secondary-container";
        let mut status = SegmentedButton::new().class("pwt-button-elevated");
        for (filter, label) in [
            (StatusFilter::All, tr!("All")),
            (StatusFilter::Ok, tr!("OK")),
            (StatusFilter::Failed, tr!("Failed")),
            (StatusFilter::Interrupted, tr!("Interrupted")),
        ] {
            let pressed = self.filters.status == filter;
            status.add_button(
                Button::new(label)
                    .pressed(pressed)
                    .class(pressed.then_some(pressed_scheme))
                    .onclick(ctx.link().callback(move |_| Msg::StatusFilter(filter))),
            );
        }
        let major_upgrades = self.filters.major_upgrades;
        let major_upgrades = SegmentedButton::new()
            .class("pwt-button-elevated")
            .with_button(
                Button::new(tr!("Major Upgrades"))
                    .pressed(major_upgrades)
                    .class(major_upgrades.then_some(pressed_scheme))
                    .onclick(ctx.link().callback(|_| Msg::ToggleMajorUpgrades)),
            );

        // wraps instead of a toolbar, as the filters do not fit into narrow panels
        let toolbar = Row::new()
            .class(FlexWrap::Wrap)
            .class(AlignItems::Center)
            .gap(2)
            .padding(2)
            .class("pwt-border-bottom")
            .with_child(status)
            .with_child(Tooltip::new(major_upgrades).tip(tr!(
                "Only transactions that changed the major release of Debian or Ceph"
            )))
            .with_child(labeled(
                tr!("Since"),
                DateField::new()
                    .aria_label(tr!("Since"))
                    .max_value(self.filters.until)
                    .on_change(ctx.link().callback(Msg::Since)),
            ))
            .with_child(labeled(
                tr!("Until"),
                DateField::new()
                    .aria_label(tr!("Until"))
                    .min_value(self.filters.since)
                    .on_change(ctx.link().callback(Msg::Until)),
            ))
            .with_flex_spacer()
            // text fields always take the full width of their parent, so that sets their width
            .with_child(
                Container::new().width("12em").with_child(
                    Field::new()
                        .aria_label(tr!("Package names"))
                        .placeholder(tr!("Package names"))
                        .on_input(ctx.link().callback(Msg::Packages)),
                ),
            )
            .with_child(
                Container::new().width("18em").with_child(
                    Field::new()
                        .aria_label(tr!("Search"))
                        .placeholder(tr!("Search command line or user"))
                        .on_input(ctx.link().callback(Msg::Filter)),
                ),
            )
            .with_child({
                let loading = self.loading();
                let link = ctx.link().clone();
                Button::refresh(loading).onclick(move |_| link.send_reload())
            });

        Some(toolbar.into())
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let empty_text = (!self.loading() && self.store.filtered_data_len() == 0).then(|| {
            if self.filters == Filters::default() {
                tr!("No APT history available.")
            } else {
                tr!("No transactions match the filters.")
            }
        });
        // the table only shrinks to its header to show the text below, so it keeps its state
        let history = Column::new()
            .class("pwt-flex-fit")
            .with_child(
                DataTable::new(self.columns.clone(), self.store.clone())
                    .selection(self.selection.clone())
                    .on_row_dblclick({
                        let link = ctx.link().clone();
                        move |_: &mut DataTableMouseEvent| link.send_message(Msg::ShowTermLog)
                    })
                    .class(empty_text.is_none().then_some("pwt-flex-fit"))
                    .striped(false)
                    .borderless(true),
            )
            .with_optional_child(
                empty_text.map(|text| Container::new().padding(2).with_child(text)),
            );

        let selected_change = self
            .selected_change()
            .filter(|change| change.changelog_range().is_some());
        let packages = Column::new()
            .class("pwt-flex-fit")
            .with_child(
                Toolbar::new().class("pwt-border-bottom").with_child(
                    Button::new(tr!("Changelog"))
                        .disabled(selected_change.is_none())
                        .onclick({
                            let link = ctx.link().clone();
                            move |_| {
                                link.change_view(selected_change.clone().map(ViewState::Changelog))
                            }
                        }),
                ),
            )
            .with_child(
                DataTable::new(self.package_columns.clone(), self.package_store.clone())
                    .selection(self.package_selection.clone())
                    .on_row_dblclick({
                        let link = ctx.link().clone();
                        let store = self.package_store.clone();
                        move |event: &mut DataTableMouseEvent| {
                            let change = store.read().lookup_record(&event.record_key).cloned();
                            if let Some(change) =
                                change.filter(|change| change.changelog_range().is_some())
                            {
                                link.change_view(Some(ViewState::Changelog(change)));
                            }
                        }
                    })
                    .class("pwt-flex-fit")
                    .borderless(true),
            );

        let transaction = self
            .selected_entry()
            .map(|entry| (entry.start_time, entry.start_index));
        let generation = self.generation;
        let base_url = ctx.props().base_url.clone();
        let enable_term_log = ctx.props().enable_term_log;
        let term_log_parameters = serde_json::Map::from_iter([("format".into(), "styled".into())]);

        let tabs = TabPanel::new()
            .class("pwt-flex-fit")
            .selection(self.tab_selection.clone())
            .with_item(
                TabBarItem::new().key(PACKAGES_TAB).label(tr!("Packages")),
                packages,
            )
            .with_item_builder(
                TabBarItem::new()
                    .key(CHANGELOGS_TAB)
                    .label(tr!("Changelogs")),
                {
                    let base_url = base_url.clone();
                    move |info: &SelectionViewRenderInfo| {
                        let renderer = RenderFn::new(|groups: &Rc<Vec<TransactionChangelog>>| {
                            TransactionChangelogs::new(groups.clone()).into()
                        });
                        html! {
                            <ProxmoxTransactionDetail<Vec<TransactionChangelog>>
                                base_url={base_url.clone()}
                                path="transaction-changelogs"
                                {transaction}
                                {generation}
                                visible={info.visible}
                                {renderer}
                            />
                        }
                    }
                },
            )
            .with_item_builder(
                TabBarItem::new()
                    .key(TERM_LOG_TAB)
                    .label(tr!("Terminal Log"))
                    .disabled(!enable_term_log),
                move |info: &SelectionViewRenderInfo| {
                    let renderer = RenderFn::new(|output: &Rc<String>| {
                        Container::from_tag("pre")
                            .padding(2)
                            .class("pwt-flex-fit pwt-log-content")
                            .with_child(if output.is_empty() {
                                tr!("No terminal output recorded.").into()
                            } else {
                                render_term_log(output)
                            })
                            .into()
                    });
                    html! {
                        <ProxmoxTransactionDetail<String>
                            base_url={base_url.clone()}
                            path="termlog"
                            {transaction}
                            {generation}
                            visible={info.visible && enable_term_log}
                            parameters={term_log_parameters.clone()}
                            {renderer}
                        />
                    }
                },
            );

        let details: Html = match self.selected_entry() {
            Some(entry) => Column::new()
                .class("pwt-flex-fit")
                .with_child(self.summary_view(ctx, entry))
                .with_child(tabs)
                .into(),
            None => Column::new()
                .class("pwt-flex-fit")
                .class(AlignItems::Center)
                .class(JustifyContent::Center)
                .gap(2)
                .padding(4)
                .with_child(Fa::new("history").large_4x())
                .with_child(tr!("Select a transaction to see its details."))
                .into(),
        };

        let split = SplitPane::new()
            .class("pwt-flex-fit")
            .vertical(self.stacked)
            .with_child(Pane::new(history).flex(1))
            .with_child(Pane::new(details).flex(1));
        SizeObserver::new(
            Column::new().class("pwt-flex-fit").with_child(split),
            ctx.link().callback(|(width, _height)| Msg::Resize(width)),
        )
        .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        let ViewState::Changelog(change) = view_state;
        let (since_version, to_version) = change.changelog_range()?;
        Some(
            PackageChangelog::new(change.package.clone())
                .base_url(ctx.props().base_url.clone())
                .since_version(since_version.map(str::to_owned))
                .to_version(to_version.to_owned())
                .on_close({
                    let link = ctx.link().clone();
                    move |_| link.change_view(None)
                })
                .into(),
        )
    }

    fn changed(
        &mut self,
        ctx: &LoadableComponentContext<Self>,
        old_props: &Self::Properties,
    ) -> bool {
        if ctx.props().base_url != old_props.base_url {
            // the entries and the selected transaction belong to the previous node, keep only the
            // filters
            self.store.clear();
            self.selection.clear();
            self.expanded_command = None;
            self.show_details();
            ctx.link().send_reload();
        }
        ctx.props() != old_props
    }
}

impl From<AptHistory> for VNode {
    fn from(prop: AptHistory) -> VNode {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxAptHistory>>(Rc::new(prop), None);
        VNode::from(comp)
    }
}

impl ProxmoxAptHistory {
    /// The selected transaction, unless the filters hide it.
    fn selected_entry(&self) -> Option<HistoryEntry> {
        let key = self.selection.selected_key()?;
        self.store.filtered_record_pos(&key)?;
        self.store.read().lookup_record(&key).cloned()
    }

    fn selected_change(&self) -> Option<PackageChange> {
        let key = self.package_selection.selected_key()?;
        self.package_store.read().lookup_record(&key).cloned()
    }

    fn apply_filters(&mut self) {
        if self.filters == Filters::default() {
            self.store.set_filter(None::<FilterFn<HistoryEntry>>);
        } else {
            let filters = self.filters.clone();
            self.store
                .set_filter(move |entry: &HistoryEntry| filters.matches(entry));
        }
        if self.clear_hidden_selection() {
            self.show_details();
        }
    }

    /// Clears the selection of a transaction that the filters hide, so that its details are not
    /// shown anymore, returns whether it did. The table does not clean up a single selection if it
    /// was the only visible row.
    fn clear_hidden_selection(&mut self) -> bool {
        let hidden = self
            .selection
            .selected_key()
            .is_some_and(|key| self.store.filtered_record_pos(&key).is_none());
        if hidden {
            self.selection.clear();
        }
        hidden
    }

    fn show_details(&self) {
        let packages = self
            .selected_entry()
            .map(|entry| entry.packages)
            .unwrap_or_default();
        self.package_store.set_data(packages);
    }

    fn command_expanded(&self) -> bool {
        self.expanded_command.is_some() && self.expanded_command == self.selection.selected_key()
    }

    /// Measures whether the collapsed command line does not fit, returns whether that changed.
    fn update_command_long(&mut self) -> bool {
        if self.command_expanded() {
            return false;
        }
        let Some(text) = self.command_ref.cast::<web_sys::Element>() else {
            return false;
        };
        // measure as if the toggle was not there, it takes some of the width while shown
        let toggle_width = match self.command_long {
            true => self
                .command_toggle_ref
                .cast::<web_sys::HtmlElement>()
                .map_or(0, |toggle| toggle.offset_width()),
            false => 0,
        };
        let long = text.scroll_width() > text.client_width() + toggle_width;
        std::mem::replace(&mut self.command_long, long) != long
    }

    fn summary_view(&self, ctx: &LoadableComponentContext<Self>, entry: HistoryEntry) -> Html {
        let command = entry.command_line.clone().unwrap_or_else(|| "-".into());
        let mut info = vec![render_changes(&entry, &[])];
        if let Some(user) = &entry.requested_by {
            info.push(tr!("Requested by {0}", user));
        }
        if let Some(comment) = &entry.comment {
            info.push(comment.clone());
        }
        info.retain(|part| !part.is_empty());
        let info = info.join(" · ");

        let icon = |name| {
            Fa::new(name)
                .fixed_width()
                .padding_end(1)
                .class(Opacity::ThreeQuarters)
        };
        // fixed widths keep the values in place when going through the transactions
        let status = Row::new()
            .with_child(html! {
                <span class="pwt-me-2" style="min-width: 11.5em;">
                    {icon("clock-o")}
                    {render_epoch(entry.start_time)}
                </span>
            })
            .with_child(html! {
                <span class="pwt-me-2" style="min-width: 5.5em;">
                    {icon("hourglass-half")}
                    {render_duration(&entry)}
                </span>
            })
            .with_child(html! {
                <div class="pwt-text-truncate pwt-flex-fill">
                    {render_status(&entry)}
                </div>
            });

        // a single line until expanded, then wrapped and scrolling beyond five lines
        let expanded = self.command_expanded();
        let expandable = expanded || self.command_long;
        let (command_class, command_style) = if expanded {
            (
                classes!(
                    "pwt-font-monospace",
                    WhiteSpace::PreWrap,
                    OverflowY::Auto,
                    UserSelect::Text
                ),
                "min-width: 0; overflow-wrap: anywhere; max-height: 8em; cursor: text;",
            )
        } else {
            (
                classes!("pwt-font-monospace", "pwt-text-truncate", UserSelect::Text),
                match expandable {
                    true => "min-width: 0; cursor: pointer;",
                    false => "min-width: 0; cursor: text;",
                },
            )
        };
        let link = ctx.link().clone();
        let expand_on_click = (expandable && !expanded).then(|| {
            let link = link.clone();
            Callback::from(move |_: MouseEvent| {
                // not when the click ends selecting some of the text
                let selecting = web_sys::window()
                    .and_then(|window| window.get_selection().ok().flatten())
                    .is_some_and(|selection| !selection.is_collapsed());
                if !selecting {
                    link.send_message(Msg::ExpandCommand(true));
                }
            })
        });
        let toggle = expandable.then(|| {
            let label = if expanded {
                tr!("Collapse")
            } else {
                tr!("Expand")
            };
            let onclick = {
                let link = link.clone();
                Callback::from(move |_: MouseEvent| {
                    link.send_message(Msg::ExpandCommand(!expanded))
                })
            };
            let onkeydown = Callback::from(move |event: KeyboardEvent| {
                if matches!(event.key().as_str(), "Enter" | " ") {
                    event.prevent_default();
                    link.send_message(Msg::ExpandCommand(!expanded));
                }
            });
            html! {
                <span
                    ref={self.command_toggle_ref.clone()}
                    class={classes!("fa", "fa-fw", "fa-caret-down", Opacity::ThreeQuarters)}
                    style={match expanded {
                        true => "line-height: inherit; cursor: pointer; transform: rotate(180deg);",
                        false => "line-height: inherit; cursor: pointer;",
                    }}
                    role="button"
                    tabindex="0"
                    aria-controls={self.command_id.clone()}
                    aria-expanded={expanded.to_string()}
                    aria-label={label.clone()}
                    title={label}
                    {onclick}
                    {onkeydown}
                />
            }
        });

        let summary = Column::new()
            .padding_x(2)
            .padding_y(1)
            .style("line-height", "1.6")
            .with_child(status)
            .with_child(
                Row::new()
                    .class(AlignItems::FlexStart)
                    .margin_y(1)
                    .with_child(icon("terminal").style("line-height", "inherit"))
                    .with_child(html! {
                        <span
                            id={self.command_id.clone()}
                            ref={self.command_ref.clone()}
                            class={command_class}
                            style={command_style}
                            onclick={expand_on_click}
                        >
                            {command}
                        </span>
                    })
                    .with_optional_child(toggle),
            )
            .with_child(html! {
                <div class="pwt-text-truncate" title={info.clone()}>{info}</div>
            });
        // the command line fits into its line or not depending on the width
        SizeObserver::new(
            summary,
            ctx.link().callback(|_: (f64, f64)| Msg::SummaryResize),
        )
        .into()
    }

    fn columns(package_terms: &[String]) -> Rc<Vec<DataTableHeader<HistoryEntry>>> {
        let package_terms = package_terms.to_vec();
        Rc::new(vec![
            DataTableColumn::new(tr!("Start Time"))
                .width("160px")
                .sorter(|a: &HistoryEntry, b: &HistoryEntry| {
                    (a.start_time, a.start_index).cmp(&(b.start_time, b.start_index))
                })
                .sort_order(false)
                .render(|entry: &HistoryEntry| render_epoch(entry.start_time).into())
                .into(),
            DataTableColumn::new(tr!("Duration"))
                .width("100px")
                .render(|entry: &HistoryEntry| render_duration(entry).into())
                .into(),
            DataTableColumn::new(tr!("Command Line"))
                .flex(2)
                .render(|entry: &HistoryEntry| {
                    render_with_tooltip(entry.command_line.as_deref().unwrap_or_default())
                })
                .into(),
            DataTableColumn::new(tr!("Requested By"))
                .width("100px")
                .hidden(true)
                .render(|entry: &HistoryEntry| {
                    entry.requested_by.clone().unwrap_or_default().into()
                })
                .into(),
            DataTableColumn::new(tr!("Comment"))
                .flex(1)
                .hidden(true)
                .render(|entry: &HistoryEntry| entry.comment.clone().unwrap_or_default().into())
                .into(),
            DataTableColumn::new(tr!("Changes"))
                .flex(3)
                .render(move |entry: &HistoryEntry| {
                    render_with_tooltip(&render_changes(entry, &package_terms))
                })
                .into(),
            DataTableColumn::new(tr!("Status"))
                .flex(1)
                .render(render_status)
                .into(),
        ])
    }

    fn package_columns() -> Rc<Vec<DataTableHeader<PackageChange>>> {
        Rc::new(vec![
            DataTableColumn::new(tr!("Action"))
                .width("100px")
                .render(|change: &PackageChange| action_text(change.action).into())
                .into(),
            DataTableColumn::new(tr!("Package"))
                .flex(3)
                .render(|change: &PackageChange| render_with_tooltip(&change.package))
                .into(),
            DataTableColumn::new(tr!("Architecture"))
                .width("120px")
                .render(|change: &PackageChange| change.arch.clone().unwrap_or_default().into())
                .into(),
            DataTableHeaderGroup::new(tr!("Version"))
                .with_child(DataTableColumn::new(tr!("old")).flex(2).render(
                    |change: &PackageChange| {
                        render_with_tooltip(change.old_version.as_deref().unwrap_or_default())
                    },
                ))
                .with_child(DataTableColumn::new(tr!("new")).flex(2).render(
                    |change: &PackageChange| {
                        render_with_tooltip(change.new_version.as_deref().unwrap_or_default())
                    },
                ))
                .into(),
            DataTableColumn::new(tr!("Automatic"))
                .width("100px")
                .render(|change: &PackageChange| {
                    if change.automatic {
                        tr!("Yes").into()
                    } else {
                        html! {}
                    }
                })
                .into(),
        ])
    }
}

/// All actions in the order APT logs them.
const ACTIONS: [PackageAction; 7] = [
    PackageAction::Install,
    PackageAction::Reinstall,
    PackageAction::Upgrade,
    PackageAction::Downgrade,
    PackageAction::Remove,
    PackageAction::Purge,
    PackageAction::Disappear,
];

fn action_text(action: PackageAction) -> String {
    match action {
        PackageAction::Install => tr!("package action" => "Install"),
        PackageAction::Reinstall => tr!("package action" => "Reinstall"),
        PackageAction::Upgrade => tr!("package action" => "Upgrade"),
        PackageAction::Downgrade => tr!("package action" => "Downgrade"),
        PackageAction::Remove => tr!("package action" => "Remove"),
        PackageAction::Purge => tr!("package action" => "Purge"),
        PackageAction::Disappear => tr!("package action" => "Disappear"),
    }
}

fn action_summary(action: PackageAction, n: usize) -> String {
    match action {
        PackageAction::Install => tr!("{n} installed" | "{n} installed" % n),
        PackageAction::Reinstall => tr!("{n} reinstalled" | "{n} reinstalled" % n),
        PackageAction::Upgrade => tr!("{n} upgraded" | "{n} upgraded" % n),
        PackageAction::Downgrade => tr!("{n} downgraded" | "{n} downgraded" % n),
        PackageAction::Remove => tr!("{n} removed" | "{n} removed" % n),
        PackageAction::Purge => tr!("{n} purged" | "{n} purged" % n),
        PackageAction::Disappear => tr!("{n} disappeared" | "{n} disappeared" % n),
    }
}

/// The changes between major releases, then the changes of the packages matching the terms of the
/// package filter, or without terms the number of changes by action.
fn render_changes(entry: &HistoryEntry, package_terms: &[String]) -> String {
    let releases = entry
        .release_changes()
        .map(|release| format!("{} {} → {}", release.name, release.old, release.new));
    let changes: Vec<String> = if package_terms.is_empty() {
        ACTIONS
            .iter()
            .filter_map(|&action| {
                let n = entry.packages.iter().filter(|c| c.action == action).count();
                (n > 0).then(|| action_summary(action, n))
            })
            .collect()
    } else {
        entry
            .packages
            .iter()
            .filter(|change| matches_package(package_terms, change))
            .map(|change| {
                let text = format!("{} {}", change.package, versions_text(change));
                match change.action {
                    PackageAction::Upgrade => text,
                    action => format!("{text} ({})", action_text(action)),
                }
            })
            .collect()
    };
    releases.chain(changes).collect::<Vec<_>>().join(", ")
}

fn versions_text(change: &PackageChange) -> String {
    match (change.old_version.as_deref(), change.new_version.as_deref()) {
        (Some(old), Some(new)) if old != new => format!("{old} → {new}"),
        (old, new) => new.or(old).unwrap_or_default().to_owned(),
    }
}

/// Keeps a label on the same line as its field when the filters wrap.
fn labeled(label: String, field: impl Into<Html>) -> Row {
    Row::new()
        .class(AlignItems::Center)
        .gap(2)
        .with_child(label)
        .with_child(field)
}

fn render_with_tooltip(text: &str) -> Html {
    html! { <span class="pwt-text-truncate" title={text.to_string()}>{text}</span> }
}

fn render_status(entry: &HistoryEntry) -> Html {
    let (status, text) = match (&entry.error, entry.end_time) {
        (Some(error), _) => (Status::Error, error.clone()),
        (None, None) => (Status::Warning, tr!("Interrupted or still running")),
        (None, Some(_)) => (Status::Success, tr!("OK")),
    };
    html! {
        <span class="pwt-text-truncate" title={text.clone()}>
            {Fa::from(status).padding_end(2)}{text}
        </span>
    }
}

// the log only has local time, so a transaction can end before it started if the clock was set back
fn render_duration(entry: &HistoryEntry) -> String {
    match entry.end_time {
        Some(end_time) if end_time >= entry.start_time => {
            format_duration_human((end_time - entry.start_time) as f64)
        }
        Some(_) => tr!("Unknown"),
        None => "-".into(),
    }
}

/// Renders styled terminal output, with the colors mapped to the ones of the theme.
fn render_term_log(output: &str) -> Html {
    styled_segments(output)
        .map(|(style, text)| {
            if style == TextStyle::default() {
                return html! {text};
            }
            let color = style.color.map(|color| match color {
                TextColor::Red => FontColor::Error,
                TextColor::Green => FontColor::Success,
                TextColor::Yellow => FontColor::Warning,
                TextColor::Blue => FontColor::Primary,
                TextColor::Magenta => FontColor::Tertiary,
                TextColor::Cyan => FontColor::Secondary,
            });
            let class = classes!(color, style.dim.then_some(Opacity::ThreeQuarters));
            let bold = style.bold.then_some("font-weight: bold;");
            html! { <span {class} style={bold}>{text}</span> }
        })
        .collect()
}
