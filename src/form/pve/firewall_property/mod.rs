mod log_ratelimit_selector;
pub use log_ratelimit_selector::LogRatelimitSelector;

mod log_ratelimit_property;
pub use log_ratelimit_property::log_ratelimit_property;

mod log_level_selector;
pub use log_level_selector::LogLevelSelector;

use pwt::prelude::*;
use pwt::widget::form::{Combobox, Number};
use pwt::widget::InputPanel;
use serde_json::{json, Value};

use crate::form::delete_empty_values;
use crate::{EditableProperty, PropertyEditorState};

fn policy_combobox(with_reject: bool) -> Combobox {
    let mut items = vec![("ACCEPT", tr!("Accept")), ("DROP", tr!("Drop"))];
    if with_reject {
        items.push(("REJECT", tr!("Reject")));
    }
    Combobox::from_key_value_pairs(items).submit_empty(true)
}

pub fn enable_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("enable", tr!("Enable Firewall"), false, mobile).required(true)
}

/// cluster wide enable firewall (stored as integer instead of bool)
pub fn cluster_enable_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("enable", tr!("Enable Firewall"), false, mobile)
        .required(true)
        .load_hook(|mut record: Value| {
            let enable = match record["enable"].as_u64() {
                Some(n) if n == 0 => false,
                Some(_) => true,
                None => false,
            };
            record["enable"] = Value::Bool(enable);
            Ok(record)
        })
        .submit_hook(|state: PropertyEditorState| {
            let form_ctx = state.form_ctx;
            let enable = match form_ctx.read().get_field_checked("enable") {
                true => 1,
                false => 0,
            };

            Ok(json!({ "enable": enable }))
        })
}

pub fn ebtables_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("ebtables", tr!("Enable ebtables"), false, mobile).required(true)
}

pub fn dhcp_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("dhcp", tr!("Enable DHCP"), true, mobile).required(true)
}

pub fn ndp_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("ndp", tr!("Enable NDP"), true, mobile).required(true)
}

pub fn radv_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("radv", tr!("Router Advertisement"), false, mobile).required(true)
}

pub fn macfilter_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("macfilter", tr!("MAC filter"), true, mobile).required(true)
}

pub fn ipfilter_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("ipfilter", tr!("IP filter"), false, mobile).required(true)
}

pub fn tcpflags_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("tcpflags", tr!("TCP flags filter"), false, mobile).required(true)
}

pub fn nosmurfs_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("nosmurfs", tr!("SMURFS filter"), false, mobile).required(true)
}

pub fn nftables_property(mobile: bool) -> EditableProperty {
    EditableProperty::new_bool("nftables", tr!("nftables (tech preview)"), false, mobile)
        .required(true)
}

pub fn log_level_property(name: &str, title: String, mobile: bool) -> EditableProperty {
    let name = name.to_string();
    EditableProperty::new(name.clone(), title.clone())
        .required(true)
        .placeholder("nolog")
        .render_input_panel({
            let name = name.clone();
            move |_| {
                let log_level_field = LogLevelSelector::new()
                    .name(name.clone())
                    .submit_empty(true);
                InputPanel::new()
                    .mobile(mobile)
                    .class(pwt::css::FlexFit)
                    .padding_x(2)
                    .with_field(title.clone(), log_level_field)
                    .into()
            }
        })
        .submit_hook(move |state: PropertyEditorState| {
            let data = state.form_ctx.get_submit_data();
            let data = delete_empty_values(&data, &[&name], false);
            Ok(data)
        })
}

pub fn log_level_in_property(mobile: bool) -> EditableProperty {
    log_level_property("log_level_in", tr!("Input log level"), mobile)
}

pub fn log_level_out_property(mobile: bool) -> EditableProperty {
    log_level_property("log_level_out", tr!("Output log level"), mobile)
}

pub fn log_level_forward_property(mobile: bool) -> EditableProperty {
    log_level_property("log_level_forward", tr!("Forward log level"), mobile)
}

pub fn tcp_flags_log_level_property(mobile: bool) -> EditableProperty {
    log_level_property("tcp_flags_log_level", tr!("TCP Flags Log Level"), mobile)
}

pub fn smurf_log_level_property(mobile: bool) -> EditableProperty {
    log_level_property("smurf_log_level", tr!("SMURF Log Level"), mobile)
}

fn policy_poperty(name: &str, title: String, placeholder: &str, mobile: bool) -> EditableProperty {
    let name = name.to_string();
    let placeholder = placeholder.to_string();
    EditableProperty::new(name.clone(), title.clone())
        .required(true)
        .placeholder(placeholder.clone())
        .render_input_panel({
            let name = name.clone();
            move |_| {
                let input_policy_field = policy_combobox(true)
                    .name(name.clone())
                    .placeholder(placeholder.clone());
                InputPanel::new()
                    .mobile(mobile)
                    .class(pwt::css::FlexFit)
                    .padding_x(2)
                    .with_field(title.clone(), input_policy_field)
                    .into()
            }
        })
        .submit_hook(move |state: PropertyEditorState| {
            let data = state.form_ctx.get_submit_data();
            let data = delete_empty_values(&data, &[&name], false);
            Ok(data)
        })
}

pub fn input_policy_poperty(mobile: bool) -> EditableProperty {
    policy_poperty("policy_in", tr!("Input Policy"), "DROP", mobile)
}

pub fn output_policy_poperty(mobile: bool) -> EditableProperty {
    policy_poperty("policy_out", tr!("Output Policy"), "ACCEPT", mobile)
}

pub fn forward_policy_poperty(mobile: bool) -> EditableProperty {
    policy_poperty("policy_forward", tr!("Forward Policy"), "ACCEPT", mobile)
}

pub fn nf_conntrack_max_poperty(mobile: bool) -> EditableProperty {
    let title = tr!("Connection Tracking Max");
    EditableProperty::new("nf_conntrack_max", title.clone())
        .required(true)
        .placeholder(tr!("Default"))
        .render_input_panel(move |_| {
            let field = Number::<u64>::new()
                .name("nf_conntrack_max")
                .placeholder(tr!("Default"));
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), field)
                .into()
        })
}

pub fn nf_timeout_established_poperty(mobile: bool) -> EditableProperty {
    let title = tr!("TCP Timeout Established");
    EditableProperty::new("nf_conntrack_tcp_timeout_established", title.clone())
        .required(true)
        .placeholder(tr!("Default"))
        .render_input_panel(move |_| {
            let field = Number::<u64>::new()
                .name("nf_conntrack_tcp_timeout_established")
                .placeholder(tr!("Default"));
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), field)
                .into()
        })
}
