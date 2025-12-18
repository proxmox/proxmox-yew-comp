use std::{collections::HashMap, fmt::Display, sync::Mutex};

use pwt::tr;

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
    // TRANSLATORS: Spice is a proper name, see https://www.spice-space.org
    register_task_description("spiceshell", tr!("Shell (Spice)"));
    register_task_description("vncshell", tr!("Shell (VNC)"));
    register_task_description("termproxy", tr!("Console (xterm.js)"));

    register_task_description("diskinit", (tr!("Disk"), tr!("Initialize Disk with GPT")));
    register_task_description("srvstart", (tr!("Service"), tr!("Start")));
    register_task_description("srvstop", (tr!("Service"), tr!("Stop")));
    register_task_description("srvrestart", (tr!("Service"), tr!("Restart")));
    register_task_description("srvreload", (tr!("Service"), tr!("Reload")));
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
