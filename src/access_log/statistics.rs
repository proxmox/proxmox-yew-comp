use std::rc::Rc;

use serde_json::Value;
use yew::virtual_dom::Key;

use pwt::prelude::*;
use pwt::state::Store;
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeaderRenderArgs, DataTableKeyboardEvent,
    DataTableMouseEvent,
};
use pwt::widget::{Button, Column, Container, Dialog, Mask, TabBarItem, TabPanel, Toolbar};
use pwt::AsyncPool;

use proxmox_access_log::{AccessLogStats, StatsList, StopReason};

use crate::utils::{format_duration_human, render_epoch};
use crate::{RRDGraph, Series};

/// The number of values with the most requests that statistics list.
const STATS_TOP: u64 = 25;

type FilterFields = Vec<(&'static str, Value)>;

pub struct DrillDown {
    pub filter: Value,
    pub since: i64,
    pub until: i64,
    pub fields: FilterFields,
}

#[derive(PartialEq, Properties)]
pub struct StatisticsProps {
    pub url: String,
    pub filter: Value,
    pub request: usize,
    pub open: bool,
    pub on_reload: Callback<()>,
    pub on_drill_down: Callback<DrillDown>,
    pub on_close: Callback<()>,
}

#[derive(Clone, PartialEq)]
struct Query {
    url: String,
    filter: Value,
}

impl From<&StatisticsProps> for Query {
    fn from(props: &StatisticsProps) -> Self {
        Self {
            url: props.url.clone(),
            filter: props.filter.clone(),
        }
    }
}

pub enum Msg {
    Loaded {
        generation: usize,
        result: Result<AccessLogStats, String>,
    },
    DrillDown(FilterFields),
}

pub struct Statistics {
    async_pool: AsyncPool,
    generation: usize,
    query: Query,
    pending: Option<Query>,
    loading: bool,
    data: Option<Result<AccessLogStats, String>>,
}

impl Component for Statistics {
    type Properties = StatisticsProps;
    type Message = Msg;

    fn create(ctx: &Context<Self>) -> Self {
        let query = Query::from(ctx.props());
        let mut me = Self {
            async_pool: AsyncPool::new(),
            generation: 0,
            query: query.clone(),
            pending: None,
            loading: false,
            data: None,
        };
        me.load(ctx, query);
        me
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if old_props.open && !props.open {
            self.pending = None;
        }
        if props.request != old_props.request || props.url != old_props.url {
            let query = Query::from(props);
            if self.loading {
                // one scan at a time, but one of another filter or range replaces this result
                self.pending = (query != self.query).then_some(query);
            } else {
                self.load(ctx, query);
            }
        }
        true
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded { generation, result } => {
                if generation != self.generation {
                    return false;
                }
                if let Some(query) = self.pending.take() {
                    self.load(ctx, query);
                } else {
                    self.loading = false;
                    self.data = Some(result);
                }
                true
            }
            Msg::DrillDown(fields) => {
                let Some(Ok(stats)) = &self.data else {
                    return false;
                };
                ctx.props().on_drill_down.emit(DrillDown {
                    filter: self.query.filter.clone(),
                    since: stats.since,
                    until: stats.until,
                    fields,
                });
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        if !props.open {
            return html! {};
        }
        let body: Html = match &self.data {
            Some(Ok(stats)) => self.statistics_view(ctx, stats),
            Some(Err(err)) => pwt::widget::error_message(err).into(),
            None => html! {},
        };
        let title = match &self.data {
            Some(Ok(stats)) => tr!(
                "Access Log Statistics: {0} to {1}",
                render_epoch(stats.since),
                render_epoch(stats.until),
            ),
            _ => tr!("Access Log Statistics"),
        };
        Dialog::new(title)
            .resizable(true)
            .width(900)
            .height(760)
            .on_close(props.on_close.clone())
            .with_child(
                Column::new()
                    .class("pwt-flex-fit")
                    .with_child(Mask::new(body).class("pwt-flex-fit").visible(self.loading))
                    .with_child(
                        Toolbar::new().with_flex_spacer().with_child(
                            Button::new(tr!("Reload"))
                                .icon_class("fa fa-refresh")
                                .disabled(self.loading)
                                .onclick(props.on_reload.reform(|_| ())),
                        ),
                    ),
            )
            .into()
    }
}

impl Statistics {
    fn load(&mut self, ctx: &Context<Self>, query: Query) {
        self.generation += 1;
        self.loading = true;
        self.data = None;
        self.query = query.clone();
        let generation = self.generation;
        let mut params = query.filter;
        params["top"] = STATS_TOP.into();
        let link = ctx.link().clone();
        self.async_pool.spawn(async move {
            let result = crate::http_get(&query.url, Some(params))
                .await
                .map_err(|err| err.to_string());
            link.send_message(Msg::Loaded { generation, result });
        });
    }

    fn statistics_view(&self, ctx: &Context<Self>, stats: &AccessLogStats) -> Html {
        let total = &stats.total;
        let counts = match stats.invalid_lines {
            0 => tr!(
                "{0} requests, {1} client errors, {2} server errors",
                total.requests,
                total.client_errors,
                total.server_errors,
            ),
            invalid => tr!(
                "{0} requests, {1} client errors, {2} server errors, {n} invalid line"
                    | "{0} requests, {1} client errors, {2} server errors, {n} invalid lines"
                        % invalid,
                total.requests,
                total.client_errors,
                total.server_errors
            ),
        };
        let mut summary = Column::new().padding(2).gap(1).with_child(counts);
        if !stats.complete {
            let reason = match stats.stopped {
                Some(StopReason::ReadBudget | StopReason::ParseBudget) => tr!(
                    "Statistics are incomplete because the scan limit was reached. \
                    Actual counts may be higher."
                ),
                _ => tr!(
                    "Could not read all of {0}. Actual counts may be higher.",
                    stats.gaps.first().map_or("?", |gap| gap.file.as_str())
                ),
            };
            summary.add_child(
                Container::new()
                    .with_child(html! {<i class="fa fa-exclamation-triangle pwt-color-warning"/>})
                    .with_child(" ")
                    .with_child(reason),
            );
        }

        let histogram = &stats.histogram;
        let time_data: Vec<i64> = (0..histogram.counts.len() as i64)
            .map(|index| histogram.start + index * histogram.bucket as i64)
            .collect();
        let requests = histogram.counts.iter().map(|c| c.requests as f64).collect();
        let errors = histogram
            .counts
            .iter()
            .map(|c| (c.client_errors + c.server_errors) as f64)
            .collect();
        let graph = RRDGraph::new(Rc::new(time_data))
            .title(tr!(
                "Requests per {0}",
                format_duration_human(histogram.bucket as f64)
            ))
            .serie0(Some(Rc::new(Series::new(tr!("Requests"), requests))))
            .serie1(Some(Rc::new(Series::new(tr!("Errors"), errors))));

        let mut lists = vec![
            (tr!("Client"), &stats.clients, StatsKind::Client),
            (tr!("User"), &stats.auth_ids, StatsKind::User),
            (tr!("Path Group"), &stats.path_groups, StatsKind::PathGroup),
            (tr!("Status"), &stats.statuses, StatsKind::Status),
        ];
        if let Some(user_agents) = &stats.user_agents {
            lists.push((tr!("User Agent"), user_agents, StatsKind::UserAgent));
        }
        let mut panel = TabPanel::new().class("pwt-flex-fit");
        for (title, list, kind) in lists {
            panel.add_item(
                TabBarItem::new().label(title.clone()),
                stats_table(title, list, kind, ctx.link().callback(Msg::DrillDown)),
            );
        }

        Column::new()
            .class("pwt-flex-fit")
            .with_child(summary)
            .with_child(graph)
            .with_child(panel)
            .into()
    }
}

/// A row of a statistics list, or the one for all other values.
#[derive(Clone, PartialEq)]
struct StatsTableRow {
    index: usize,
    key: Option<String>,
    others: Option<String>,
    requests: u64,
    client_errors: u64,
    server_errors: u64,
    last: Option<i64>,
}

/// What a statistics list counts by, which sets the filter its rows lead to.
#[derive(Clone, Copy)]
enum StatsKind {
    Client,
    User,
    PathGroup,
    Status,
    UserAgent,
}

impl StatsKind {
    /// The filter fields for exactly the requests a row with this value counted.
    fn drill_down(self, key: Option<String>) -> Option<FilterFields> {
        match (self, key) {
            (StatsKind::Client, Some(key)) => Some(vec![("client", key.into())]),
            (StatsKind::User, key) => Some(vec![
                ("auth-id", key.unwrap_or_else(|| "-".to_string()).into()),
                ("auth-id-exact", true.into()),
            ]),
            (StatsKind::PathGroup, Some(key)) => Some(vec![("path-group", key.into())]),
            (StatsKind::UserAgent, Some(key)) => Some(vec![("user-agent", key.into())]),
            (StatsKind::Status, Some(key)) => Some(vec![("status", key.into())]),
            _ => None,
        }
    }
}

fn stats_table(
    title: String,
    list: &StatsList,
    kind: StatsKind,
    on_activate: Callback<FilterFields>,
) -> DataTable<Store<StatsTableRow>> {
    let mut rows: Vec<StatsTableRow> = list
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| StatsTableRow {
            index,
            key: row.key.clone(),
            others: None,
            requests: row.requests,
            client_errors: row.client_errors,
            server_errors: row.server_errors,
            last: Some(row.last),
        })
        .collect();
    if list.others.requests > 0 {
        rows.push(StatsTableRow {
            index: rows.len(),
            key: None,
            others: Some(match list.truncated {
                true => tr!("Other values, more than counted separately"),
                false => tr!("Other values"),
            }),
            requests: list.others.requests,
            client_errors: list.others.client_errors,
            server_errors: list.others.server_errors,
            last: None,
        });
    }
    let store = Store::with_extract_key(|row: &StatsTableRow| Key::from(row.index));
    store.set_data(rows);

    let columns = Rc::new(vec![
        DataTableColumn::new(title)
            .flex(1)
            .render({
                let on_activate = on_activate.clone();
                move |row: &StatsTableRow| {
                    if let Some(others) = &row.others {
                        return html! {<i>{others}</i>};
                    }
                    let text = match (&row.key, kind) {
                        (Some(key), _) => key.clone(),
                        (None, StatsKind::Status) => tr!("Unknown"),
                        (None, _) => "-".to_string(),
                    };
                    match kind.drill_down(row.key.clone()) {
                        Some(fields) => {
                            let on_activate = on_activate.clone();
                            let onclick = move |event: MouseEvent| {
                                event.prevent_default();
                                event.stop_propagation();
                                on_activate.emit(fields.clone());
                            };
                            html! {<a href="#" {onclick}>{text}</a>}
                        }
                        None => html! {text},
                    }
                }
            })
            .into(),
        DataTableColumn::new(tr!("Requests"))
            .width("100px")
            .justify("end")
            .render(|row: &StatsTableRow| html! { row.requests })
            .into(),
        DataTableColumn::new(tr!("Client Errors"))
            .render_header(|args: &mut DataTableHeaderRenderArgs<StatsTableRow>| {
                args.set_attribute(
                    "title",
                    tr!("Error counts exclude requests with an unknown status."),
                );
                html! { tr!("Client Errors") }
            })
            .width("110px")
            .justify("end")
            .render(|row: &StatsTableRow| html! { row.client_errors })
            .into(),
        DataTableColumn::new(tr!("Server Errors"))
            .render_header(|args: &mut DataTableHeaderRenderArgs<StatsTableRow>| {
                args.set_attribute(
                    "title",
                    tr!("Error counts exclude requests with an unknown status."),
                );
                html! { tr!("Server Errors") }
            })
            .width("110px")
            .justify("end")
            .render(|row: &StatsTableRow| html! { row.server_errors })
            .into(),
        DataTableColumn::new(tr!("Last Seen"))
            .width("160px")
            .render(|row: &StatsTableRow| match row.last {
                Some(last) => render_epoch(last).into(),
                None => html! {},
            })
            .into(),
    ]);
    let activate = Callback::from({
        let store = store.clone();
        move |key: Key| {
            let store = store.read();
            let Some(row) = store.lookup_record(&key).filter(|row| row.others.is_none()) else {
                return;
            };
            if let Some(fields) = kind.drill_down(row.key.clone()) {
                on_activate.emit(fields);
            }
        }
    });
    DataTable::new(columns, store)
        .class("pwt-flex-fit")
        .on_row_keydown({
            let activate = activate.clone();
            move |event: &mut DataTableKeyboardEvent| {
                if event.key() == "Enter" {
                    event.prevent_default();
                    activate.emit(event.record_key.clone());
                }
            }
        })
        .on_row_dblclick(move |event: &mut DataTableMouseEvent| {
            activate.emit(event.record_key.clone());
        })
}
