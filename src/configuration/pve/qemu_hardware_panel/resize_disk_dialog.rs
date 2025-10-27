use anyhow::bail;
use serde_json::Value;

use pwt::prelude::*;
use pwt::widget::form::Number;
use pwt::widget::{Column, InputPanel};

use crate::layout::mobile_form::label_field;
use crate::{PropertyEditDialog, PropertyEditorState};

pub fn qemu_resize_disk_dialog(
    name: &str,
    _node: Option<AttrValue>,
    _remote: Option<AttrValue>,
    mobile: bool,
) -> PropertyEditDialog {
    let title = tr!("Resize Disk");

    PropertyEditDialog::new(title.clone() + " (" + name + ")")
        .mobile(mobile)
        .edit(false)
        .submit_text(title.clone())
        .submit_hook({
            let disk = name.to_string();
            move |state: PropertyEditorState| {
                let mut data = state.form_ctx.get_submit_data(); // get digest
                let incr = match state
                    .form_ctx
                    .read()
                    .get_last_valid_value("_size_increment_")
                {
                    Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
                    _ => bail!("invalid size increase - internal error"),
                };
                data["disk"] = disk.clone().into();
                data["size"] = format!("+{incr}G").into();
                Ok(data)
            }
        })
        .renderer(move |_| {
            let incr_label = tr!("Size Increment") + " (" + &tr!("GiB") + ")";
            let incr_field = Number::<f64>::new()
                .name("_size_increment_")
                .default(0.0)
                .min(0.0)
                .max(128.0 * 1024.0)
                .submit(false);
            if mobile {
                Column::new()
                    .class(pwt::css::FlexFit)
                    .gap(2)
                    .with_child(label_field(incr_label, incr_field, true))
                    .into()
            } else {
                InputPanel::new().with_field(incr_label, incr_field).into()
            }
        })
}
