use crate::percent_encoding::percent_encode_component;
use pwt::prelude::*;

/// Context defining the scope of firewall configuration (Cluster, Node, or Guest level)
#[derive(Clone, PartialEq)]
pub enum FirewallContext {
    Cluster {
        remote: AttrValue,
    },
    Node {
        remote: AttrValue,
        node: AttrValue,
    },
    Guest {
        remote: AttrValue,
        node: AttrValue,
        vmid: u64,
        vmtype: AttrValue,
    },
}

impl FirewallContext {
    pub fn cluster(remote: impl Into<AttrValue>) -> Self {
        Self::Cluster {
            remote: remote.into(),
        }
    }

    pub fn node(remote: impl Into<AttrValue>, node: impl Into<AttrValue>) -> Self {
        Self::Node {
            remote: remote.into(),
            node: node.into(),
        }
    }

    pub fn guest(
        remote: impl Into<AttrValue>,
        node: impl Into<AttrValue>,
        vmid: u64,
        vmtype: impl Into<AttrValue>,
    ) -> Self {
        Self::Guest {
            remote: remote.into(),
            node: node.into(),
            vmid,
            vmtype: vmtype.into(),
        }
    }

    pub fn rules_url(&self) -> String {
        match self {
            Self::Cluster { remote } => {
                format!(
                    "/pve/remotes/{}/firewall/rules",
                    percent_encode_component(remote)
                )
            }
            Self::Node { remote, node } => {
                format!(
                    "/pve/remotes/{}/nodes/{}/firewall/rules",
                    percent_encode_component(remote),
                    percent_encode_component(node)
                )
            }
            Self::Guest {
                remote,
                node,
                vmid,
                vmtype,
            } => {
                let mut url = format!(
                    "/pve/remotes/{}/{}/{}/firewall/rules",
                    percent_encode_component(remote),
                    percent_encode_component(vmtype),
                    vmid
                );
                if !node.is_empty() {
                    url = format!("{}?node={}", url, percent_encode_component(node));
                }
                url
            }
        }
    }

    pub fn options_url(&self) -> String {
        match self {
            Self::Cluster { remote } => {
                format!(
                    "/pve/remotes/{}/firewall/options",
                    percent_encode_component(remote)
                )
            }
            Self::Node { remote, node } => {
                format!(
                    "/pve/remotes/{}/nodes/{}/firewall/options",
                    percent_encode_component(remote),
                    percent_encode_component(node)
                )
            }
            Self::Guest {
                remote,
                node,
                vmid,
                vmtype,
            } => {
                let mut url = format!(
                    "/pve/remotes/{}/{}/{}/firewall/options",
                    percent_encode_component(remote),
                    percent_encode_component(vmtype),
                    vmid
                );
                if !node.is_empty() {
                    url = format!("{}?node={}", url, percent_encode_component(node));
                }
                url
            }
        }
    }

    pub fn title(&self, prefix: &str) -> String {
        match self {
            Self::Cluster { remote } => {
                if !remote.is_empty() {
                    format!("{}: {}", prefix, remote)
                } else {
                    prefix.to_string()
                }
            }
            Self::Node { remote, node } => {
                format!("{}: {}/{}", prefix, remote, node)
            }
            Self::Guest {
                remote,
                vmtype,
                vmid,
                ..
            } => {
                format!("{}: {}/{} {}", prefix, remote, vmtype.to_uppercase(), vmid)
            }
        }
    }
}
