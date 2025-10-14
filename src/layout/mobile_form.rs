use pwt::prelude::*;
use pwt::props::PwtSpace;
use pwt::widget::{Column, FieldLabel};

/// Column with label and field
pub fn label_widget(label: impl Into<AttrValue>, field: impl Into<Html>) -> Column {
    Column::new()
        .with_child(FieldLabel::new(label.into()).padding_bottom(PwtSpace::Em(0.3)))
        .with_child(field)
        .into()
}

/// Column with label and field
///
/// This disables both label and field if enabled is not set. Also connect the label
/// to the field using a `label_id`.
pub fn label_field(
    label: impl Into<FieldLabel>,
    field: impl FieldBuilder,
    enabled: bool,
) -> Column {
    let label_id = pwt::widget::get_unique_element_id();

    Column::new()
        .with_child(
            label
                .into()
                .id(label_id.clone())
                .padding_bottom(PwtSpace::Em(0.3))
                .class((!enabled).then(|| "pwt-label-disabled")),
        )
        .with_child(field.label_id(label_id).disabled(!enabled))
        .into()
}
