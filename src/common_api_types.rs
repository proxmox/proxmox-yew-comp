//! API types shared by different Proxmox products

use anyhow::{bail, Error};
use serde_json::Value;

use proxmox_schema::{api, const_regex, ApiStringFormat, ApiType, Schema, StringSchema};

use serde::{Deserialize, Serialize};
use yew::virtual_dom::Key;

use pwt::props::ExtractPrimaryKey;

#[derive(Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq, Clone)]
pub struct BasicRealmInfo {
    pub realm: String,
    #[serde(rename = "type")]
    pub ty: String,
    /// True if it is the default realm
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl ExtractPrimaryKey for BasicRealmInfo {
    fn extract_key(&self) -> yew::virtual_dom::Key {
        Key::from(self.realm.clone())
    }
}

/// Upid covering different products (PVE, PBS and PDM)
///
/// Different products use different UPID formats. This type can parse
/// all of them and provides enough information to nicely display task list/info.
#[derive(Debug, Clone, PartialEq)]
pub struct ProxmoxUpid {
    /// The task start time (Epoch)
    pub starttime: i64,
    /// Worker type (arbitrary ASCII string)
    pub worker_type: String,
    /// Worker ID (arbitrary ASCII string)
    pub worker_id: Option<String>,
    /// The authenticated entity who started the task
    pub auth_id: String,
    /// The node name.
    pub node: String,
    /// Remote name for PDM RemoteUpid
    pub remote: Option<String>,
}

impl std::str::FromStr for ProxmoxUpid {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (remote, upid_part) = match s.split_once('!') {
            Some((p1, p2)) => (Some(p1.to_string()), p2),
            None => (None, s),
        };

        let colon_count = upid_part.chars().filter(|c| *c == ':').count();

        if colon_count == 9 {
            // assume PBS
            let upid = proxmox_schema::upid::UPID::from_str(upid_part)?;
            Ok(Self {
                starttime: upid.starttime,
                worker_id: upid.worker_id,
                worker_type: upid.worker_type,
                auth_id: upid.auth_id,
                node: upid.node,
                remote,
            })
        } else if colon_count == 8 {
            // assume PVE
            let upid = PveUpid::from_str(upid_part)?;
            Ok(Self {
                starttime: upid.starttime,
                worker_id: upid.worker_id,
                worker_type: upid.worker_type,
                auth_id: upid.auth_id,
                node: upid.node,
                remote,
            })
        } else {
            bail!("unable to parse UPID '{}'", s);
        }
    }
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_version: Option<String>,
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
    pub created_at: Option<String>,
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

// Copied from pve-api-type
/// A PVE Upid, contrary to a PBS Upid, contains no 'task-id' number.
pub struct PveUpid {
    /// The Unix PID
    pub pid: i32, // really libc::pid_t, but we don't want this as a dependency for proxmox-schema
    /// The Unix process start time from `/proc/pid/stat`
    pub pstart: u64,
    /// The task start time (Epoch)
    pub starttime: i64,
    /// Worker type (arbitrary ASCII string)
    pub worker_type: String,
    /// Worker ID (arbitrary ASCII string)
    pub worker_id: Option<String>,
    /// The authenticated entity who started the task
    pub auth_id: String,
    /// The node name.
    pub node: String,
}

const_regex! {
    pub PVE_UPID_REGEX = concat!(
        r"^UPID:(?P<node>[a-zA-Z0-9]([a-zA-Z0-9\-]*[a-zA-Z0-9])?):(?P<pid>[0-9A-Fa-f]{8}):",
        r"(?P<pstart>[0-9A-Fa-f]{8,9}):(?P<starttime>[0-9A-Fa-f]{8}):",
        r"(?P<wtype>[^:\s]+):(?P<wid>[^:\s]*):(?P<authid>[^:\s]+):$"
    );
}

impl std::str::FromStr for PveUpid {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(cap) = PVE_UPID_REGEX.captures(s) {
            let worker_id = if cap["wid"].is_empty() {
                None
            } else {
                let wid = unescape_id(&cap["wid"])?;
                Some(wid)
            };

            Ok(PveUpid {
                pid: i32::from_str_radix(&cap["pid"], 16).unwrap(),
                pstart: u64::from_str_radix(&cap["pstart"], 16).unwrap(),
                starttime: i64::from_str_radix(&cap["starttime"], 16).unwrap(),
                worker_type: cap["wtype"].to_string(),
                worker_id,
                auth_id: cap["authid"].to_string(),
                node: cap["node"].to_string(),
            })
        } else {
            bail!("unable to parse UPID '{}'", s);
        }
    }
}

// Copied from pve-api-type for use in PveUpid
fn hex_digit(d: u8) -> Result<u8, Error> {
    match d {
        b'0'..=b'9' => Ok(d - b'0'),
        b'A'..=b'F' => Ok(d - b'A' + 10),
        b'a'..=b'f' => Ok(d - b'a' + 10),
        _ => bail!("got invalid hex digit"),
    }
}

// Copied from pve-api-type for use in PveUpid
// FIXME: This is in `proxmox_schema::upid` and should be `pub` there instead.
/// systemd-unit compatible escaping
fn unescape_id(text: &str) -> Result<String, Error> {
    let mut i = text.as_bytes();

    let mut data: Vec<u8> = Vec::new();

    loop {
        if i.is_empty() {
            break;
        }
        let next = i[0];
        if next == b'\\' {
            if i.len() < 4 || i[1] != b'x' {
                bail!("error in escape sequence");
            }
            let h1 = hex_digit(i[2])?;
            let h0 = hex_digit(i[3])?;
            data.push(h1 << 4 | h0);
            i = &i[4..]
        } else if next == b'-' {
            data.push(b'/');
            i = &i[1..]
        } else {
            data.push(next);
            i = &i[1..]
        }
    }

    let text = String::from_utf8(data)?;

    Ok(text)
}

// Todo: should be defined in pve-api-types
/// Get the virtual machine configuration with both current and pending values.
///
/// (`GET /api2/json/nodes/{node}/qemu/{vmid}/pending) -> Vec<QemuPendingConfigValue>`
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct QemuPendingConfigValue {
    /// Configuration option name.
    pub key: String,
    /// Indicates a pending delete request if present and not 0. The value 2 indicates a force-delete request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete: Option<u8>,
    /// Current value.
    pub value: Option<Value>,
    /// Pending value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<Value>,
}
