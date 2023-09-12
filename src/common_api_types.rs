//! API types shared by different Proxmox products

use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct RoleInfo {
    pub roleid: String,
    pub privs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

// copied from pbs_api_types::TaskListItem;
#[derive(Serialize, Deserialize, Clone, PartialEq)]
/// Task properties.
pub struct TaskListItem {
    pub upid: String,
    /// The node name where the task is running on.
    pub node: String,
    /// The Unix PID
    pub pid: i64,
    /// The task start time (Epoch)
    pub pstart: u64,
    /// The task start time (Epoch)
    pub starttime: i64,
    /// Worker type (arbitrary ASCII string)
    pub worker_type: String,
    /// Worker ID (arbitrary ASCII string)
    pub worker_id: Option<String>,
    /// The authenticated entity who started the task
    pub user: String,
    /// The task end time (Epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endtime: Option<i64>,
    /// Task end status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}