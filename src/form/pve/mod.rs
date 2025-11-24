use std::collections::HashSet;

mod boot_device_list;
pub use boot_device_list::{BootDeviceList, PveBootDeviceList};

mod qemu_ostype_selector;
pub use qemu_ostype_selector::{format_qemu_ostype, QemuOstypeSelector};

mod qemu_cache_type_selector;
pub use qemu_cache_type_selector::QemuCacheTypeSelector;

mod qemu_controller_selector;
pub use qemu_controller_selector::{parse_qemu_controller_name, QemuControllerSelector};

mod qemu_cpu_flags_list;
pub use qemu_cpu_flags_list::QemuCpuFlags;

mod qemu_cpu_model_selector;
pub use qemu_cpu_model_selector::QemuCpuModelSelector;

mod qemu_disk_format_selector;
pub use qemu_disk_format_selector::QemuDiskFormatSelector;

mod qemu_disk_size_format_selector;
pub use qemu_disk_size_format_selector::QemuDiskSizeFormatSelector;

mod qemu_display_type_selector;
pub use qemu_display_type_selector::{format_qemu_display_type, QemuDisplayTypeSelector};

mod pve_guest_selector;
pub use pve_guest_selector::PveGuestSelector;

mod qemu_machine_version_selector;
pub use qemu_machine_version_selector::QemuMachineVersionSelector;

mod pve_network_selector;
pub use pve_network_selector::PveNetworkSelector;

mod pve_storage_content_selector;
pub use pve_storage_content_selector::PveStorageContentSelector;

mod pve_vlan_field;
pub use pve_vlan_field::PveVlanField;

mod hotplug_feature_selector;
pub use hotplug_feature_selector::{
    format_hotplug_feature, normalize_hotplug_value, HotplugFeatureSelector,
    PveHotplugFeatureSelector,
};

mod lxc_mount_options_selector;
pub use lxc_mount_options_selector::LxcMountOptionsSelector;

mod lxc_property;
pub use lxc_property::{
    extract_used_mount_points, first_unused_mount_point, lxc_architecture_property,
    lxc_console_mode_property, lxc_console_property, lxc_cores_property, lxc_features_property,
    lxc_hookscript_property, lxc_hostname_property, lxc_memory_property, lxc_mount_point_property,
    lxc_nameserver_property, lxc_ostype_property, lxc_rootfs_property, lxc_searchdomain_property,
    lxc_swap_property, lxc_tty_count_property, lxc_unpriviledged_property,
    lxc_unused_volume_property,
};

mod qemu_property;
pub use qemu_property::{
    extract_used_devices, qemu_acpi_property, qemu_agent_property, qemu_amd_sev_property,
    qemu_bios_property, qemu_boot_property, qemu_cdrom_property, qemu_cpu_flags_property,
    qemu_disk_property, qemu_display_property, qemu_efidisk_property, qemu_freeze_property,
    qemu_hotplug_property, qemu_kernel_scheduler_property, qemu_kvm_property,
    qemu_localtime_property, qemu_machine_property, qemu_memory_property, qemu_name_property,
    qemu_network_mtu_property, qemu_network_property, qemu_onboot_property, qemu_ostype_property,
    qemu_protection_property, qemu_scsihw_property, qemu_smbios_property,
    qemu_sockets_cores_property, qemu_spice_enhancement_property, qemu_startdate_property,
    qemu_startup_property, qemu_tablet_property, qemu_tpmstate_property, qemu_vmstate_property,
    qemu_vmstatestorage_property,
};

mod pve_storage_selector;
pub use pve_storage_selector::PveStorageSelector;
use serde_json::Value;

#[derive(PartialEq, Clone, Copy)]
pub enum PveGuestType {
    Qemu,
    Lxc,
}

fn parse_unused_key(key: &str) -> Option<usize> {
    if key.starts_with("unused") {
        if let Ok(id) = key[6..].parse::<usize>() {
            return Some(id);
        }
    }
    None
}

pub fn extract_unused_keys(record: &Value) -> HashSet<String> {
    let mut list = HashSet::new();
    if let Some(map) = record.as_object() {
        for key in map.keys() {
            if parse_unused_key(key).is_some() {
                list.insert(key.to_string());
            }
        }
    }
    list
}
