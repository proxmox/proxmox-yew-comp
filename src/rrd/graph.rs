use std::rc::Rc;

use derivative::Derivative;

use yew::html::IntoPropValue;
use yew::prelude::*;
use yew::virtual_dom::{VComp, VNode};

use pwt::dom::align::{align_to, AlignOptions};
use pwt::dom::DomSizeObserver;
use pwt::prelude::*;
use pwt::props::{IntoOptionalTextRenderFn, TextRenderFn};
use pwt::state::optional_rc_ptr_eq;
use pwt::widget::{Button, Container, Panel};

use pwt_macros::builder;

#[derive(Derivative)]
#[derivative(Clone, PartialEq)]
#[derive(Properties)]
#[builder]
pub struct RRDGraph {
    #[prop_or_default]
    pub title: Option<AttrValue>,
    // Legend Label
    #[prop_or_default]
    pub label: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,

    #[prop_or_default]
    pub time_data: Rc<Vec<i64>>,

    #[prop_or_default]
    #[derivative(PartialEq(compare_with = "optional_rc_ptr_eq"))]
    #[prop_or_default]
    pub serie0: Option<Rc<Series>>,

    #[prop_or_default]
    #[derivative(PartialEq(compare_with = "optional_rc_ptr_eq"))]
    #[prop_or_default]
    pub serie1: Option<Rc<Series>>,

    /// Always include zero in displayed data range.
    #[prop_or(true)]
    #[builder]
    pub include_zero: bool,

    /// Compute axis range using log2 instead of log10.
    #[prop_or_default]
    #[builder]
    pub binary: bool,

    #[prop_or_default]
    pub render_value: Option<TextRenderFn<f64>>,
}

impl RRDGraph {
    pub fn new(time_data: Rc<Vec<i64>>) -> Self {
        yew::props!(RRDGraph { time_data })
    }

    pub fn serie0(mut self, serie: Option<Rc<Series>>) -> Self {
        self.serie0 = serie;
        self
    }

    pub fn serie1(mut self, serie: Option<Rc<Series>>) -> Self {
        self.serie1 = serie;
        self
    }

    pub fn title(mut self, title: impl IntoPropValue<Option<AttrValue>>) -> Self {
        self.set_title(title);
        self
    }

    pub fn set_title(&mut self, title: impl IntoPropValue<Option<AttrValue>>) {
        self.title = title.into_prop_value();
    }

    pub fn label(mut self, label: impl IntoPropValue<Option<AttrValue>>) -> Self {
        self.label = label.into_prop_value();
        self
    }

    /// Builder style method to add a html class
    pub fn class(mut self, class: impl Into<Classes>) -> Self {
        self.add_class(class);
        self
    }

    /// Method to add a html class
    pub fn add_class(&mut self, class: impl Into<Classes>) {
        self.class.push(class);
    }

    pub fn render_value(mut self, renderer: impl IntoOptionalTextRenderFn<f64>) -> Self {
        self.render_value = renderer.into_optional_text_render_fn();
        self
    }
}

pub enum Msg {
    Reload,
    ViewportResize(f64, f64),
    AdjustLeftOffset(usize),
    StartSelection(i32, i32),
    EndSelection(i32),
    PointerMove(i32, i32),
    PointerEnter,
    PointerLeave,
    ClearViewRange,
    ToggleSeries(u32), // index
}

pub struct PwtRRDGraph {
    node_ref: NodeRef,
    size_observer: Option<DomSizeObserver>,
    canvas_ref: NodeRef,
    graph_space: GraphSpace,
    selection: Option<(usize, usize)>,
    view_range: Option<(usize, usize)>,
    captured_pointer_id: Option<i32>,
    cross_pos: Option<(i32, i32)>,
    tooltip_align_ref: NodeRef,
    tooltip_ref: NodeRef,
    y_label_ref: NodeRef,
    serie0_visible: bool,
    serie1_visible: bool,
    grid: RrdGrid,
    series_paths: Vec<Option<(String, String)>>, //outline path, fill path
}

use pwt::widget::canvas::{Canvas, Circle, Group, Path, Rect};

use super::graph_space::{CoordinateRange, GraphSpace};
use super::grid::RrdGrid;
use super::series::{compute_fill_path, compute_outline_path};
use super::Series;

fn format_date_time(t: i64) -> String {
    let (time, date) = format_time(t);
    format!("{date} {time}")
}

