mod dns;
pub use dns::DnsPanel;

mod time;
pub use time::TimePanel;

mod network_view;
pub use network_view::{NetworkView, ProxmoxNetworkView};

mod network_edit;
pub use network_edit::{NetworkEdit, ProxmoxNetworkEdit};
