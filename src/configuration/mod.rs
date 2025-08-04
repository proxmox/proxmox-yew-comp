#[cfg(feature = "dns")]
mod dns;
#[cfg(feature = "dns")]
pub use dns::DnsPanel;

mod time;
pub use time::TimePanel;

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