fn format_time(t: i64) -> (String, String) {
    let date = js_sys::Date::new_0();
    date.set_time((t * 1000) as f64);
    let h = date.get_hours();
    let m = date.get_minutes();

    let time = format!("{:02}:{:02}", h, m);

    let year = date.get_full_year();
    let mon = date.get_month() + 1;
    let day = date.get_date();
    let date = format!("{}-{:02}-{:02}", year, mon, day);

    (time, date)
}

fn reduce_float_precision(v: f64) -> f64 {
    if v == 0.0 {
        return 0.0;
    }

    let mag = v.abs().log10().floor();

    if mag > 0.0 {
        let base = 10.0f64.powf(mag.min(3.0));
        (v * base).round() / base
    } else {
        let base = 10.0f64.powf(3.0 - mag);
        (v * base).round() / base
    }
}

fn render_value(props: &RRDGraph, v: f64) -> String {
    match &props.render_value {
        Some(render) => render.apply(&v),
        None => reduce_float_precision(v).to_string(),
    }
}

impl PwtRRDGraph {
    fn update_grid_content(&mut self, ctx: &Context<Self>) {
        let props = ctx.props();
        let (time_data, data1, data2) = self.get_view_data(ctx);
        self.graph_space
            .update(time_data, &[data1, data2], props.include_zero, props.binary);
        self.grid = RrdGrid::new(&self.graph_space);

        let mut paths = Vec::new();
        if self.serie0_visible {
            let outline_path = compute_outline_path(time_data, data1, &self.graph_space);
            let fill_path = compute_fill_path(time_data, data1, &self.graph_space);
            paths.push(Some((outline_path, fill_path)));
        } else {
            paths.push(None);
        }
        if self.serie1_visible {
            let outline_path = compute_outline_path(time_data, data2, &self.graph_space);
            let fill_path = compute_fill_path(time_data, data2, &self.graph_space);
            paths.push(Some((outline_path, fill_path)));
        } else {
            paths.push(None);
        }

        self.series_paths = paths;
    }

