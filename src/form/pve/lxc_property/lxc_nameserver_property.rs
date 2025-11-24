use pwt::{
    prelude::*,
    widget::{form::Field, InputPanel},
};
use serde_json::Value;

use crate::{
    form::delete_empty_values, EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn,
};

const SEARCDOMAIN_PN: &'static str = "searchdomain";
const NAMESERVER_PN: &'static str = "nameserver";

fn renderer(_name: &str, value: &Value, _record: &Value) -> Html {
    match value {
        Value::Null => tr!("use host settings"),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
    .into()
}

fn input_panel(mobile: bool) -> RenderPropertyInputPanelFn {
    RenderPropertyInputPanelFn::new(move |_state: PropertyEditorState| {
        let domain_label = tr!("DNS domain");
        let domain_field = Field::new()
            .name(SEARCDOMAIN_PN)
            .submit_empty(true)
            .placeholder(tr!("use host settings"));

        let server_label = tr!("DNS server");
        let server_field = Field::new()
            .name(NAMESERVER_PN)
            .submit_empty(true)
            .placeholder(tr!("use host settings"));

        InputPanel::new()
            .class(pwt::css::FlexFit)
            .mobile(mobile)
            .padding_x(2)
            .with_field(domain_label, domain_field)
            .with_field(server_label, server_field)
            .into()
    })
}

pub fn lxc_nameserver_property(mobile: bool) -> EditableProperty {
    let title = tr!("DNS servers");
    EditableProperty::new(NAMESERVER_PN, title)
        .required(true)
        .renderer(renderer)
        .render_input_panel(input_panel(mobile))
        .submit_hook(|state: PropertyEditorState| {
            let form_ctx = state.form_ctx;
            let data = form_ctx.get_submit_data();
            let data = delete_empty_values(&data, &[NAMESERVER_PN, SEARCDOMAIN_PN], false);
            Ok(data)
        })
}

pub fn lxc_searchdomain_property(mobile: bool) -> EditableProperty {
    let title = tr!("DNS domain");
    EditableProperty::new(SEARCDOMAIN_PN, title)
        .required(true)
        .renderer(renderer)
        .render_input_panel(input_panel(mobile))
        .submit_hook(|state: PropertyEditorState| {
            let form_ctx = state.form_ctx;
            let data = form_ctx.get_submit_data();
            let data = delete_empty_values(&data, &[NAMESERVER_PN, SEARCDOMAIN_PN], false);
            Ok(data)
        })
}
