use proxmox_human_byte::HumanByte;
use proxmox_node_status::BootMode;
use pwt::{prelude::*, widget::Container};

use crate::{MeterLabel, StatusRow};

/// Type that holds either a PVE NodeStatus or a PBS NodeStatus
pub enum NodeStatus<'a> {
    Pve(&'a pve_api_types::NodeStatus),
    Pbs(&'a pbs_api_types::NodeStatus),
    Common(&'a proxmox_node_status::NodeStatus),
}

impl<'a> From<&'a pve_api_types::NodeStatus> for NodeStatus<'a> {
    fn from(value: &'a pve_api_types::NodeStatus) -> Self {
        NodeStatus::Pve(value)
    }
}

impl<'a> From<&'a pbs_api_types::NodeStatus> for NodeStatus<'a> {
    fn from(value: &'a pbs_api_types::NodeStatus) -> Self {
        NodeStatus::Pbs(value)
    }
}

/// Renders the NodeInfo panel content
// TODO: add repository status
// NOTE: if we need internal state or the tree get's too big, we should convert this
// into a proper component
pub fn node_info(data: Option<NodeStatus>) -> Container {
    let (cpu, cpus_total) = match data {
        Some(NodeStatus::Pve(node_status)) => (node_status.cpu, node_status.cpuinfo.cpus as u64),
        Some(NodeStatus::Pbs(node_status)) => (node_status.cpu, node_status.cpuinfo.cpus as u64),
        Some(NodeStatus::Common(node_status)) => (node_status.cpu, node_status.cpuinfo.cpus as u64),
        None => (0.0, 1),
    };

    let wait = match data {
        Some(NodeStatus::Pve(node_status)) => node_status
            .additional_properties
            .get("wait")
            .and_then(|wait| wait.as_f64())
            .unwrap_or_default(),
        Some(NodeStatus::Pbs(node_status)) => node_status.wait,
        Some(NodeStatus::Common(node_status)) => node_status.wait,
        None => 0.0,
    };

    let (memory_used, memory_total) = match data {
        Some(NodeStatus::Pve(node_status)) => (
            node_status.memory.used as u64,
            node_status.memory.total as u64,
        ),
        Some(NodeStatus::Pbs(node_status)) => (node_status.memory.used, node_status.memory.total),
        Some(NodeStatus::Common(node_status)) => {
            (node_status.memory.used, node_status.memory.total)
        }
        None => (0, 1),
    };

    let loadavg = match data {
        Some(NodeStatus::Pve(node_status)) => node_status.loadavg.join(" "),
        Some(NodeStatus::Pbs(node_status)) => format!(
            "{:.2} {:.2} {:.2}",
            node_status.loadavg[0], node_status.loadavg[1], node_status.loadavg[2]
        ),
        Some(NodeStatus::Common(node_status)) => format!(
            "{:.2} {:.2} {:.2}",
            node_status.loadavg[0], node_status.loadavg[1], node_status.loadavg[2]
        ),
        None => tr!("N/A"),
    };

    let (root_used, root_total) = match data {
        Some(NodeStatus::Pve(node_status)) => (
            node_status.rootfs.used as u64,
            node_status.rootfs.total as u64,
        ),
        Some(NodeStatus::Pbs(node_status)) => (node_status.root.used, node_status.root.total),
        Some(NodeStatus::Common(node_status)) => (node_status.root.used, node_status.root.total),
        None => (0, 1),
    };

    let (swap_used, swap_total) = match data {
        Some(NodeStatus::Pve(node_status)) => {
            if let Some(swap) = node_status
                .additional_properties
                .get("swap")
                .and_then(|swap| swap.as_object())
            {
                let used = swap
                    .get("used")
                    .and_then(|used| used.as_u64())
                    .unwrap_or_default();
                let total = swap
                    .get("total")
                    .and_then(|used| used.as_u64())
                    .unwrap_or(0);
                (used, total)
            } else {
                (0, 0)
            }
        }
        Some(NodeStatus::Pbs(node_status)) => (node_status.swap.used, node_status.swap.total),
        Some(NodeStatus::Common(node_status)) => (node_status.swap.used, node_status.swap.total),
        None => (0, 1),
    };

    let (model, sockets) = match data {
        Some(NodeStatus::Pve(node_status)) => (
            node_status.cpuinfo.model.clone(),
            node_status.cpuinfo.sockets as u64,
        ),
        Some(NodeStatus::Pbs(node_status)) => (
            node_status.cpuinfo.model.clone(),
            node_status.cpuinfo.sockets as u64,
        ),
        Some(NodeStatus::Common(node_status)) => (
            node_status.cpuinfo.model.clone(),
            node_status.cpuinfo.sockets as u64,
        ),
        None => (String::new(), 1),
    };

    let version = match data {
        Some(NodeStatus::Pve(node_status)) => Some(node_status.pveversion.clone()),
        _ => None,
    };

    let (k_sysname, k_release, k_version) = match data {
        Some(NodeStatus::Pve(node_status)) => (
            node_status.current_kernel.sysname.clone(),
            node_status.current_kernel.release.clone(),
            node_status.current_kernel.version.clone(),
        ),
        Some(NodeStatus::Pbs(node_status)) => (
            node_status.current_kernel.sysname.clone(),
            node_status.current_kernel.release.clone(),
            node_status.current_kernel.version.clone(),
        ),
        Some(NodeStatus::Common(node_status)) => (
            node_status.current_kernel.sysname.clone(),
            node_status.current_kernel.release.clone(),
            node_status.current_kernel.version.clone(),
        ),
        None => (String::new(), String::new(), String::new()),
    };

    let build_date = k_version.split(['(', ')']).nth(1).unwrap_or("unknown");

    let boot_mode = if let Some(NodeStatus::Common(node_status)) = data {
        Some(&node_status.boot_info)
    } else {
        None
    };

    Container::new()
        .class("pwt-d-grid pwt-gap-2 pwt-align-items-center")
        .style("grid-template-columns", "1fr 20px 1fr")
        .style("height", "fit-content")
        .padding(4)
        .with_child(
            MeterLabel::with_zero_optimum(tr!("CPU Usage"))
                .animated(true)
                .icon_class("fa fa-fw fa-cpu")
                .value(cpu as f32)
                .status(format!("{:.2}% of {} CPU(s)", cpu * 100.0, cpus_total)),
        )
        .with_child(
            MeterLabel::with_zero_optimum(tr!("IO delay"))
                .animated(true)
                .style("grid-column", "3")
                .icon_class("fa fa-fw fa-clock-o")
                .value(wait as f32),
        )
        .with_child(Container::new().padding(2).style("grid-column", "1/-1"))
        .with_child({
            let fraction = ((memory_used as f64) / (memory_total as f64)) as f32;
            MeterLabel::with_zero_optimum(tr!("RAM Usage"))
                .low(0.9) // memory is there to be used!
                .high(0.975)
                .animated(true)
                .icon_class("fa fa-fw fa-memory")
                .value(fraction)
                .status(format!(
                    "{:.2}% ({} of {})",
                    fraction * 100.0,
                    HumanByte::from(memory_used),
                    HumanByte::from(memory_total),
                ))
        })
        .with_child(
            StatusRow::new(tr!("Load Average"))
                .icon_class("fa fa-fw fa-tasks")
                .status(loadavg)
                .style("grid-column", "3"),
        )
        .with_child({
            let fraction = ((root_used as f64) / (root_total as f64)) as f32;
            MeterLabel::with_zero_optimum(tr!("HD space (root)"))
                .animated(true)
                .icon_class("fa fa-fw fa-hdd-o")
                .value(fraction)
                .status(format!(
                    "{:.2}% ({} of {})",
                    fraction * 100.0,
                    HumanByte::new_decimal(root_used as f64),
                    HumanByte::new_decimal(root_total as f64),
                ))
        })
        .with_child({
            let (fraction, status) = if swap_total > 0 {
                let fraction = ((swap_used as f64) / (swap_total as f64)) as f32;
                let status = format!(
                    "{:.2}% ({} of {})",
                    fraction * 100.0,
                    HumanByte::from(swap_used),
                    HumanByte::from(swap_total),
                );
                (Some(fraction), status)
            } else {
                (None, tr!("N/A"))
            };
            MeterLabel::with_zero_optimum(tr!("SWAP usage"))
                .animated(true)
                .style("grid-column", "3")
                .icon_class("fa fa-fw fa-refresh")
                .value(fraction)
                .animated(true)
                .status(status)
        })
        .with_child(Container::new().padding(2).style("grid-column", "1/-1"))
        .with_child({
            let cpu_model_text = format!(
                "{} x {} ({})",
                cpus_total,
                model,
                ngettext!("1 Socket", "{n} Sockets", sockets),
            );
            StatusRow::new(tr!("CPU(s)"))
                .style("grid-column", "1/-1")
                .status(cpu_model_text)
        })
        .with_optional_child(version.map(|version| {
            StatusRow::new(tr!("Version"))
                .style("grid-column", "1/-1")
                .status(version)
        }))
        .with_child(
            StatusRow::new(tr!("Kernel Version"))
                .style("grid-column", "1/-1")
                .status(format!("{k_sysname} {k_release} ({build_date})")),
        )
        .with_optional_child(boot_mode.map(|m| {
            let mode = match m.mode {
                BootMode::Efi => tr!("Legacy BIOS"),
                BootMode::LegacyBios if m.secureboot => tr!("UEFI (Secure Boot Enabled)"),
                BootMode::LegacyBios => tr!("UEFI"),
            };
            StatusRow::new(tr!("Boot Mode"))
                .style("grid-column", "1/-1")
                .status(mode)
        }))
}
