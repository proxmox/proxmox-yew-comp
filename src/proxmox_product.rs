//! Define product specific things here

/// Enumerate the different Proxmox products. 
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ProxmoxProduct {
    PVE,
    PMG,
    PBS,
}

impl ProxmoxProduct {
    pub fn auth_cookie_name(&self) -> &'static str {
        match self {
            ProxmoxProduct::PVE => "PVEAuthCookie",
            ProxmoxProduct::PMG => "PMGAuthCookie",
            ProxmoxProduct::PBS => "PBSAuthCookie",
        }
    }
}
