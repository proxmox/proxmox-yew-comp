use std::rc::Rc;

use yew::html::IntoPropValue;
use yew::prelude::*;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::Panel;

#[derive(Clone, PartialEq, Properties)]
pub struct RRDGraph {
    pub title: Option<AttrValue>,
    // Legend Label
    pub label: Option<String>,
    #[prop_or_default]
    pub class: Classes,

    pub data: Rc<(Vec<i64>, Vec<f64>)>,
}

impl RRDGraph {
    pub fn new(data: Rc<(Vec<i64>, Vec<f64>)>) -> Self {
        yew::props!(RRDGraph { data })
    }

    pub fn title(mut self, title: impl IntoPropValue<Option<AttrValue>>) -> Self {
        self.set_title(title);
        self
    }

    pub fn set_title(&mut self, title: impl IntoPropValue<Option<AttrValue>>) {
        self.title = title.into_prop_value();
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
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
}

pub enum Msg {
    Reload,
    StartSelection(i32, i32),
    EndSelection(i32),
    PointerMove(i32, i32),
    PointerEnter,
    PointerLeave,
    ClearViewRange,
}

pub struct PwtRRDGraph {
    canvas_ref: NodeRef,
    layout: LayoutProps,
    selection: Option<(usize, usize)>,
    view_range: Option<(usize, usize)>,
    captured_pointer_id: Option<i32>,
    draw_cross: bool,
    cross_pos: Option<(i32, i32)>,
}

pub struct LayoutProps {
    width: usize,
    height: usize,
    grid_border: usize,
    left_offset: usize,
    bottom_offset: usize,
}

impl Default for LayoutProps {
    fn default() -> Self {
        Self {
            width: 800,
            height: 250,
            grid_border: 10,
            left_offset: 50,
            bottom_offset: 50,
        }
    }
}

use pwt::widget::canvas::{Canvas, Circle, Path, Rect, SvgLength, Text};

fn get_grid_unit(min: f64, max: f64) -> f64 {
    let range = max - min;

    if range == 0.0 {
        return 0.1; // avoid returning 0 (avoid endless loops, division by zero)
    }

    let mut l = range.log10() as i32;

    while (range / (10.0 as f64).powi(l)) < 2.0 {
        l -= 1;
    }

    let mut res = (10.0 as f64).powi(l);

    let count = range / res;

    if count > 15.0 {
        res = res * 5.0;
    } else if count > 10.0 {
        res = res * 2.0;
    }

    res
}

fn get_time_grid_unit(min: i64, max: i64) -> i64 {
    let range = max - min;

    if range < 10 {
        // should not happen
        return 1;
    }

    let units = [
        3600 * 24,
        3600 * 12,
        3600 * 6,
        3600 * 4,
        3600 * 2,
        60 * 60,
        60 * 30,
        60 * 15,
        60 * 10,
        60 * 5,
        60 * 2,
        60,
        30,
        15,
        10,
        5,
        2,
        1,
    ];

    let mut l = *units.first().unwrap();
    for unit in units {
        if (range / unit) > 5 {
            l = unit;
            break;
        }
    }

    while (l > *units.first().unwrap()) && (range / l) > 10 {
        l = l * 2;
    }

    //log::info!("TIMERANG {l}");

    l
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
    let date = format!("{}/{}/{}", year, mon, day);

    (time, date)
}

impl PwtRRDGraph {
    fn get_view_data<'a>(&self, ctx: &'a Context<Self>) -> (&'a [i64], &'a [f64]) {
        let props = ctx.props();

        let data0 = &props.data.0;
        let data1 = &props.data.1;

        if let Some((start, end)) = self.view_range {
            (&data0[start..end], &data1[start..end])
        } else {
            (&data0, &data1)
        }
    }

