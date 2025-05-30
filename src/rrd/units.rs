pub(crate) fn get_grid_unit_base10(min: f64, max: f64) -> f64 {
    let range = max - min;

    if range == 0.0 {
        panic!("get_grid_unit_base10: got zero range - internal error");
    }

    let mut l = range.log10() as i32;

    while (range / 10.0_f64.powi(l)) < 2.0 {
        l -= 1;
    }

    let mut res = 10.0_f64.powi(l);

    let count = range / res;

    if count > 15.0 {
        res *= 5.0;
    } else if count > 10.0 {
        res *= 2.0;
    }

    res
}

pub(crate) fn get_grid_unit_base2(min: f64, max: f64) -> f64 {
    let range = max - min;

    if range == 0.0 {
        panic!("get_grid_unit_base2: got zero range - internal error");
    }

    let mut l = range.log2() as i32;

    while (range / 2.0_f64.powi(l)) < 4.0 {
        l -= 1;
    }

    let mut res = 2.0_f64.powi(l);

    let count = range / res;

    if count > 15.0 {
        res *= 2.0;
    }

    res
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
