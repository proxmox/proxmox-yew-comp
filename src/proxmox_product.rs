//! Define product specific things here

use pwt::tr;

/// Enumerate the different Proxmox products.
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ProxmoxProduct {
    PVE,
    PMG,
    PBS,
    POM,
}

impl ProxmoxProduct {
    pub fn auth_cookie_name(&self) -> &'static str {
        match self {
            ProxmoxProduct::PVE => "PVEAuthCookie",
            ProxmoxProduct::PMG => "PMGAuthCookie",
            ProxmoxProduct::PBS => "PBSAuthCookie",
            ProxmoxProduct::POM => "POMAuthCookie",
        }
    }

    pub fn auth_cookie_prefixes(&self) -> &'static[&'static str] {
        match self {
            ProxmoxProduct::PVE => &["PVE"],
            ProxmoxProduct::PMG => &["PMG", "PMGQUAR"],
            ProxmoxProduct::PBS => &["PBS"],
            ProxmoxProduct::POM => &["POM"],
        }
    }

    pub fn product_text(&self) -> String {
        match self {
            ProxmoxProduct::PVE => tr!("Proxmox Virtual Environment"),
            ProxmoxProduct::PMG => tr!("Proxmox Mail Gateway"),
            ProxmoxProduct::PBS => tr!("Proxmox Backup Server"),
            ProxmoxProduct::POM => tr!("Proxmox Offline Mirror"),
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            ProxmoxProduct::PVE => "PVE",
            ProxmoxProduct::PMG => "PMG",
            ProxmoxProduct::PBS => "PBS",
            ProxmoxProduct::POM => "POM",
        }
    }
}