    fn custom_view(&self, ctx: &Context<Self>) -> Html {
        let layout = &self.layout;

        let (data0, data1) = self.get_view_data(ctx);

        let start_time = data0.first().unwrap_or(&0);
        let end_time = data0.last().unwrap_or(&0);

        let mut min_data: Option<f64> = None;
        let mut max_data: Option<f64> = None;

        for v in data1.iter() {
            if v.is_nan() {
                continue;
            }
            if let Some(min) = min_data {
                if *v < min {
                    min_data = Some(*v);
                }
            } else {
                min_data = Some(*v);
            }
            if let Some(max) = max_data {
                if *v > max {
                    max_data = Some(*v);
                }
            } else {
                max_data = Some(*v);
            }
        }

        let mut max_data = max_data.unwrap_or(1.0);
        let mut min_data = min_data.unwrap_or(0.0);

        let points = data0.len();
        let time_span = (end_time - start_time) as f64;

        /*
        let shift = (max_data - min_data) / 3.0;
        for i in 0..points {
            data1[i] -= shift;
        }
        max_data -= shift;
        min_data -= shift;
        */

        let grid_unit = get_grid_unit(min_data, max_data);

        let t = (((min_data / grid_unit) as i64) as f64) * grid_unit;
        if t > min_data {
            min_data = t - grid_unit;
        } else {
            min_data = t;
        }

        let t = (((max_data / grid_unit) as i64) as f64) * grid_unit;
        if t < max_data {
            max_data = t + grid_unit;
        } else {
            max_data = t;
        }

        let data_range = max_data - min_data;

        //log::info!("POINTS {points} MIN {:?} MAX {:?}", min_data, max_data);

        /*
        if data1.len() > 100 {
            for i in 30..100 {
                data1[i] = f64::NAN;
            }
        }
        */

        let compute_x = {
            let width = (layout.width - layout.left_offset - layout.grid_border * 2) as f64;
            move |t: i64| -> f64 {
                (layout.left_offset + layout.grid_border) as f64
                    + (((t - start_time) as f64 * width) as f64) / time_span
            }
        };

        let compute_y = {
            let height = (layout.height - layout.bottom_offset - layout.grid_border * 2) as f64;
            move |value: f64| -> f64 {
                (layout.height - layout.grid_border - layout.bottom_offset) as f64
                    - ((value - min_data) * height) / data_range
            }
        };

        let y0 = compute_y(0.0);

        let compute_fill = |fill_dir: bool| -> String {
            let mut y0 = compute_y(0.0);
            if fill_dir {
                if min_data > 0.0 {
                    y0 = compute_y(min_data)
                }
            } else {
                if max_data < 0.0 {
                    y0 = compute_y(max_data)
                }
            }
            let mut path = format!("M 0 {y0}"); // not used
            let mut last_undefined = true;
            for i in 0..points {
                let t = data0[i];
                let mut value = *data1.get(i).unwrap_or(&f64::NAN);

                if fill_dir {
                    if value < 0.0 {
                        value = f64::NAN;
                    }
                } else {
                    if value > 0.0 {
                        value = f64::NAN;
                    }
                }

                let x = compute_x(t);

                if last_undefined {
                    if value.is_nan() {
                        continue;
                    }
                    last_undefined = false;
                    path.push_str(&format!(",M {x} {y0}"));
                } else {
                    if value.is_nan() {
                        last_undefined = true;
                        path.push_str(&format!(",L {x} {y0}"));

                        continue;
                    }
                }
                let y = compute_y(value);
                path.push_str(&format!(",L {x} {y}"));
            }

            if let Some(t) = data0.last() {
                if !last_undefined {
                    let x = compute_x(*t);
                    path.push_str(&format!(",L {x} {y0}"));
                }
            }

            path
        };

        let mut path = format!("M 0 {y0}");
        let mut last_undefined = true;
        for i in 0..points {
            let t = data0[i];
            let value = *data1.get(i).unwrap_or(&f64::NAN);
            let x = compute_x(t);

            if last_undefined {
                if value.is_nan() {
                    continue;
                }
                last_undefined = false;
                let y = compute_y(value);
                path.push_str(&format!(",M {x} {y}"));
            } else {
                if value.is_nan() {
                    last_undefined = true;
                    continue;
                }
                let y = compute_y(value);
                path.push_str(&format!(",L {x} {y}"));
            }
        }

        let pos_fill_path = compute_fill(true);
        let neg_fill_path = compute_fill(false);

        let mut grid_path = String::new();

        let mut value_labels: Vec<Html> = Vec::new();
        let mut time_labels: Vec<Html> = Vec::new();

        if let Some(start) = data0.first() {
            if let Some(end) = data0.last() {
                let x0 = compute_x(*start) - (layout.grid_border as f64);
                let x1 = compute_x(*end) + (layout.grid_border as f64);

                let mut v = min_data;
                while v <= max_data {
                    let y = compute_y(v);
                    grid_path.push_str(&format!("M {x0} {y}, L {x1} {y}"));

                    // round value to 4 relevant digits
                    let rounded_value = if v == 0.0 {
                        0.0
                    } else {
                        let mag = v.log10();
                        let base = 10.0f64.powf((mag + 4.0).floor());
                        (v * base).round() / base
                    };

                    value_labels.push(
                        Text::new(format!("{rounded_value}"))
                            .position(x0 as f32, y as f32)
                            .dy(SvgLength::Px(4.0))
                            .dx(SvgLength::Px(-4.0))
                            .attribute("text-anchor", "end")
                            .into(),
                    );

                    v += grid_unit;
                }

                let time_grid_unit = get_time_grid_unit(*start, *end);
                let mut t = ((*start + time_grid_unit - 1) / time_grid_unit) * time_grid_unit;
                let ymax = compute_y(max_data) - (layout.grid_border as f64);
                let ymin = compute_y(min_data) + (layout.grid_border as f64);

                let mut last_date = String::new();

                while t <= *end {
                    let x = compute_x(t);
                    grid_path.push_str(&format!("M {x} {ymin}, L {x} {ymax}"));

                    let (time, date) = format_time(t);

                    time_labels.push(
                        Text::new(time)
                            .position(x as f32, ymin as f32)
                            .dy(SvgLength::Px(10.0))
                            .attribute("text-anchor", "middle")
                            .into(),
                    );

                    if date != last_date {
                        time_labels.push(
                            Text::new(date.clone())
                                .position(x as f32, ymin as f32)
                                .dy(SvgLength::Px(10.0 + 16.0))
                                .attribute("text-anchor", "middle")
                                .into(),
                        );

                        last_date = date;
                    }

                    t += time_grid_unit;
                }
            }
        }

        let mut children: Vec<Html> = vec![
            Path::new().stroke("#94ae0a").fill("none").d(path).into(),
            Path::new()
                .key("grid-path")
                .stroke("black")
                .stroke_width(0.1)
                .d(grid_path)
                .into(),
            Path::new()
                .stroke("none")
                .fill("#94ae0a80")
                .d(pos_fill_path)
                .into(),
            Path::new()
                .stroke("none")
                .fill("#94ae0a80")
                .d(neg_fill_path)
                .into(),
        ];

        children.extend(value_labels);
        children.extend(time_labels);

        if let Some((start, end)) = &self.selection {
            let start = (*start).min(data0.len() - 1);
            let end = (*end).min(data0.len() - 1);

            let mut start_x = compute_x(data0[start]);
            let mut end_x = compute_x(data0[end]);

            if start_x > end_x {
                let t = start_x;
                start_x = end_x;
                end_x = t;
            }

            let start_y = compute_y(min_data);
            let end_y = compute_y(max_data);

            children.push(
                Rect::new()
                    .position(start_x as f32, end_y as f32)
                    .width((end_x - start_x) as f32)
                    .height((start_y - end_y) as f32)
                    .fill("#cccccc80")
                    .into(),
            );
        }

        if self.draw_cross {
            if let Some((x, y)) = self.cross_pos {
                let idx = self.offset_to_time_index(x, data0);

                if let Some(t) = data0.get(idx) {
                    if let Some(v) = data1.get(idx) {
                        let px = compute_x(*t) as f32;
                        let py = compute_y(*v) as f32;
                        children.push(
                            Circle::new()
                                .stroke("red")
                                .stroke_width(0.5)
                                .fill("none")
                                .position(px, py)
                                .r(5)
                                .into(),
                        )
                    }
                }

                let max_y = compute_y(min_data);
                let min_x = self.layout.left_offset;
                let max_x = self.layout.width;

                let x = x.max(min_x as i32);
                let y = y.min(max_y as i32);

                children.push(
                    Path::new()
                        .stroke("red")
                        .stroke_width(0.3)
                        .attribute("stroke-dasharray", "10 3")
                        .d(format!(
                            "M {x} 0, L {x} {max_y}, M {min_x} {y}, L {max_x} {y}"
                        ))
                        .into(),
                )
            }
        }

        Canvas::new()
            .node_ref(self.canvas_ref.clone())
            .class("proxmox-comp-rdd")
            .class("pwt-user-select-none")
            .class("pwt-pt-2 pwt-pe-2")
            .width(layout.width)
            .height(layout.height)
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
            .into()
    }

