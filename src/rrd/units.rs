// calculates the distance of the x axis labels + grid for base10 units
// The distance calculated is always between 1/2 and 1/10 of the range
pub(crate) fn get_grid_unit_base10(min: f64, max: f64) -> f64 {
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
pub(crate) fn get_grid_unit_base2(min: f64, max: f64) -> f64 {
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
