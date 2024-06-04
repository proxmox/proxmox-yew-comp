mod dns;
pub use dns::DnsPanel;

mod time;
pub use time::TimePanel;

mod network_view;
pub use network_view::{NetworkView, ProxmoxNetworkView};

mod network_edit;
pub use network_edit::{NetworkEdit, ProxmoxNetworkEdit};

use pwt::tr;
use proxmox_network_api::NetworkInterfaceType;

pub fn format_network_interface_type(interface_type: NetworkInterfaceType) -> String {
    match interface_type {
        NetworkInterfaceType::Loopback => tr!("Lookback"),
        NetworkInterfaceType::Eth => tr!("Network Device"),
        NetworkInterfaceType::Bridge => tr!("Linux Bridge"),
        NetworkInterfaceType::Bond => tr!("Linux Bond"),
        NetworkInterfaceType::Vlan => tr!("Linux VLAN"),
        NetworkInterfaceType::Alias => tr!("Alias"),
        NetworkInterfaceType::Unknown => tr!("Unknown"),
    }
}
