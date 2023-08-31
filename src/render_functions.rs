/// epoch to "M d H:i:s" (localtime)
pub fn render_epoch_short(epoch: i64) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((epoch * 1000) as f64);

    let month_map = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    format!(
        "{} {:02} {:02}:{:02}:{:02}",
        month_map[date.get_month() as usize],
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds(),
    )
}

/// epoch to "Y-m-d H:i:s" (localtime)
pub fn render_epoch(epoch: i64) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((epoch * 1000) as f64);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds(),
    )
}
