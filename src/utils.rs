use std::collections::HashMap;
use std::sync::Mutex;
use std::fmt::Display;

use yew::prelude::*;

use proxmox_schema::upid::UPID;

use pwt::tr;

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
    if v { tr!("Yes") } else { tr!("No") }
}

// todo: we want to use Fn(&str, Option<&str>),
static TASK_DESCR_TABLE: Mutex<Option<HashMap<String, Box<dyn Send + Sync + Fn(String, Option<String>) -> String>>>> = Mutex::new(None);

pub trait IntoTaskDescriptionRenderFn {
    fn into_task_description_render_fn(self) -> Box<dyn Send + Sync + Fn(String, Option<String>) -> String>;
}

impl <F: 'static + Send + Sync + Fn(String, Option<String>) -> String> IntoTaskDescriptionRenderFn for F {
    fn into_task_description_render_fn(self) -> Box<dyn Send + Sync + Fn(String, Option<String>) -> String> {
        Box::new(self)
    }
}

impl <A: Display, B: Display> IntoTaskDescriptionRenderFn for (A, B) {
    fn into_task_description_render_fn(self) -> Box<dyn 'static + Send + Sync + Fn(String, Option<String>) -> String> {
        let task_type = self.0.to_string();
        let action = self.1.to_string();
        Box::new(move |_, id| {
            format!("{} {} {}", task_type, id.as_deref().unwrap_or("unknown"), action)
        })
    }
}

impl IntoTaskDescriptionRenderFn for String {
    fn into_task_description_render_fn(self) -> Box<dyn 'static + Send + Sync + Fn(String, Option<String>) -> String> {
        Box::new(move |_, _id| {
            self.clone()
        })
    }
}

pub fn register_task_description(name: impl Into<String>, render: impl IntoTaskDescriptionRenderFn) {

    let mut map = TASK_DESCR_TABLE.lock().unwrap();
    if map.is_none() {
        *map = Some(HashMap::new());
    }

    let map = map.as_mut().unwrap();

    let name: String = name.into();
    let render = render.into_task_description_render_fn();

    map.insert(name, render);
}

pub fn lookup_task_description(name: &str, id: Option<&str>) -> Option<String> {
    let map = TASK_DESCR_TABLE.lock().unwrap();
    match *map {
        Some(ref map) => {
            match map.get(name) {
                //Some(function) => Some(function(name, id)),
                Some(function) => Some(function(name.to_string(), id.map(|id| id.to_string()))),
                None => None,
            }
        }
        None => None,
    }
}

pub fn registered_task_types() -> Vec<String> {
    let map = TASK_DESCR_TABLE.lock().unwrap();
    match *map {
        Some(ref map) => map.keys().map(|t| t.to_string()).collect(),
        None => Vec::new(),
    }
}

pub fn init_task_descr_table_base() {
    register_task_description("aptupdate", tr!("Update package database"));
    register_task_description("spiceshell", tr!("Shell (Spice)"));
    register_task_description("vncshell", tr!("Shell (VNC)"));
    register_task_description("termproxy", tr!("Console (xterm.js)"));

    register_task_description("diskinit", (tr!("Disk"), tr!("Initialize Disk with GPT")));
    register_task_description("srvstart", (tr!("Service"), tr!("Start")));
    register_task_description("srvstop", (tr!("Setrvice"), tr!("Stop")));
    register_task_description("srvrestart", (tr!("Service"), tr!("Restart")));
    register_task_description("srvreload", (tr!("Service"), tr!("Reload")));
}

pub fn render_upid(upid: &str) -> Html {
    match upid.parse::<UPID>() {
        Err(_) => { html!{upid} }
        Ok(upid) => {
            if let Some(text) = lookup_task_description(upid.worker_type.as_str(), upid.worker_id.as_deref()) {
                html!{text}
            } else {
                let text = match (upid.worker_type.as_str(), upid.worker_id) {
                    (worker_type, Some(id)) => format!("{} {}", worker_type, id),
                    (worker_type, None) =>  format!("{}", worker_type),
                };
                html!{text}
            }
        }
    }
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
            edit: false,
            tfa: true,
            pwchange: false,
            sync: false,
        });
    }
    if ty == "pve" {
        return Some(AuthDomainInfo {
            ty: ty.to_string(),
            //description: tr!("Proxmox VE authentication server"),
            add: false,
            edit: false,
            tfa: true,
            pwchange: true,
            sync: false,
        });
    }
    if ty == "pbs" {
        return Some(AuthDomainInfo {
            ty: ty.to_string(),
            //description: tr!("Proxmox Backup authentication server"),
            add: false,
            edit: false,
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
    if ty == "ldap" {
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