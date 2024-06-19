use std::rc::Rc;

use anyhow::Error;
use serde_json::Value;

use pwt::prelude::*;
use pwt::widget::form::FormContext;
use pwt::widget::InputPanel;

use crate::utils::render_epoch;
use crate::{ObjectGrid, ObjectGridRow, TimezoneSelector};

async fn store_timezone(form: FormContext) -> Result<(), Error> {
    let value = form.get_submit_data();
    crate::http_put("/nodes/localhost/time", Some(value)).await
}

fn render_localtime(_name: &str, value: &Value, _record: &Value) -> Html {
    let date = js_sys::Date::new_0();
    let tz_offset = (date.get_timezone_offset() * 60.0) as i64;

    match value.as_i64() {
        Some(epoch) => {
            html! { {render_epoch(epoch + tz_offset)} }
        }
        None => {
            html! { "NaN" }
        }
    }
}

fn timezone_editor(_form_ctx: &FormContext, _name: &str, _value: &Value, _record: &Value) -> Html {
    InputPanel::new()
        .padding(4)
        .with_field(
            tr!("Time zone"),
            TimezoneSelector::new().name("timezone").autofocus(true),
        )
        .into()
}

#[function_component(TimePanel)]
pub fn time_panel() -> Html {
    let rows = Rc::new(vec![
        ObjectGridRow::new("timezone", tr!("Time zone"))
            .editor(timezone_editor)
            .required(true),
        ObjectGridRow::new("localtime", tr!("Server time"))
            .renderer(render_localtime)
            .required(true),
    ]);

    ObjectGrid::new()
        .editable(true)
        .loader("/nodes/localhost/time")
        .on_submit(store_timezone)
        .rows(rows)
        .into()
}
