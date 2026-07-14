use std::collections::HashMap;

use percent_encoding::percent_decode;
use serde_json::Value;
use yew::prelude::*;

use crate::common_api_types::ProxmoxUpid;

use pwt::tr;

mod clipboard;
mod task_descriptions;

#[allow(deprecated)]
pub use clipboard::{copy_text_to_clipboard, copy_to_clipboard};

pub use task_descriptions::*;

/// Somewhat like a human would tell durations, omit zero values and do not
/// give seconds precision if we talk days already
pub fn format_duration_human(ut: f64) -> String {
    let mut minutes = 0;
    let mut hours = 0;
    let mut days = 0;
    let mut years = 0;

    if ut < 1.0 {
        return "<1s".into();
    }
    let mut remaining = ut as u64;
    let seconds = remaining % 60;
    remaining /= 60;
    if remaining > 0 {
        minutes = remaining % 60;
        remaining /= 60;
        if remaining > 0 {
            hours = remaining % 24;
            remaining /= 24;
            if remaining > 0 {
                days = remaining % 365;
                remaining /= 365; // yea, just lets ignore leap years...
                if remaining > 0 {
                    years = remaining;
                }
            }
        }
    }

    let mut parts = Vec::new();

    if years > 0 {
        parts.push(format!("{years}y"))
    };
    if days > 0 {
        parts.push(format!("{days}d"))
    };
    if hours > 0 {
        parts.push(format!("{hours}h"))
    };

    if years == 0 {
        if minutes > 0 {
            parts.push(format!("{minutes}m"))
        };
        if days == 0 && seconds > 0 {
            parts.push(format!("{seconds}s"))
        }
    }

    parts.join(" ")
}

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

/// epoch to "Y-m-dTH:i:sZ" (UTC)
pub fn render_epoch_utc(epoch: i64) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((epoch * 1000) as f64);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.get_utc_full_year(),
        date.get_utc_month() + 1,
        date.get_utc_date(),
        date.get_utc_hours(),
        date.get_utc_minutes(),
        date.get_utc_seconds(),
    )
}

pub fn render_boolean(v: bool) -> String {
    if v {
        tr!("Yes")
    } else {
        tr!("No")
    }
}

pub fn render_url(url: &str) -> Html {
    if url.starts_with("http://") || url.starts_with("https://") {
        html! {<a target="_blank" href={url.to_owned()}>{url}</a>}
    } else {
        html! {<span>{url}</span>}
    }
}

pub fn epoch_to_input_value(epoch: i64) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((epoch * 1000) as f64);

    if date.get_date() == 0 {
        // invalid data (clear field creates this)
        String::new()
    } else {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}",
            date.get_full_year(),
            date.get_month() + 1,
            date.get_date(),
            date.get_hours(),
            date.get_minutes(),
        )
    }
}

/// epoch to "Y-m-d" (localtime), the submit value format of a date input
pub fn epoch_to_input_date(epoch: i64) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((epoch * 1000) as f64);

    if date.get_date() == 0 {
        // invalid data (clear field creates this)
        String::new()
    } else {
        format!(
            "{:04}-{:02}-{:02}",
            date.get_full_year(),
            date.get_month() + 1,
            date.get_date(),
        )
    }
}

/// epoch to "H:i" (localtime), for a 24h time-of-day field complementing a date input
pub fn epoch_to_input_time(epoch: i64) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((epoch * 1000) as f64);

    format!("{:02}:{:02}", date.get_hours(), date.get_minutes())
}

