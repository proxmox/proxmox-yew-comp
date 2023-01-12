//! Define product specific things here

/// Enumerate the different Proxmox products. 
#[derive(PartialEq, Debug, Clone)]
pub enum ProxmoxProduct {
    PVE,
    PMG,
    PBS,
}