    fn get_view_data<'a>(&self, ctx: &'a Context<Self>) -> (&'a [i64], &'a [f64], &'a [f64]) {
        let props = ctx.props();

        let time_data = &props.time_data;
        let serie0_data = match (self.serie0_visible, &props.serie0) {
            (true, Some(serie)) => &serie.data[..],
            _ => &[],
        };
        let serie1_data = match (self.serie1_visible, &props.serie1) {
            (true, Some(serie)) => &serie.data[..],
            _ => &[],
        };

        if let Some((start, end)) = self.view_range {
            let serie0_start = start.min(serie0_data.len().saturating_sub(1));
            let serie0_end = end.min(serie0_data.len());
            let serie1_start = start.min(serie1_data.len().saturating_sub(1));
            let serie1_end = end.min(serie1_data.len());
            (
                &time_data[start..end],
                &serie0_data[serie0_start..serie0_end],
                &serie1_data[serie1_start..serie1_end],
            )
        } else {
            (time_data, serie0_data, serie1_data)
        }
    }

    fn create_graph(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let (data0, data1, data2) = self.get_view_data(ctx);

        let mut children: Vec<Html> = Vec::new();

        // draw grid and labels
        let (value_labels, time_labels) = self
            .grid
            .to_label_list(|x| render_value(props, x), format_time);

        children.push(self.grid.to_path().key("grid").into());
        children.push(Group::new().key("time-labels").children(time_labels).into());

        children.push(
            Group::new()
                .key("value-labels")
                .children(value_labels)
                .into_html_with_ref(self.y_label_ref.clone()),
        );

        // draw series
        for (idx, series) in self.series_paths.iter().enumerate() {
            let idx = idx + 1;
            let (outline_path, fill_path) = match series {
                Some(res) => res,
                None => continue,
            };
            children.extend(vec![
                Path::new()
                    .key(format!("series{idx}-path"))
                    .class(format!("pwt-rrd-outline-path{idx}"))
                    .d(outline_path.to_string())
                    .into(),
                Path::new()
                    .key(format!("series{idx}-fill"))
                    .class(format!("pwt-rrd-fill-path{idx}"))
                    .d(fill_path.to_string())
                    .into(),
            ]);
        }

        // draw selection rectangle
        if let Some((start, end)) = &self.selection {
            match (data0.get(*start), data0.get(*end)) {
                (Some(start_data), Some(end_data)) => {
                    let (x, y, width, height) = self.get_selection_rect(*start_data, *end_data);
                    children.push(
                        Rect::new()
                            .key("selection-rect")
                            .class("pwt-rrd-selection")
                            .position(x, y)
                            .width(width)
                            .height(height)
                            .into(),
                    );
                }
                _ if data0.is_empty() => {}
                _ => log::debug!("out of bound selection start {start}, end {end} for {data0:?}"),
            }
        }

        // draw cross and data circles
        if let Some((x, y)) = self.cross_pos {
            let (path, circles) = self.get_cross_positions(data0, &[data1, data2], x, y);
            for (idx, (px, py)) in circles.into_iter().enumerate() {
                children.push(
                    Circle::new()
                        .key(format!("selection-circle{idx}"))
                        .class("pwt-rrd-selected-datapoint")
                        .position(px, py)
                        .r(5)
                        .into(),
                );
            }

            children.push(
                Path::new()
                    .key("cross")
                    .class("pwt-rrd-cross")
                    .d(path)
                    .into(),
            );

            children.push(
                Circle::new()
                    .fill("none")
                    .stroke("none")
                    .position(x, y)
                    .r(1)
                    .into_html_with_ref(self.tooltip_align_ref.clone()),
            )
        }

        Canvas::new()
            .class("pwt-rrd-svg")
            .width(self.graph_space.get_width())
            .height(self.graph_space.get_height())
            .children(children)
            .ondblclick(ctx.link().callback(|_| Msg::ClearViewRange))
            .onpointerenter(ctx.link().callback(|_| Msg::PointerEnter))
            .onpointerleave(ctx.link().callback(|_| Msg::PointerLeave))
            .onpointerdown({
                let link = ctx.link().clone();
                move |event: PointerEvent| {
                    if !event.shift_key() {
                        link.send_message(Msg::StartSelection(
                            event.offset_x(),
                            event.pointer_id(),
                        ));
                    }
                }
            })
            .onpointerup(
                ctx.link()
                    .callback(|event: PointerEvent| Msg::EndSelection(event.offset_x())),
            )
            .onpointermove(ctx.link().callback(|event: PointerEvent| {
                Msg::PointerMove(event.offset_x(), event.offset_y())
            }))
            .into_html_with_ref(self.canvas_ref.clone())
    }

    // returns x, y, width, height
    fn get_selection_rect(&self, start: i64, end: i64) -> (f32, f32, f32, f32) {
        let mut start_x = self.graph_space.compute_x(start);
        let mut end_x = self.graph_space.compute_x(end);

        if start_x > end_x {
            std::mem::swap(&mut start_x, &mut end_x);
        }

        let (start_y, end_y) = self.graph_space.get_y_range(CoordinateRange::InsideBorder);

        (
            start_x as f32,
            end_y as f32,
            (end_x - start_x) as f32,
            (start_y - end_y) as f32,
        )
    }

    // returns the path for the cross and the positions of the circles for the series data points
    fn get_cross_positions(
        &self,
        data0: &[i64],
        series: &[&[f64]],
        x: i32,
        y: i32,
    ) -> (String, Vec<(f32, f32)>) {
        let idx = self.offset_to_time_index(x, data0);

        let mut children = Vec::new();

        if let Some(t) = data0.get(idx) {
            for data in series {
                if let Some(v) = data.get(idx) {
                    if v.is_finite() {
                        let px = self.graph_space.compute_x(*t) as f32;
                        let py = self.graph_space.compute_y(*v) as f32;
                        children.push((px, py));
                    }
                }
            }
        }

        let (min_y, _) = self.graph_space.get_y_range(CoordinateRange::InsideBorder);
        let (min_x, max_x) = self.graph_space.get_x_range(CoordinateRange::InsideBorder);

        let x = x.max(min_x as i32).min(max_x as i32);
        let y = y.min(min_y as i32);

        let path = format!("M {x} 0 L {x} {min_y} M {min_x} {y} L {max_x} {y}");

        (path, children)
    }

    fn offset_to_time_index(&self, x: i32, data0: &[i64]) -> usize {
        let t = self.graph_space.original_x(x as f64);
        let start_index = data0.partition_point(|&x| x < t);

        // Select nearest point
        if start_index > 0 {
            if start_index >= data0.len() {
                return data0.len() - 1;
            }

            if let Some(next_t) = data0.get(start_index) {
                if let Some(prev_t) = data0.get(start_index - 1) {
                    if (t - prev_t) < (next_t - t) {
                        return start_index - 1;
                    }
                }
            }
        }

        //log::info!("START SELECTION {x} {fraction} {start_time} {t} {end_time} {start_index}");

        start_index
    }
}

