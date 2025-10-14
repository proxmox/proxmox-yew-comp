use pwt::prelude::*;
use pwt::props::IntoOptionalInlineHtml;
use pwt::widget::{Column, Container, ListTile};

pub fn title_subtitle_column(
    title: impl IntoOptionalInlineHtml,
    subtitle: impl IntoOptionalInlineHtml,
) -> Column {
    let mut column = Column::new().gap(1);

    if let Some(title) = title.into_optional_inline_html() {
        column.add_child(
            Container::new()
                .class("pwt-font-size-title-medium")
                .style("overflow-wrap", "anywhere")
                .key("title")
                .with_child(title),
        );
    }

    if let Some(subtitle) = subtitle.into_optional_inline_html() {
        column.add_child(
            Container::new()
                .class("pwt-font-size-title-small")
                .style("overflow-wrap", "anywhere")
                .key("subtitle")
                .with_child(subtitle),
        );
    }
    column
}

pub fn form_list_tile(
    title: impl Into<AttrValue>,
    subtitle: impl IntoOptionalInlineHtml,
    trailing: impl IntoOptionalInlineHtml,
) -> ListTile {
    ListTile::new()
        .class(pwt::css::AlignItems::Center)
        .class("pwt-column-gap-2")
        .class("pwt-row-gap-1")
        //.class("pwt-scheme-surface")
        .border_bottom(true)
        .with_child(title_subtitle_column(title.into(), subtitle))
        .with_optional_child(trailing.into_optional_inline_html())
}
