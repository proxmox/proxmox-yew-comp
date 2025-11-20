#[cfg(feature = "dns")]
mod dns;
#[cfg(feature = "dns")]
pub use dns::DnsPanel;

mod time;
pub use time::TimePanel;

pub mod pve;

#[cfg(feature = "network")]
mod network_view;
#[cfg(feature = "network")]
pub use network_view::{NetworkView, ProxmoxNetworkView};

#[cfg(feature = "network")]
mod network_edit;
#[cfg(feature = "network")]
pub use network_edit::{NetworkEdit, ProxmoxNetworkEdit};

#[cfg(feature = "network")]
use proxmox_network_api::NetworkInterfaceType;
#[cfg(feature = "network")]
use pwt::tr;

#[cfg(feature = "network")]
pub fn format_network_interface_type(interface_type: NetworkInterfaceType) -> String {
    match interface_type {
        NetworkInterfaceType::Loopback => tr!("Loopback"),
        NetworkInterfaceType::Eth => tr!("Network Device"),
        NetworkInterfaceType::Bridge => tr!("Linux Bridge"),
        NetworkInterfaceType::Bond => tr!("Linux Bond"),
        NetworkInterfaceType::Vlan => tr!("Linux VLAN"),
        NetworkInterfaceType::Alias => tr!("Alias"),
        NetworkInterfaceType::Unknown => tr!("Unknown"),
    }
}

use yew::AttrValue;

use crate::form::pve::PveGuestType;
use crate::percent_encoding::percent_encode_component;

fn guest_base_url(
    vmid: u32,
    node: &AttrValue,
    remote: &Option<AttrValue>,
    guest_type: PveGuestType,
) -> String {
    let guest_type = match guest_type {
        PveGuestType::Lxc => "lxc",
        PveGuestType::Qemu => "qemu",
    };

    if let Some(remote) = remote {
        format!(
            "/pve/remotes/{}/{}/{}",
            percent_encode_component(remote),
            guest_type,
            vmid
        )
    } else {
        format!(
            "/nodes/{}/{}/{}",
            percent_encode_component(node),
            guest_type,
            vmid
        )
    }
}

pub fn guest_config_url(
    vmid: u32,
    node: &AttrValue,
    remote: &Option<AttrValue>,
    guest_type: PveGuestType,
) -> String {
    let base_url = guest_base_url(vmid, node, remote, guest_type);
    if remote.is_some() {
        format!("{base_url}/config?state=pending")
    } else {
        format!("{base_url}/config")
    }
}

pub fn guest_pending_url(
    vmid: u32,
    node: &AttrValue,
    remote: &Option<AttrValue>,
    guest_type: PveGuestType,
) -> String {
    let base_url = guest_base_url(vmid, node, remote, guest_type);
    format!("{base_url}/pending")
}

pub fn guest_resize_disk_url(
    vmid: u32,
    node: &AttrValue,
    remote: &Option<AttrValue>,
    guest_type: PveGuestType,
) -> String {
    let base_url = guest_base_url(vmid, node, remote, guest_type);
    format!("{base_url}/resize")
}

pub fn guest_move_volume_url(
    vmid: u32,
    node: &AttrValue,
    remote: &Option<AttrValue>,
    guest_type: PveGuestType,
) -> String {
    let name = if remote.is_some() {
        "move-volume"
    } else {
        "move_volume"
    };
    let base_url = guest_base_url(vmid, node, remote, guest_type);
    format!("{base_url}/{name}")
}

pub fn guest_move_disk_url(
    vmid: u32,
    node: &AttrValue,
    remote: &Option<AttrValue>,
    guest_type: PveGuestType,
) -> String {
    let name = if remote.is_some() {
        "move-disk"
    } else {
        "move_disk"
    };
    let base_url = guest_base_url(vmid, node, remote, guest_type);
    format!("{base_url}/{name}")
}
