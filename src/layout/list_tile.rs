use pwt::prelude::*;
use pwt::props::IntoOptionalInlineHtml;
use pwt::widget::{Column, Container, Fa, ListTile, Progress, Row};

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

pub fn standard_list_tile(
    title: impl IntoOptionalInlineHtml,
    subtitle: impl IntoOptionalInlineHtml,
    leading: impl IntoOptionalInlineHtml,
    trailing: impl IntoOptionalInlineHtml,
) -> ListTile {
    let leading = leading
        .into_optional_inline_html()
        .unwrap_or(html! {<div/>});

    ListTile::new()
        .class(pwt::css::AlignItems::Center)
        .class("pwt-column-gap-2")
        .class("pwt-row-gap-1")
        //.class("pwt-scheme-surface")
        .border_bottom(true)
        .with_child(leading)
        .with_child(title_subtitle_column(title, subtitle))
        .with_optional_child(trailing.into_optional_inline_html())
}

pub fn icon_list_tile(
    icon: impl Into<Fa>,
    title: impl IntoOptionalInlineHtml,
    subtitle: impl IntoOptionalInlineHtml,
    trailing: impl IntoOptionalInlineHtml,
) -> ListTile {
    let icon: Html = icon.into().fixed_width().large_2x().into();
    standard_list_tile(title, subtitle, icon, trailing)
}

pub fn list_tile_usage(
    left_text: impl Into<AttrValue>,
    right_text: impl Into<AttrValue>,
    percentage: f32,
) -> Column {
    let progress = Progress::new().value(percentage);

    let text_row = Row::new()
        .gap(2)
        .class("pwt-align-items-flex-end")
        .with_child(html! {
            <div class="pwt-font-size-title-small pwt-flex-fill">{left_text.into()}</div>
        })
        .with_child(html! {
            <div class="pwt-font-size-title-small">{right_text.into()}</div>
        });

    Column::new()
        .gap(1)
        .style("grid-column", "1 / -1")
        .with_child(text_row)
        .with_child(progress)
}