    fn offset_to_time_index(&self, x: i32, data0: &[i64]) -> usize {
        let layout = &self.layout;
        let width = (layout.width - layout.left_offset - layout.grid_border * 2) as f64;

        let start_time: i64 = *data0.first().unwrap_or(&0);
        let end_time: i64 = *data0.last().unwrap_or(&0);
        let time_span: i64 = end_time - start_time;

        let fraction: f64 = ((x - (layout.left_offset + layout.grid_border) as i32) as f64) / width;

        let t: i64 = ((fraction * (time_span as f64)) as i64) + start_time;
        let start_index = data0.partition_point(|&x| x < t);

        // Select nearest point
        if start_index > 0 {
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
        Self {
            canvas_ref: NodeRef::default(),
            layout: LayoutProps::default(),
            selection: None,
            view_range: None,
            captured_pointer_id: None,
            draw_cross: false,
            cross_pos: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        //let props = ctx.props();
        match msg {
            Msg::Reload => true,
            Msg::ClearViewRange => {
                self.view_range = None;
                true
            }
            Msg::PointerEnter => {
                self.draw_cross = true;
                true
            }
            Msg::PointerLeave => {
                self.draw_cross = false;
                true
            }
            Msg::StartSelection(x, pointer_id) => {
                self.captured_pointer_id = Some(pointer_id);
                if let Some(el) = self.canvas_ref.cast::<web_sys::Element>() {
                    let _ = el.set_pointer_capture(pointer_id);
                }
                let (data0, _) = self.get_view_data(ctx);
                let start_index = self.offset_to_time_index(x, data0);
                self.selection = Some((start_index, start_index));
                true
            }
            Msg::PointerMove(x, y) => {
                self.cross_pos = Some((x, y));
                self.selection = match self.selection {
                    Some((start, _)) => {
                        let (data0, _) = self.get_view_data(ctx);
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
                        let (data0, _) = self.get_view_data(ctx);
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

                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        log::info!("FIXME DATA CHANGE");
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Panel::new()
            .title(props.title.clone())
            .class(props.class.clone())
            .with_child(self.custom_view(ctx))
            .into()
    }
}

impl Into<VNode> for RRDGraph {
    fn into(self) -> VNode {
        let comp = VComp::new::<PwtRRDGraph>(Rc::new(self), None);
        VNode::from(comp)
    }
}
