use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Mutex;

use serde_json::Value;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew::NodeRef;

use crate::common_api_types::ProxmoxUpid;

use pwt::tr;

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

// todo: we want to use Fn(&str, Option<&str>),
#[allow(clippy::type_complexity)]
static TASK_DESCR_TABLE: Mutex<
    Option<HashMap<String, Box<dyn Send + Sync + Fn(String, Option<String>) -> String>>>,
> = Mutex::new(None);

pub trait IntoTaskDescriptionRenderFn {
    fn into_task_description_render_fn(
        self,
    ) -> Box<dyn Send + Sync + Fn(String, Option<String>) -> String>;
}

impl<F: 'static + Send + Sync + Fn(String, Option<String>) -> String> IntoTaskDescriptionRenderFn
    for F
{
    fn into_task_description_render_fn(
        self,
    ) -> Box<dyn Send + Sync + Fn(String, Option<String>) -> String> {
        Box::new(self)
    }
}

impl<A: Display, B: Display> IntoTaskDescriptionRenderFn for (A, B) {
    fn into_task_description_render_fn(
        self,
    ) -> Box<dyn 'static + Send + Sync + Fn(String, Option<String>) -> String> {
        let task_type = self.0.to_string();
        let action = self.1.to_string();
        Box::new(move |_, id| {
            format!(
                "{} {} {}",
                task_type,
                id.as_deref().unwrap_or("unknown"),
                action
            )
        })
    }
}

impl IntoTaskDescriptionRenderFn for String {
    fn into_task_description_render_fn(
        self,
    ) -> Box<dyn 'static + Send + Sync + Fn(String, Option<String>) -> String> {
        Box::new(move |_, _id| self.clone())
    }
}

pub fn register_task_description(
    name: impl Into<String>,
    render: impl IntoTaskDescriptionRenderFn,
) {
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
        Some(ref map) => map
            .get(name)
            .map(|function| function(name.to_string(), id.map(|id| id.to_string()))),
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
    register_task_description("srvstop", (tr!("Service"), tr!("Stop")));
    register_task_description("srvrestart", (tr!("Service"), tr!("Restart")));
    register_task_description("srvreload", (tr!("Service"), tr!("Reload")));
}

/// Uses information from the given `UPID` to render the task description with [`format_task_description`]
pub fn format_upid(upid: &str) -> String {
    match upid.parse::<ProxmoxUpid>() {
        Err(_) => upid.to_string(),
        Ok(upid) => format_task_description(&upid.worker_type, upid.worker_id.as_deref()),
    }
}

