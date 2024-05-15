use std::rc::Rc;

use anyhow::Error;
use serde_json::Value;

use crate::{ObjectGrid, ObjectGridRow, SchemaValidation};
use pwt::prelude::*;
use pwt::widget::form::{delete_empty_values, Field, FormContext};
use pwt::widget::InputPanel;

async fn store_dns(form_ctx: FormContext) -> Result<(), Error> {
    let data = form_ctx.get_submit_data();
    let data = delete_empty_values(&data, &["dns1", "dns2", "dns3"], true);
    crate::http_put("/nodes/localhost/dns", Some(data)).await
}

fn dns_editor(_form_ctx: &FormContext, _name: &str, _value: &Value, record: &Value) -> Html {
    InputPanel::new()
        .class("pwt-p-4")
        .with_field(
            tr!("Search domain"),
            Field::new()
                .name("search")
                .required(true)
                .default(record["search"].as_str().unwrap_or("").to_string())
                .schema(&proxmox_system_management_api::dns::SEARCH_DOMAIN_SCHEMA)
                .autofocus(true),
        )
        .with_field(
            tr!("DNS server 1"),
            Field::new()
                .name("dns1")
                .required(true)
                .default(record["dns1"].as_str().unwrap_or("").to_string())
                .schema(&proxmox_system_management_api::dns::FIRST_DNS_SERVER_SCHEMA),
        )
        .with_field(
            tr!("DNS server 2"),
            Field::new()
                .name("dns2")
                .default(record["dns2"].as_str().unwrap_or("").to_string())
                .schema(&proxmox_system_management_api::dns::SECOND_DNS_SERVER_SCHEMA),
            //.validate(validate_ip.clone()),
        )
        .with_field(
            tr!("DNS server 3"),
            Field::new()
                .name("dns3")
                .default(record["dns3"].as_str().unwrap_or("").to_string())
                .schema(&proxmox_system_management_api::dns::THIRD_DNS_SERVER_SCHEMA),
            //.validate(validate_ip),
        )
        .into()
}

#[function_component(DnsPanel)]
pub fn dns_panel() -> Html {
    let rows = Rc::new(vec![
        ObjectGridRow::new("search", tr!("Search domain"))
            .editor(dns_editor)
            .required(true),
        ObjectGridRow::new("dns1", tr!("DNS server 1"))
            .editor(dns_editor)
            .required(true),
        ObjectGridRow::new("dns2", tr!("DNS server 2")).editor(dns_editor),
        ObjectGridRow::new("dns3", tr!("DNS server 3")).editor(dns_editor),
    ]);

    ObjectGrid::new()
        .editable(true)
        .loader("/nodes/localhost/dns")
        .on_submit(store_dns)
        .rows(rows)
        .into()
}
