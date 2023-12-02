// TODO: use types from proxmox-apt crate

// Note: copied from proxmox-apt

use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
/// Handles for Proxmox repositories.
pub enum APTRepositoryHandle {
    /// The enterprise repository for production use.
    Enterprise,
    /// The repository that can be used without subscription.
    NoSubscription,
    /// The test repository.
    Test,
    /// Ceph Quincy enterprise repository.
    CephQuincyEnterprise,
    /// Ceph Quincy no-subscription repository.
    CephQuincyNoSubscription,
    /// Ceph Quincy test repository.
    CephQuincyTest,
    // TODO: Add separate enum for ceph releases and use something like
    // `CephTest(CephReleaseCodename),` once the API macro supports it.
    /// Ceph Reef enterprise repository.
    CephReefEnterprise,
    /// Ceph Reef no-subscription repository.
    CephReefNoSubscription,
    /// Ceph Reef test repository.
    CephReefTest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
/// Reference to a standard repository and configuration status.
pub struct APTStandardRepository {
    /// Handle referencing a standard repository.
    pub handle: APTRepositoryHandle,

    /// Configuration status of the associated repository, where `None` means
    /// not configured, and `Some(bool)` indicates enabled or disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<bool>,

    /// Display name of the repository.
    pub name: String,

    /// Description of the repository.
    pub description: String,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum APTRepositoryPackageType {
    /// Debian package
    Deb,
    /// Debian source package
    DebSrc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")] // for consistency
/// Additional options for an APT repository.
/// Used for both single- and mutli-value options.
pub struct APTRepositoryOption {
    /// Option key.
    pub key: String,
    /// Option value(s).
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
/// Describes an APT repository.
pub struct APTRepository {
    /// List of package types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<APTRepositoryPackageType>,

    /// List of repository URIs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "URIs")]
    pub uris: Vec<String>,

    /// List of package distributions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suites: Vec<String>,

    /// List of repository components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<String>,

    /// Additional options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<APTRepositoryOption>,

    /// Associated comment.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,

    /// Format of the defining file.
    pub file_type: APTRepositoryFileType,

    /// Whether the repository is enabled or not.
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Represents an abstract APT repository file.
pub struct APTRepositoryFile {
    /// The path to the file. If None, `contents` must be set directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// The type of the file.
    pub file_type: APTRepositoryFileType,

    /// List of repositories in the file.
    pub repositories: Vec<APTRepository>,
    // /// The file content, if already parsed.
    //pub content: Option<String>,

    // /// Digest of the original contents.
    //pub digest: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Error type for problems with APT repository files.
pub struct APTRepositoryFileError {
    /// The path to the problematic file.
    pub path: String,

    /// The error message.
    pub error: String,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum APTRepositoryFileType {
    /// One-line-style format
    List,
    /// DEB822-style format
    Sources,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Additional information for a repository.
pub struct APTRepositoryInfo {
    /// Path to the defining file.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,

    /// Index of the associated respository within the file (starting from 0).
    pub index: usize,

    /// The property from which the info originates (e.g. "Suites")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    /// Info kind (e.g. "warning")
    pub kind: String,

    /// Info message
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// API return type for /nodes/{node}/apt/repositories
pub struct APTConfiguration {
    pub digest: String,
    pub files: Vec<APTRepositoryFile>,
    pub standard_repos: Vec<APTStandardRepository>,
    pub errors: Vec<APTRepositoryFileError>,
    pub infos: Vec<APTRepositoryInfo>,
}
