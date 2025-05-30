use pwt::{
    props::WidgetBuilder,
    widget::canvas::{Path, SvgLength, Text},
};
use yew::Html;

use super::graph_space::{CoordinateRange, GraphSpace};

/// Holds the coordinates and path for the rrd grid and labels, so we don't have to recalculate
/// them too often
pub(crate) struct RrdGrid {
    x_points: XPoints,
    y_points: YPoints,
    grid_path: String,

    x0: f64,
    y0: f64,
}

pub type XPoints = Vec<(f64, i64)>;
pub type YPoints = Vec<(f64, f64)>;

impl RrdGrid {
    /// Calculates the correct points for the grid and labels with help from [`GraphSpace`]
    pub fn new(graph_space: &GraphSpace) -> Self {
        let (x_points, y_points) = Self::calculate_grid_points(graph_space);
        let grid_path = Self::create_svg_path(&x_points, &y_points, graph_space);

        let (x0, _) = graph_space.get_x_range(CoordinateRange::OutsideBorder);
        let (y0, _) = graph_space.get_y_range(CoordinateRange::OutsideBorder);

        Self {
            x_points,
            y_points,
            grid_path,
            x0,
            y0,
        }
    }

    /// returns a [`Path`] object for the RRD grid
    pub fn to_path(&self) -> Path {
        Path::new().class("pwt-rrd-grid").d(self.grid_path.clone())
    }

    /// Returns a list of labels (using a [`Text`] component) for the values (y axis) and points in time (x axis)
    pub fn to_label_list<R, T>(&self, render_value: R, format_time: T) -> (Vec<Html>, Vec<Html>)
    where
        R: Fn(f64) -> String,
        T: Fn(i64) -> (String, String),
    {
        let mut value_labels: Vec<Html> = Vec::new();
        let mut time_labels: Vec<Html> = Vec::new();

        for (y, v) in &self.y_points {
            let label = render_value(*v);
            value_labels.push(
                Text::new(label)
                    .class("pwt-rrd-label-text")
                    .position(self.x0 as f32, *y as f32)
                    .dy(SvgLength::Px(4.0))
                    .dx(SvgLength::Px(-4.0))
                    .attribute("text-anchor", "end")
                    .into(),
            );
        }

        let mut last_date = String::new();
        for (x, t) in &self.x_points {
            let (time, date) = format_time(*t);

            time_labels.push(
                Text::new(time)
                    .class("pwt-rrd-label-text")
                    .position(*x as f32, self.y0 as f32)
                    .dy(SvgLength::Px(10.0))
                    .attribute("text-anchor", "middle")
                    .into(),
            );

            if date != last_date {
                time_labels.push(
                    Text::new(date.clone())
                        .class("pwt-rrd-label-text")
                        .position(*x as f32, self.y0 as f32)
                        .dy(SvgLength::Px(10.0 + 16.0))
                        .attribute("text-anchor", "middle")
                        .into(),
                );

                last_date = date;
            }
        }

        (value_labels, time_labels)
    }

    // maps the coordinates of the grid points from data space to svg space
    fn calculate_grid_points(graph_space: &GraphSpace) -> (XPoints, YPoints) {
        let mut x_points = Vec::new();
        let mut y_points = Vec::new();

        let parameters = &graph_space.graph_data;

        let mut v = parameters.data_min;
        if parameters.data_range > 0.0 {
            while v <= parameters.data_max {
                let y = graph_space.compute_y(v);
                y_points.push((y, v));
                v += parameters.data_interval;
            }
        }

        let mut t = parameters.start_time;

        if parameters.time_range > 0 {
            while t <= parameters.time_max {
                let x = graph_space.compute_x(t);
                x_points.push((x, t));
                t += parameters.time_interval;
            }
        }
        (x_points, y_points)
    }

    // creates the svg path for the grid lines
    fn create_svg_path(
        x_points: &[(f64, i64)],
        y_points: &[(f64, f64)],
        graph_space: &GraphSpace,
    ) -> String {
        let mut grid_path = String::new();

        let (x0, x1) = graph_space.get_x_range(CoordinateRange::OutsideBorder);
        let (y0, y1) = graph_space.get_y_range(CoordinateRange::OutsideBorder);

        for (y, _) in y_points {
            grid_path.push_str(&format!("M {:.1} {:.1} L {:.1} {:.1}", x0, y, x1, y));
        }

        for (x, _) in x_points {
            grid_path.push_str(&format!("M {:.1} {:.1} L {:.1} {:.1}", x, y0, x, y1));
        }

        grid_path
    }
}
