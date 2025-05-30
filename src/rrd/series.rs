use yew::AttrValue;

/// Represents a series of data for an [`crate::RRDGraph`]
pub struct Series {
    pub label: AttrValue,
    pub data: Vec<f64>,
}

impl Series {
    pub fn new(label: impl Into<AttrValue>, data: Vec<f64>) -> Self {
        Self {
            label: label.into(),
            data,
        }
    }
}

/// Calculate the outline path of a series of [`f64`] data for [`i64`] points in time.
///
/// The line will not be drawn for points that are missing
pub fn compute_outline_path(
    time_data: &[i64],
    values: &[f64],
    compute_x: impl Fn(i64) -> f64,
    compute_y: impl Fn(f64) -> f64,
) -> String {
    let mut path = String::new();
    let mut last_undefined = true;
    for (i, t) in time_data.iter().enumerate() {
        let value = *values.get(i).unwrap_or(&f64::NAN);
        let x = compute_x(*t);

        if last_undefined {
            if value.is_nan() {
                continue;
            }
            last_undefined = false;
            let y = compute_y(value);
            path.push_str(&format!(" M {:.1} {:.1}", x, y));
        } else {
            if value.is_nan() {
                last_undefined = true;
                continue;
            }
            let y = compute_y(value);
            path.push_str(&format!(" L {:.1} {:.1}", x, y));
        }
    }
    path
}

/// Calculate the fill path for a series of [`f64`] points for [`i64`] points in time.
///
/// The area will not be filled for points that are missing
pub fn compute_fill_path(
    time_data: &[i64],
    values: &[f64],
    min_data: f64,
    max_data: f64,
    compute_x: impl Fn(i64) -> f64,
    compute_y: impl Fn(f64) -> f64,
) -> String {
    let mut y0 = compute_y(0.0);
    if min_data > 0.0 {
        y0 = compute_y(min_data)
    }
    if max_data < 0.0 {
        y0 = compute_y(max_data)
    }
    let mut path = String::new();
    let mut last_undefined = true;
    for i in 0..time_data.len() {
        let t = time_data[i];
        let value = *values.get(i).unwrap_or(&f64::NAN);

        let x = compute_x(t);

        if last_undefined {
            if value.is_nan() {
                continue;
            }
            last_undefined = false;
            path.push_str(&format!(" M {:.1} {:.1}", x, y0));
        } else if value.is_nan() {
            last_undefined = true;
            let x = if i > 0 {
                compute_x(time_data[i - 1])
            } else {
                x
            };
            path.push_str(&format!(" L {:.1} {:.1}", x, y0));

            continue;
        }
        let y = compute_y(value);
        path.push_str(&format!(" L {:.1} {:.1}", x, y));
    }

    if let Some(t) = time_data.last() {
        if !last_undefined {
            let x = compute_x(*t);
            path.push_str(&format!(" L {:.1} {:.1}", x, y0));
        }
    }

    path
}
