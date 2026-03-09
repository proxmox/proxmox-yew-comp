use proxmox_schema::ApiType;
use pve_api_types::ClusterFirewallOptionsLogRatelimit;
use serde_json::Value;

use pwt::prelude::*;
use pwt::widget::form::{Checkbox, Number};
use pwt::widget::InputPanel;

use crate::form::{
    delete_empty_values, flatten_property_string, get_field_schema, property_string_from_parts,
};
use crate::RenderPropertyInputPanelFn;
use crate::{EditableProperty, PropertyEditorState, SchemaValidation};

use super::LogRatelimitSelector;

const LOG_RATELIMIT_PN: &'static str = "log_ratelimit";
const ENABLE_PN: &'static str = "_enable";
const RATE_PN: &'static str = "_rate";
const BURST_PN: &'static str = "_burst";

const RATE_FIELD_NAME: &'static str = "_rate_";
const UNIT_FIELD_NAME: &'static str = "_unit_";

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |_state: PropertyEditorState| {
        let base_schema = &pve_api_types::ClusterFirewallOptionsLogRatelimit::API_SCHEMA;
        let burst_schema = get_field_schema(base_schema, vec!["burst"]);

        let enable_label = tr!("Enable");
        let enable_field = Checkbox::new().switch(mobile).name(ENABLE_PN);

        let rate_label = tr!("Log rate limit");
        let rate_field = LogRatelimitSelector::new()
            .rate_name(RATE_FIELD_NAME)
            .unit_name(UNIT_FIELD_NAME);

        let burst_label = tr!("Burst");
        let burst_field = Number::<u64>::new().name(BURST_PN).schema(burst_schema);

        InputPanel::new()
            .mobile(mobile)
            .label_width("max-content")
            .class(pwt::css::FlexFit)
            .padding_x(2)
            .padding_bottom(1) // avoid scrollbar
            .with_single_line_field(false, false, enable_label, enable_field)
            .with_field(rate_label, rate_field)
            .with_field(burst_label, burst_field)
            .into()
    })
}

pub fn log_ratelimit_property(mobile: bool) -> EditableProperty {
    let placeholder = tr!("Default") + " (enable=1,rate1/second,burst=5)";
    EditableProperty::new(LOG_RATELIMIT_PN, tr!("Log rate limiting"))
        .required(true)
        .placeholder(placeholder)
        .render_input_panel(input_panel(mobile))
        .load_hook(|mut record| {
            flatten_property_string::<ClusterFirewallOptionsLogRatelimit>(
                &mut record,
                LOG_RATELIMIT_PN,
            )?;

            if let Value::String(rate) = record[RATE_PN].clone() {
                if let Some((num, unit)) = rate.split_once('/') {
                    record[RATE_FIELD_NAME] = num.into();
                    record[UNIT_FIELD_NAME] = unit.into();
                }
            }
            Ok(record)
        })
        .submit_hook(|state: PropertyEditorState| {
            let form_ctx = state.form_ctx;
            let mut form_data = form_ctx.get_submit_data();

            let rate = form_ctx.read().get_field_text(RATE_FIELD_NAME);
            let unit = form_ctx.read().get_field_text(UNIT_FIELD_NAME);

            form_data[RATE_PN] = format!("{rate}/{unit}").into();

            property_string_from_parts::<ClusterFirewallOptionsLogRatelimit>(
                &mut form_data,
                LOG_RATELIMIT_PN,
                true,
            )?;

            let form_data = delete_empty_values(&form_data, &[LOG_RATELIMIT_PN], false);
            Ok(form_data)
        })
}
