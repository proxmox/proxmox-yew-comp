//! API types shared by different Proxmox products

use serde::{Serialize, Deserialize};
use yew::virtual_dom::Key;

use pwt::props::ExtractPrimaryKey;

#[derive(Serialize, Deserialize, Ord, PartialOrd, Eq,  PartialEq, Clone)]
pub struct BasicRealmInfo {
    pub realm: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl ExtractPrimaryKey for BasicRealmInfo {
    fn extract_key(&self) -> yew::virtual_dom::Key {
        Key::from(self.realm.clone())
    }
}

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

impl ExtractPrimaryKey for TaskListItem {
    fn extract_key(&self) -> Key {
        Key::from(self.upid.clone())
    }
}

/// Clasify task status.
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatusClass {
    Ok,
    Warning,
    Error,
}

impl<T: AsRef<str>> From<T> for TaskStatusClass {
    fn from(status: T) -> Self {
        let status = status.as_ref();
        if status == "OK" {
            TaskStatusClass::Ok
        } else if status.starts_with("WARNINGS:") {
            TaskStatusClass::Warning
        } else {
            TaskStatusClass::Error
        }
    }
}

// Copied from pbs-api-types
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
/// Describes a package for which an update is available.
pub struct APTUpdateInfo {
    /// Package name
    pub package: String,
    /// Package title
    pub title: String,
    /// Package architecture
    pub arch: String,
    /// Human readable package description
    pub description: String,
    /// New version to be updated to
    pub version: String,
    /// Old version currently installed
    pub old_version: String,
    /// Package origin
    pub origin: String,
    /// Package priority in human-readable form
    pub priority: String,
    /// Package section
    pub section: String,
    /// Custom extra field for additional package information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_info: Option<String>,
}

/// Certificate information.
#[derive(PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct CertificateInfo {
    /// Certificate file name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,

    /// Certificate subject name.
    pub subject: String,

    /// List of certificate's SubjectAlternativeName entries.
    pub san: Vec<String>,

    /// Certificate issuer name.
    pub issuer: String,

    /// Certificate's notBefore timestamp (UNIX epoch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notbefore: Option<i64>,

    /// Certificate's notAfter timestamp (UNIX epoch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notafter: Option<i64>,

    /// Certificate in PEM format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pem: Option<String>,

    /// Certificate's public key algorithm.
    pub public_key_type: String,

    /// Certificate's public key size if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key_bits: Option<u32>,

    /// The SSL Fingerprint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}
