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

use crate::{EditableProperty, PropertyEditorState};

fn policy_combobox(with_reject: bool) -> Combobox {
    let mut items = vec![("ACCEPT", tr!("Accept")), ("DROP", tr!("Drop"))];
    if with_reject {
        items.push(("REJECT", tr!("Reject")));
    }
    Combobox::from_key_value_pairs(items)
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

pub fn log_level_in_property(mobile: bool) -> EditableProperty {
    let title = tr!("Input log level");
    EditableProperty::new("log_level_in", title.clone())
        .required(true)
        .render_input_panel(move |_| {
            let log_level_field = LogLevelSelector::new().name("log_level_in");
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), log_level_field)
                .into()
        })
}

pub fn log_level_out_property(mobile: bool) -> EditableProperty {
    let title = tr!("Output log level");
    EditableProperty::new("log_level_out", title.clone())
        .required(true)
        .render_input_panel(move |_| {
            let log_level_field = LogLevelSelector::new().name("log_level_out");
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), log_level_field)
                .into()
        })
}

pub fn log_level_forward_property(mobile: bool) -> EditableProperty {
    let title = tr!("Forward log level");
    EditableProperty::new("log_level_forward", title.clone())
        .required(true)
        .render_input_panel(move |_| {
            let log_level_field = LogLevelSelector::new().name("log_level_forward");
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), log_level_field)
                .into()
        })
}

pub fn tcp_flags_log_level_property(mobile: bool) -> EditableProperty {
    let title = tr!("TCP Flags Log Level");
    EditableProperty::new("tcp_flags_log_level", title.clone())
        .required(true)
        .render_input_panel(move |_| {
            let log_level_field = LogLevelSelector::new().name("tcp_flags_log_level");
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), log_level_field)
                .into()
        })
}

pub fn smurf_log_level_property(mobile: bool) -> EditableProperty {
    let title = tr!("SMURF Log Level");
    EditableProperty::new("smurf_log_level", title.clone())
        .required(true)
        .render_input_panel(move |_| {
            let log_level_field = LogLevelSelector::new().name("smurf_log_level");
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), log_level_field)
                .into()
        })
}

pub fn input_policy_poperty(mobile: bool) -> EditableProperty {
    let title = tr!("Input Policy");
    EditableProperty::new("policy_in", title.clone())
        .required(true)
        .render_input_panel(move |_| {
            let input_policy_field = policy_combobox(true).name("policy_in").placeholder("DROP");
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), input_policy_field)
                .into()
        })
}

pub fn output_policy_poperty(mobile: bool) -> EditableProperty {
    let title = tr!("Output Policy");
    EditableProperty::new("policy_out", title.clone())
        .required(true)
        .render_input_panel(move |_| {
            let output_policy_field = policy_combobox(true)
                .name("policy_out")
                .placeholder("ACCEPT");
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), output_policy_field)
                .into()
        })
}

pub fn forward_policy_poperty(mobile: bool) -> EditableProperty {
    let title = tr!("Forward Policy");
    EditableProperty::new("policy_forward", title.clone())
        .required(true)
        .render_input_panel(move |_| {
            let forward_policy_field = policy_combobox(false)
                .name("policy_forward")
                .placeholder("ACCEPT");
            InputPanel::new()
                .mobile(mobile)
                .class(pwt::css::FlexFit)
                .padding_x(2)
                .with_field(title.clone(), forward_policy_field)
                .into()
        })
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
