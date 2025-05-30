// calculates the distance of the x axis labels + grid for base10 units
// The distance calculated is always between 1/2 and 1/10 of the range
fn get_grid_unit_base10(min: f64, max: f64) -> f64 {
    let range = max - min;

    if range <= 0.0 {
        panic!("get_grid_unit_base10: got zero or negative range - internal error");
    }

    let mut l = range.log10() as i32;

    // the count can be between 1 and 10
    if (range / 10.0_f64.powi(l)) < 2.0 {
        l -= 1;
    }

    // now it must be between 2 and 20

    let mut res = 10.0_f64.powi(l);

    let count = range / res;

    if count > 15.0 {
        res *= 5.0;
    } else if count > 10.0 {
        res *= 2.0;
    }
    // here the count must be between 2 and 10

    res
}

// calculates the distance of the x axis labels + grid for base2 units
// The distance calculated is always smaller than 1/4 of the range
fn get_grid_unit_base2(min: f64, max: f64) -> f64 {
    let range = max - min;

    if range <= 0.0 {
        panic!("get_grid_unit_base2: got zero or negative range - internal error");
    }

    let mut l = range.log2() as i32 - 2;

    if range / 2.0_f64.powi(l) < 4.0 {
        l -= 1;
    }

    2.0_f64.powi(l)
}

