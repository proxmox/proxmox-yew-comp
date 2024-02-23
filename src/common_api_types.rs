//! API types shared by different Proxmox products

use anyhow::Error;

use proxmox_schema::{api, ApiStringFormat, ApiType, Schema, StringSchema};

use serde::{Deserialize, Serialize};
use yew::virtual_dom::Key;

use pwt::props::ExtractPrimaryKey;

#[derive(Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq, Clone)]
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
    pub filename: String,

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

#[api(
    properties: {
        "alias": {
            optional: true,
        },
        "plugin": {
            optional: true,
        },
    },
    default_key: "domain",
)]
#[derive(Clone, PartialEq, Deserialize, Serialize)]
/// A domain entry for an ACME certificate.
pub struct AcmeDomain {
    /// The domain to certify for.
    pub domain: String,
    /// The domain to use for challenges instead of the default acme challenge domain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// The plugin to use to validate this domain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
}

pub const ACME_DOMAIN_PROPERTY_SCHEMA: Schema =
    StringSchema::new("ACME domain configuration string")
        .format(&ApiStringFormat::PropertyString(&AcmeDomain::API_SCHEMA))
        .schema();

pub fn parse_acme_domain_string(value_str: &str) -> Result<AcmeDomain, Error> {
    let value = AcmeDomain::API_SCHEMA.parse_property_string(value_str)?;
    let value: AcmeDomain = serde_json::from_value(value)?;
    Ok(value)
}

pub fn create_acme_domain_string(config: &AcmeDomain) -> String {
    proxmox_schema::property_string::print::<AcmeDomain>(config).unwrap()
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcmeAccountData {
    pub status: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub contact: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub struct AcmeAccountInfo {
    /// Raw account data.
    pub account: AcmeAccountData,

    /// The ACME directory URL the account was created at.
    pub directory: String,

    /// The account's own URL within the ACME directory.
    pub location: String,

    /// The ToS URL, if the user agreed to one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tos: Option<String>,
}

#[api()]
#[derive(Clone, PartialEq, Deserialize, Serialize)]
/// The ACME configuration.
///
/// Currently only contains the name of the account use.
pub struct AcmeConfig {
    /// Account to use to acquire ACME certificates.
    pub account: String,
}

pub fn parse_acme_config_string(value_str: &str) -> Result<AcmeConfig, Error> {
    let value = AcmeConfig::API_SCHEMA.parse_property_string(value_str)?;
    let value: AcmeConfig = serde_json::from_value(value)?;
    Ok(value)
}

pub fn create_acme_config_string(config: &AcmeConfig) -> String {
    proxmox_schema::property_string::print::<AcmeConfig>(config).unwrap()
}