impl Component for PwtRRDGraph {
    type Message = Msg;
    type Properties = RRDGraph;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Reload);

        let graph_space = GraphSpace::default();
        let grid = RrdGrid::new(&graph_space);

        let mut this = Self {
            node_ref: NodeRef::default(),
            size_observer: None,
            canvas_ref: NodeRef::default(),
            graph_space,
            selection: None,
            view_range: None,
            captured_pointer_id: None,
            cross_pos: None,
            tooltip_align_ref: NodeRef::default(),
            tooltip_ref: NodeRef::default(),
            y_label_ref: NodeRef::default(),
            serie0_visible: true,
            serie1_visible: true,
            grid,
            series_paths: Vec::new(),
        };

        this.update_grid_content(ctx);
        this
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        //let props = ctx.props();
        match msg {
            Msg::Reload => true,
            Msg::ViewportResize(width, _height) => {
                if width > 0.0 {
                    self.graph_space.set_width(width as usize);
                    self.update_grid_content(ctx);
                }
                true
            }
            Msg::ToggleSeries(idx) => {
                if idx == 0 {
                    self.serie0_visible = !self.serie0_visible;
                    if !(self.serie0_visible || self.serie1_visible) {
                        self.serie1_visible = true;
                    }
                } else if idx == 1 {
                    self.serie1_visible = !self.serie1_visible;
                    if !(self.serie0_visible || self.serie1_visible) {
                        self.serie0_visible = true;
                    }
                }
                self.update_grid_content(ctx);
                true
            }
            Msg::ClearViewRange => {
                self.view_range = None;
                self.update_grid_content(ctx);
                true
            }
            Msg::AdjustLeftOffset(offset) => {
                self.graph_space.set_left_offset(offset);
                self.update_grid_content(ctx);
                true
            }
            Msg::PointerEnter => {
                self.cross_pos = None;
                true
            }
            Msg::PointerLeave => {
                self.cross_pos = None;
                true
            }
            Msg::StartSelection(x, pointer_id) => {
                self.captured_pointer_id = Some(pointer_id);
                if let Some(el) = self.canvas_ref.cast::<web_sys::Element>() {
                    let _ = el.set_pointer_capture(pointer_id);
                }
                let (data0, _, _) = self.get_view_data(ctx);
                let start_index = self.offset_to_time_index(x, data0);
                self.selection = Some((start_index, start_index));
                true
            }
            Msg::PointerMove(x, y) => {
                self.cross_pos = Some((x, y));
                self.selection = match self.selection {
                    Some((start, _)) => {
                        let (data0, _, _) = self.get_view_data(ctx);
                        let end_index = self.offset_to_time_index(x, data0);
                        //log::info!("Move SELECTION {start} {end_index}");
                        Some((start, end_index))
                    }
                    None => None,
                };
                true
            }
            Msg::EndSelection(x) => {
                if let Some(el) = self.canvas_ref.cast::<web_sys::Element>() {
                    if let Some(pointer_id) = self.captured_pointer_id.take() {
                        let _ = el.set_pointer_capture(pointer_id);
                    }
                }
                self.selection = match self.selection {
                    Some((start, _)) => {
                        let (data0, _, _) = self.get_view_data(ctx);
                        let end_index = self.offset_to_time_index(x, data0);
                        let (start, end_index) = if start > end_index {
                            (end_index, start)
                        } else {
                            (start, end_index)
                        };

                        if (end_index - start) > 10 {
                            //log::info!("End SELECTION {start} {end_index}");
                            match self.view_range {
                                Some((view_start, _view_end)) => {
                                    self.view_range =
                                        Some((view_start + start, view_start + end_index));
                                }
                                None => {
                                    self.view_range = Some((start, end_index));
                                }
                            }
                        }
                        None
                    }
                    None => None,
                };
                self.update_grid_content(ctx);

                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let mut data_time = None;
        let mut serie0_value = None;
        let mut serie1_value = None;

        if let Some((x, _)) = self.cross_pos {
            let (data0, data1, data2) = self.get_view_data(ctx);
            let idx = self.offset_to_time_index(x, data0);
            if let Some(t) = data0.get(idx) {
                data_time = Some(format_date_time(*t));
                if let Some(v) = data1.get(idx) {
                    if v.is_finite() {
                        serie0_value = Some(render_value(props, *v));
                    }
                }
                if let Some(v) = data2.get(idx) {
                    if v.is_finite() {
                        serie1_value = Some(render_value(props, *v));
                    }
                }
            }
        }

        let tip = Container::new()
        .attribute("role", "tooltip")
        .attribute("aria-live", "polite")
        .attribute("data-show", (self.cross_pos.is_some() && data_time.is_some()).then_some(""))
        .class("pwt-tooltip")
        .class("pwt-tooltip-rich")
        .with_optional_child(match (self.serie0_visible, &props.serie0) {
            (true, Some(serie)) => Some(html!{<div>{format!("{}: {}", serie.label.clone(), serie0_value.as_deref().unwrap_or("-"))}</div>}),
            _ => None,
        })
        .with_optional_child(match (self.serie1_visible, &props.serie1) {
            (true, Some(serie)) => Some(html!{<div>{format!("{}: {}", serie.label.clone(), serie1_value.as_deref().unwrap_or("-"))}</div>}),
            _ => None,
        })
        .with_child(html!{<hr/>})
        .with_child(html!{<div>{data_time.as_deref().unwrap_or("-")}</div>})
        .into_html_with_ref(self.tooltip_ref.clone());

        let mut panel = Panel::new()
            .title(props.title.clone())
            .class(props.class.clone())
            .class("pwt-rrd-panel")
            .class("pwt-overflow-auto")
            .with_child(
                Container::new()
                    .class("pwt-rrd-container")
                    .class("pwt-flex-fill pwt-overflow-auto")
                    .with_child(self.create_graph(ctx))
                    .with_child(tip)
                    .into_html_with_ref(self.node_ref.clone()),
            );

        if let Some(serie0) = &props.serie0 {
            if let Some(serie1) = &props.serie1 {
                let icon_class0 = classes!(
                    "pwt-rrd-legend-marker0",
                    "fa",
                    "fa-circle",
                    (!self.serie0_visible).then_some("disabled")
                );
                panel.add_tool(
                    Button::new(serie0.label.clone())
                        .class("pwt-button-elevated")
                        .icon_class(icon_class0)
                        .onclick(ctx.link().callback(|_| Msg::ToggleSeries(0))),
                );
                let icon_class1 = classes!(
                    "pwt-rrd-legend-marker1",
                    "fa",
                    "fa-circle",
                    (!self.serie1_visible).then_some("disabled")
                );
                panel.add_tool(
                    Button::new(serie1.label.clone())
                        .class("pwt-button-elevated")
                        .icon_class(icon_class1)
                        .onclick(ctx.link().callback(|_| Msg::ToggleSeries(1))),
                );
            }
        }

        panel.into()
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();

        // clamp view range to the new time data range
        if let Some((start, end)) = self.view_range {
            if props.time_data.len() < 10 {
                self.view_range = None;
            } else {
                let end = end.min(props.time_data.len() - 1);
                let start = start.min(end - 10);
                self.view_range = Some((start, end));
            }
        }

        // we need to recalculate the grid content when the series or time data changes
        if props.serie0 != old_props.serie0
            || props.serie1 != old_props.serie1
            || props.time_data != old_props.time_data
        {
            self.update_grid_content(ctx);
        }

        true
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            if let Some(el) = self.node_ref.cast::<web_sys::Element>() {
                let link = ctx.link().clone();
                let size_observer = DomSizeObserver::new(&el, move |(width, height)| {
                    link.send_message(Msg::ViewportResize(width, height));
                });
                self.size_observer = Some(size_observer);
            }
        }
        if let Some(align_node) = self.tooltip_align_ref.get() {
            if let Some(tooltip_node) = self.tooltip_ref.get() {
                let _ = align_to(
                    align_node,
                    tooltip_node,
                    Some(AlignOptions::default().offset(20.0, 20.0)),
                );
            }
        }
        if let Some(el) = self.y_label_ref.cast::<web_sys::SvgsvgElement>() {
            if let Ok(bbox) = el.get_b_box() {
                let offset = (bbox.width() + 10.0) as usize;
                if self.graph_space.get_left_offset() != offset {
                    ctx.link().send_message(Msg::AdjustLeftOffset(offset));
                }
            }
        }
    }
}

impl From<RRDGraph> for VNode {
    fn from(val: RRDGraph) -> Self {
        let comp = VComp::new::<PwtRRDGraph>(Rc::new(val), None);
        VNode::from(comp)
    }
}
