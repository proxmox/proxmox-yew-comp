use super::units::GraphKeyData;

// Holds the basic data necessary for the SVG Layout
struct LayoutProps {
    width: usize,
    height: usize,
    grid_border: usize,
    left_offset: usize,
    bottom_offset: usize,
    inner_width: usize,
    inner_height: usize,
}

impl Default for LayoutProps {
    fn default() -> Self {
        Self {
            width: 800,
            height: 250,
            grid_border: 10,
            left_offset: 50,
            bottom_offset: 30,
            inner_width: 0,
            inner_height: 0,
        }
    }
}

// maps value in range 0.0..=1.0 to SVG coordinates on the y axis
fn map_relative_to_y(input: f64, layout: &LayoutProps) -> f64 {
    #[cfg(debug_assertions)]
    assert!((0.0..=1.0).contains(&input), "input: {input}");
    layout.inner_height as f64 * (1.0 - input) + layout.grid_border as f64
}

// maps value in range 0.0..=1.0 to SVG coordinates on the x axis
fn map_relative_to_x(input: f64, layout: &LayoutProps) -> f64 {
    #[cfg(debug_assertions)]
    assert!((0.0..=1.0).contains(&input), "input: {input}");
    layout.inner_width as f64 * input + (layout.left_offset + layout.grid_border) as f64
}

/// Holds all necessary information to calculate between data space and svg space
#[derive(Default)]
pub struct GraphSpace {
    layout: LayoutProps,
    pub graph_data: GraphKeyData,
}

/// Options for getting the boundaries of the SVG coordinates
pub enum CoordinateRange {
    /// Coordinates will be inside the border
    InsideBorder,
    /// Coordinates can include the border
    OutsideBorder,
}

impl GraphSpace {
    /// Update the graph space with new graph data
    pub fn update(&mut self, time_data: &[i64], data: &[&[f64]], include_zero: bool, binary: bool) {
        self.graph_data = GraphKeyData::new(time_data, data, include_zero, binary);
    }

    /// Converts from data space to svg space on the x axis
    pub fn compute_x(&self, x: i64) -> f64 {
        map_relative_to_x(
            (x - self.graph_data.time_min) as f64 / self.graph_data.time_range as f64,
            &self.layout,
        )
    }

    /// Converts from data space to svg space on the y axis
    pub fn compute_y(&self, y: f64) -> f64 {
        map_relative_to_y(
            (y - self.graph_data.data_min) / self.graph_data.data_range,
            &self.layout,
        )
    }

    /// Returns the minimum and maximum coordinates for the x axis
    pub fn get_x_range(&self, opts: CoordinateRange) -> (f64, f64) {
        let mut min = map_relative_to_x(0.0, &self.layout);
        if let CoordinateRange::OutsideBorder = opts {
            min -= self.layout.grid_border as f64;
        }
        let mut max = map_relative_to_x(1.0, &self.layout);
        if let CoordinateRange::OutsideBorder = opts {
            max += self.layout.grid_border as f64;
        }
        (min, max)
    }

    /// Returns the minimum and maximum coordinates for the y axis
    pub fn get_y_range(&self, opts: CoordinateRange) -> (f64, f64) {
        let mut min = map_relative_to_y(0.0, &self.layout);
        if let CoordinateRange::OutsideBorder = opts {
            min += self.layout.grid_border as f64;
        }
        let mut max = map_relative_to_y(1.0, &self.layout);
        if let CoordinateRange::OutsideBorder = opts {
            max -= self.layout.grid_border as f64;
        }
        (min, max)
    }

    /// Converts back from svg space to data space for the x axis
    pub fn original_x(&self, x: f64) -> i64 {
        let layout = &self.layout;
        let width = layout.inner_width as f64;
        let fraction: f64 = (x - (layout.left_offset + layout.grid_border) as f64) / width;

        ((fraction * (self.graph_data.time_range as f64)) as i64) + self.graph_data.time_min
    }

    /// Returns the complete current width of the graph
    pub fn get_width(&self) -> usize {
        self.layout.width
    }

    /// Returns the complete current height of the graph
    pub fn get_height(&self) -> usize {
        self.layout.height
    }

    fn update_inner_size(&mut self) {
        let layout = &mut self.layout;
        layout.inner_width = layout.width - layout.left_offset - layout.grid_border * 2;
        layout.inner_height = layout.height - layout.bottom_offset - layout.grid_border * 2;
    }

    /// Updates the width of the layout, recalculates all necessary fields
    pub fn set_width(&mut self, width: usize) {
        self.layout.width = width;
        self.update_inner_size();
    }

    /// Updates the left offset of the layout, recalculates all necessary fields
    pub fn set_left_offset(&mut self, offset: usize) {
        self.layout.left_offset = offset;
        self.update_inner_size();
    }

    /// Returns the left offset of the graph. This is useful for dynamically
    /// updating the space for the value labels
    pub fn get_left_offset(&self) -> usize {
        self.layout.left_offset
    }
}