pub(crate) fn get_time_grid_unit(min: i64, max: i64) -> i64 {
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

    while (l >= *units.first().unwrap()) && (range / l) > 10 {
        l *= 2;
    }

    //log::info!("TIMERANG {l}");

    l
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct GraphKeyData {
    pub data_min: f64,
    pub data_max: f64,
    pub data_interval: f64,
    pub data_range: f64,

    pub time_min: i64,
    pub time_max: i64,
    pub time_interval: i64,
    pub start_time: i64,
    pub time_range: i64,
}

impl GraphKeyData {
    pub fn new(time_data: &[i64], data: &[&[f64]], include_zero: bool, binary: bool) -> Self {
        let (data_min, data_max, data_interval) = Self::data_parameters(data, include_zero, binary);
        let (time_min, time_max, time_interval, start_time) = Self::time_parameters(time_data);

        Self {
            data_min,
            data_max,
            data_interval,
            data_range: data_max - data_min,
            time_min,
            time_max,
            time_interval,
            start_time,
            time_range: time_max - time_min,
        }
    }

    fn data_parameters(data: &[&[f64]], include_zero: bool, binary: bool) -> (f64, f64, f64) {
        let mut min_data: f64 = f64::INFINITY;
        let mut max_data: f64 = -f64::INFINITY;

        for v in data.iter().flat_map(|d| d.iter()).filter(|v| v.is_finite()) {
            min_data = min_data.min(*v);
            max_data = max_data.max(*v);
        }

        // if one is infinite, the other must be too
        if min_data.is_infinite() || max_data.is_infinite() {
            min_data = 0.0;
            max_data = 1.0;
        }

        if include_zero {
            max_data = max_data.max(0.0);
            min_data = min_data.min(0.0);
        }

        // stretch to at least 0.0005 difference
        if (max_data - min_data) < 0.0005 {
            if min_data > 0.0003 {
                max_data += 0.0002;
                min_data -= 0.0003;
            } else {
                max_data += 0.0005;
            }
        }

        let interval = if binary {
            get_grid_unit_base2(min_data, max_data)
        } else {
            get_grid_unit_base10(min_data, max_data)
        };

        let snapped = (((min_data / interval) as i64) as f64) * interval;
        if snapped > min_data {
            min_data = snapped - interval;
        } else {
            min_data = snapped;
        }

        let snapped = (((max_data / interval) as i64) as f64) * interval;
        if snapped < max_data {
            max_data = snapped + interval;
        } else {
            max_data = snapped;
        }

        (min_data, max_data, interval)
    }

    fn time_parameters(time_data: &[i64]) -> (i64, i64, i64, i64) {
        let min_time = *time_data.first().unwrap_or(&0);
        let max_time = *time_data.last().unwrap_or(&0);

        let interval = get_time_grid_unit(min_time, max_time);

        // snap the start time point to the interval
        let start_time = ((min_time + interval - 1) / interval) * interval;

        (min_time, max_time, interval, start_time)
    }
}

#[cfg(test)]
mod test {
    use std::panic;

    use crate::rrd::units::get_time_grid_unit;

    use super::{get_grid_unit_base10, get_grid_unit_base2};

    const DELTA: f64 = 0.0000001;

    #[test]
    fn test_grid_unit_base2() {
        // min, max, result
        let test_data = [
            // normal range tests
            (0.0, 0.01, Some(2.0_f64.powi(-9))),
            (0.0, 2.0, Some(2.0_f64.powi(-1))),
            (0.0, 0.00001, Some(2.0_f64.powi(-19))),
            (0.0, 100.0, Some(2.0_f64.powi(4))),
            (0.0, 1_000_000.0, Some(2.0_f64.powi(17))),
            (0.0, f64::MAX, Some(2.0_f64.powi(1021))),
            (
                10.0 * 1024.0 * 1024.0,
                12.5 * 1024.0 * 1024.0,
                Some(2.0_f64.powi(19)),
            ),
            // ranges with negative data
            (-500.0, -100.0, Some(2.0_f64.powi(6))),
            (-500.0, 100.0, Some(2.0_f64.powi(7))),
            // panic tests
            (0.0, 0.0, None),
            (100.0, 0.01, None),
        ];

        for (min, max, expected) in test_data {
            match (
                panic::catch_unwind(|| get_grid_unit_base2(min, max)).ok(),
                expected,
            ) {
                (Some(result), Some(expected)) => {
                    let diff = result - expected;
                    assert_eq!(
                        diff < DELTA,
                        diff > -DELTA,
                        "{min} .. {max} ->\n {expected} \n {result}"
                    )
                }
                (None, Some(expected)) => {
                    panic!("panic'ed when it shouldn't: {min} .. {max} -> {expected}")
                }
                (Some(result), None) => {
                    panic!("result when it should have panic'ed: {min} .. {max} -> {result}")
                }
                (None, None) => {}
            }
        }
    }

    #[test]
    fn test_grid_unit_base10() {
        // min, max, result
        let test_data = [
            // normal range tests
            (0.0, 0.01, Some(0.001)),
            (0.0, 2.0, Some(1.0)),
            (0.0, 0.00001, Some(0.000002)),
            (0.0, 100.0, Some(10.0)),
            (0.0, 1_000_000.0, Some(100_000.0)),
            (
                0.0,
                f64::MAX,
                Some(5000000000000002.0 * 10.0_f64.powf(292.0)),
            ),
            (
                10.0 * 1024.0 * 1024.0,
                12.5 * 1024.0 * 1024.0,
                Some(1_000_000.0),
            ),
            // ranges with negative data
            (-500.0, -100.0, Some(100.0)),
            (-500.0, 100.0, Some(100.0)),
            // panic tests
            (0.0, 0.0, None),
            (100.0, 0.01, None),
        ];

        for (min, max, expected) in test_data {
            match (
                panic::catch_unwind(|| get_grid_unit_base10(min, max)).ok(),
                expected,
            ) {
                (Some(result), Some(expected)) => {
                    let diff = result - expected;
                    assert_eq!(
                        diff < DELTA,
                        diff > -DELTA,
                        "{min} .. {max} ->\n {expected} \n {result}"
                    )
                }
                (None, Some(expected)) => {
                    panic!("panic'ed when it shouldn't: {min} .. {max} -> {expected}")
                }
                (Some(result), None) => {
                    panic!("result when it should have panic'ed: {min} .. {max} -> {result}")
                }
                (None, None) => {}
            }
        }
    }

    #[test]
    fn test_time_grid_unit() {
        // min max result
        let test_data = [
            (0, 10, 1),
            (0, 100, 15),
            (0, 1_000_000, 172800),
            (0, i64::MAX, 1519964874237542400),
            (-1000, 1_000_000, 172800),
            (0, 0, 1),
            (1, 0, 1),
        ];

        for (min, max, expected) in test_data {
            assert_eq!(
                get_time_grid_unit(min, max),
                expected,
                "{min}..{max} -> {expected}"
            )
        }
    }
}