/// Formats the given worker type and id to a Human readable task description
pub fn format_task_description(worker_type: &str, worker_id: Option<&str>) -> String {
    if let Some(text) = lookup_task_description(worker_type, worker_id) {
        text
    } else {
        match worker_id {
            Some(id) => format!("{} {}", worker_type, id),
            None => worker_type.to_string(),
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

    if matches!(ty, "pve" | "pbs" | "pdm") {
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

pub fn copy_to_clipboard(node_ref: &NodeRef) {
    if let Some(el) = node_ref.cast::<web_sys::HtmlInputElement>() {
        let window = gloo_utils::window();
        let document = gloo_utils::document();

        let selection = window.get_selection().unwrap().unwrap();
        let _ = selection.remove_all_ranges();

        let range = document.create_range().unwrap();
        let _ = range.select_node_contents(&el);

        let _ = selection.add_range(&range);

        let document = document.dyn_into::<web_sys::HtmlDocument>().unwrap();
        let _ = document.exec_command("copy");
    }
}

/// Set the browser window.location.href
pub fn set_location_href(href: &str) {
    let window = gloo_utils::window();
    let location = window.location();
    let _ = location.set_href(href);
}

/// Register PVE task descriptions
pub fn register_pve_tasks() {
    register_task_description("qmstart", ("VM", tr!("Start")));
    register_task_description("acmedeactivate", ("ACME Account", tr!("Deactivate")));
    register_task_description("acmenewcert", ("SRV", tr!("Order Certificate")));
    register_task_description("acmerefresh", ("ACME Account", tr!("Refresh")));
    register_task_description("acmeregister", ("ACME Account", tr!("Register")));
    register_task_description("acmerenew", ("SRV", tr!("Renew Certificate")));
    register_task_description("acmerevoke", ("SRV", tr!("Revoke Certificate")));
    register_task_description("acmeupdate", ("ACME Account", tr!("Update")));
    register_task_description("auth-realm-sync", (tr!("Realm"), tr!("Sync")));
    register_task_description("auth-realm-sync-test", (tr!("Realm"), tr!("Sync Preview")));
    register_task_description("cephcreatemds", ("Ceph Metadata Server", tr!("Create")));
    register_task_description("cephcreatemgr", ("Ceph Manager", tr!("Create")));
    register_task_description("cephcreatemon", ("Ceph Monitor", tr!("Create")));
    register_task_description("cephcreateosd", ("Ceph OSD", tr!("Create")));
    register_task_description("cephcreatepool", ("Ceph Pool", tr!("Create")));
    register_task_description("cephdestroymds", ("Ceph Metadata Server", tr!("Destroy")));
    register_task_description("cephdestroymgr", ("Ceph Manager", tr!("Destroy")));
    register_task_description("cephdestroymon", ("Ceph Monitor", tr!("Destroy")));
    register_task_description("cephdestroyosd", ("Ceph OSD", tr!("Destroy")));
    register_task_description("cephdestroypool", ("Ceph Pool", tr!("Destroy")));
    register_task_description("cephdestroyfs", ("CephFS", tr!("Destroy")));
    register_task_description("cephfscreate", ("CephFS", tr!("Create")));
    register_task_description("cephsetpool", ("Ceph Pool", tr!("Edit")));
    register_task_description("cephsetflags", tr!("Change global Ceph flags"));
    register_task_description("clustercreate", tr!("Create Cluster"));
    register_task_description("clusterjoin", tr!("Join Cluster"));
    register_task_description("dircreate", (tr!("Directory Storage"), tr!("Create")));
    register_task_description("dirremove", (tr!("Directory"), tr!("Remove")));
    register_task_description("download", (tr!("File"), tr!("Download")));
    register_task_description("hamigrate", ("HA", tr!("Migrate")));
    register_task_description("hashutdown", ("HA", tr!("Shutdown")));
    register_task_description("hastart", ("HA", tr!("Start")));
    register_task_description("hastop", ("HA", tr!("Stop")));
    register_task_description("imgcopy", tr!("Copy data"));
    register_task_description("imgdel", tr!("Erase data"));
    register_task_description("lvmcreate", (tr!("LVM Storage"), tr!("Create")));
    register_task_description("lvmremove", ("Volume Group", tr!("Remove")));
    register_task_description("lvmthincreate", (tr!("LVM-Thin Storage"), tr!("Create")));
    register_task_description("lvmthinremove", ("Thinpool", tr!("Remove")));
    register_task_description("migrateall", tr!("Bulk migrate VMs and Containers"));
    register_task_description("move_volume", ("CT", tr!("Move Volume")));
    register_task_description("pbs-download", ("VM/CT", tr!("File Restore Download")));
    register_task_description("pull_file", ("CT", tr!("Pull file")));
    register_task_description("push_file", ("CT", tr!("Push file")));
    register_task_description("qmclone", ("VM", tr!("Clone")));
    register_task_description("qmconfig", ("VM", tr!("Configure")));
    register_task_description("qmcreate", ("VM", tr!("Create")));
    register_task_description("qmdelsnapshot", ("VM", tr!("Delete Snapshot")));
    register_task_description("qmdestroy", ("VM", tr!("Destroy")));
    register_task_description("qmigrate", ("VM", tr!("Migrate")));
    register_task_description("qmmove", ("VM", tr!("Move disk")));
    register_task_description("qmpause", ("VM", tr!("Pause")));
    register_task_description("qmreboot", ("VM", tr!("Reboot")));
    register_task_description("qmreset", ("VM", tr!("Reset")));
    register_task_description("qmrestore", ("VM", tr!("Restore")));
    register_task_description("qmresume", ("VM", tr!("Resume")));
    register_task_description("qmrollback", ("VM", tr!("Rollback")));
    register_task_description("qmshutdown", ("VM", tr!("Shutdown")));
    register_task_description("qmsnapshot", ("VM", tr!("Snapshot")));
    register_task_description("qmstart", ("VM", tr!("Start")));
    register_task_description("qmstop", ("VM", tr!("Stop")));
    register_task_description("qmsuspend", ("VM", tr!("Hibernate")));
    register_task_description("qmtemplate", ("VM", tr!("Convert to template")));
    register_task_description("resize", ("VM/CT", tr!("Resize")));
    register_task_description("spiceproxy", ("VM/CT", tr!("Console") + " (Spice)"));
    register_task_description("spiceshell", tr!("Shell") + " (Spice)");
    register_task_description("startall", tr!("Bulk start VMs and Containers"));
    register_task_description("stopall", tr!("Bulk shutdown VMs and Containers"));
    register_task_description("suspendall", tr!("Suspend all VMs"));
    register_task_description("unknownimgdel", tr!("Destroy image from unknown guest"));
    register_task_description("wipedisk", ("Device", tr!("Wipe Disk")));
    register_task_description("vncproxy", ("VM/CT", tr!("Console")));
    register_task_description("vncshell", tr!("Shell"));
    register_task_description("vzclone", ("CT", tr!("Clone")));
    register_task_description("vzcreate", ("CT", tr!("Create")));
    register_task_description("vzdelsnapshot", ("CT", tr!("Delete Snapshot")));
    register_task_description("vzdestroy", ("CT", tr!("Destroy")));
    register_task_description("vzdump", |_ty, id| match id {
        Some(id) => format!("VM/CT {id} - {}", tr!("Backup")),
        None => tr!("Backup Job"),
    });
    register_task_description("vzmigrate", ("CT", tr!("Migrate")));
    register_task_description("vzmount", ("CT", tr!("Mount")));
    register_task_description("vzreboot", ("CT", tr!("Reboot")));
    register_task_description("vzrestore", ("CT", tr!("Restore")));
    register_task_description("vzresume", ("CT", tr!("Resume")));
    register_task_description("vzrollback", ("CT", tr!("Rollback")));
    register_task_description("vzshutdown", ("CT", tr!("Shutdown")));
    register_task_description("vzsnapshot", ("CT", tr!("Snapshot")));
    register_task_description("vzstart", ("CT", tr!("Start")));
    register_task_description("vzstop", ("CT", tr!("Stop")));
    register_task_description("vzsuspend", ("CT", tr!("Suspend")));
    register_task_description("vztemplate", ("CT", tr!("Convert to template")));
    register_task_description("vzumount", ("CT", tr!("Unmount")));
    register_task_description("zfscreate", (tr!("ZFS Storage"), tr!("Create")));
    register_task_description("zfsremove", ("ZFS Pool", tr!("Remove")));
}