/// Parse a "Y-m-dTH:i" string as localtime into a Unix epoch.
///
/// Counterpart of [`epoch_to_input_value`] and the epoch_to_input_date/time split. Unlike
/// `js_sys::Date::parse`, which leaves it engine-defined whether an offset-less string is read
/// as UTC or localtime, this always uses the local calendar, so values loaded through the
/// helpers above round-trip without a UTC offset shift.
pub fn parse_input_datetime(input: &str) -> Option<i64> {
    let input = input.trim();
    if input.len() < 16 {
        return None;
    }
    let bytes = input.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' || bytes[13] != b':' {
        return None;
    }
    let year: u32 = input.get(0..4)?.parse().ok()?;
    let month: i32 = input.get(5..7)?.parse().ok()?;
    let day: i32 = input.get(8..10)?.parse().ok()?;
    let hour: i32 = input.get(11..13)?.parse().ok()?;
    let minute: i32 = input.get(14..16)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    let date =
        js_sys::Date::new_with_year_month_day_hr_min_sec(year, month - 1, day, hour, minute, 0);
    let time_milli = date.get_time();
    if time_milli.is_nan() {
        return None;
    }
    // js Date silently rolls impossible dates over into the next month (February 31st becomes
    // March 3rd) and maps years below 100 to 1900..2000; require an exact component round-trip
    // instead of resolving to a shifted date.
    if date.get_full_year() != year
        || date.get_month() as i32 != month - 1
        || date.get_date() as i32 != day
    {
        return None;
    }
    Some((time_milli / 1000.0) as i64)
}

pub struct AuthDomainInfo {
    pub ty: String, // type
    //pub description: String,
    pub add: bool,
    pub edit: bool,
    pub tfa: bool,
    pub pwchange: bool,
    pub sync: bool,
}

pub fn get_auth_domain_info(ty: &str) -> Option<AuthDomainInfo> {
    if ty == "pam" {
        return Some(AuthDomainInfo {
            ty: ty.to_string(),
            //description: tr!("Linux PAM"),
            add: false,
            edit: true,
            tfa: true,
            pwchange: false,
            sync: false,
        });
    }

    if matches!(ty, "pve" | "pbs" | "pdm") {
        return Some(AuthDomainInfo {
            ty: ty.to_string(),
            //description: tr!("Proxmox VE authentication server"),
            add: false,
            edit: true,
            tfa: true,
            pwchange: true,
            sync: false,
        });
    }

    if ty == "openid" {
        return Some(AuthDomainInfo {
            ty: ty.to_string(),
            //description: tr!("OpenID Connect Server"),
            add: true,
            edit: true,
            tfa: false,
            pwchange: false,
            sync: false,
        });
    }

    if ty == "ldap" || ty == "ad" {
        return Some(AuthDomainInfo {
            ty: ty.to_string(),
            //description: tr!("LDAP Server"),
            add: true,
            edit: true,
            tfa: true,
            pwchange: false,
            sync: true,
        });
    }

    None
}

/// Convert JSON list of strings to flat, space separated string.
pub fn json_array_to_flat_string(list: &[Value]) -> String {
    let list: Vec<&str> = list
        .iter()
        .map(|p| p.as_str().unwrap_or(""))
        .filter(|p| !p.is_empty())
        .collect();
    list.join(" ")
}

/// Set the browser window.location.href
pub fn set_location_href(href: &str) {
    let window = gloo_utils::window();
    let location = window.location();
    let _ = location.set_href(href);
}

/// Uses information from the given `UPID` to render the task description with [`format_task_description`]
pub fn format_upid(upid: &str) -> String {
    match upid.parse::<ProxmoxUpid>() {
        Err(_) => upid.to_string(),
        Ok(upid) => {
            task_descriptions::format_task_description(&upid.worker_type, upid.worker_id.as_deref())
        }
    }
}

pub fn openid_redirection_authorization() -> Option<HashMap<String, String>> {
    let Ok(query_string) = gloo_utils::window().location().search() else {
        return None;
    };

    let mut auth = HashMap::new();
    let query_parameters = query_string.split('&');

    for param in query_parameters {
        let mut key_value = param.split('=');

        match (key_value.next(), key_value.next()) {
            (Some("?code") | Some("code"), Some(value)) => {
                if let Ok(code) = percent_decode(value.as_bytes()).decode_utf8() {
                    auth.insert("code".to_string(), code.to_string());
                }
            }
            (Some("?state") | Some("state"), Some(value)) => {
                if let Ok(decoded) = percent_decode(value.as_bytes()).decode_utf8() {
                    auth.insert("state".to_string(), decoded.to_string());
                }
            }
            _ => continue,
        };
    }

    if auth.contains_key("code") && auth.contains_key("state") {
        return Some(auth);
    }

    None
}
