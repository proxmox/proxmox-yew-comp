//! Define product specific things here

use pwt::tr;

/// A trait that defines several aspects of a project for other components in this project.
pub trait ProjectInfo {
    /// Returns the name of the auth cookie.
    fn auth_cookie_name(&self) -> &'static str;

    /// Returns a list prefixes that are used by the project's auth cookie(s).
    fn auth_cookie_prefixes(&self) -> &'static [&'static str];

    /// The non-abbreviated name of the project.
    fn project_text(&self) -> String;

    /// The abbreviated name of the project.
    fn short_name(&self) -> &'static str;

    /// Returns the url where a project's subscription status can be queried.
    fn subscription_url(&self) -> &'static str {
        ""
    }
}

/// Enumerate the different Proxmox products.
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ExistingProduct {
    PVE,
    PMG,
    PBS,
    POM,
    PDM,
}

impl ProjectInfo for ExistingProduct {
    fn auth_cookie_name(&self) -> &'static str {
        match self {
            ExistingProduct::PVE => "PVEAuthCookie",
            ExistingProduct::PMG => "PMGAuthCookie",
            ExistingProduct::PBS => "PBSAuthCookie",
            ExistingProduct::POM => "POMAuthCookie",
            ExistingProduct::PDM => "PDMAuthCookie",
        }
    }

    fn auth_cookie_prefixes(&self) -> &'static [&'static str] {
        match self {
            ExistingProduct::PVE => &["PVE"],
            ExistingProduct::PMG => &["PMG", "PMGQUAR"],
            ExistingProduct::PBS => &["PBS"],
            ExistingProduct::POM => &["POM"],
            ExistingProduct::PDM => &["PDM"],
        }
    }

    fn project_text(&self) -> String {
        match self {
            ExistingProduct::PVE => tr!("Proxmox Virtual Environment"),
            ExistingProduct::PMG => tr!("Proxmox Mail Gateway"),
            ExistingProduct::PBS => tr!("Proxmox Backup Server"),
            ExistingProduct::POM => tr!("Proxmox Offline Mirror"),
            ExistingProduct::PDM => tr!("Proxmox Datacenter Manager"),
        }
    }

    fn short_name(&self) -> &'static str {
        match self {
            ExistingProduct::PVE => "PVE",
            ExistingProduct::PMG => "PMG",
            ExistingProduct::PBS => "PBS",
            ExistingProduct::POM => "POM",
            ExistingProduct::PDM => "PDM",
        }
    }

    fn subscription_url(&self) -> &'static str {
        "/nodes/localhost/subscription"
    }
}
