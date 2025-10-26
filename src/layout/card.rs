use pwt::prelude::*;
use pwt::props::IntoOptionalInlineHtml;
use pwt::widget::{Card, Column, Container, Row};

pub fn standard_card(
    title: impl Into<AttrValue>,
    subtitle: impl IntoOptionalInlineHtml,
    trailing: impl IntoOptionalInlineHtml,
) -> Card {
    let title = title.into();

    let head: Html = match subtitle.into_optional_inline_html() {
        Some(subtitle) => Column::new()
            .gap(1)
            .with_child(html! {
                <div class="pwt-font-size-title-large">{title}</div>
            })
            .with_child(html! {
                <div class="pwt-font-size-title-small">{subtitle}</div>
            })
            .into(),
        None::<_> => Container::new()
            .class("pwt-font-size-title-large")
            .with_child(title)
            .into(),
    };

    let mut row = Row::new()
        .class(pwt::css::AlignItems::Center)
        .padding(2)
        .border_bottom(true)
        .gap(1)
        .with_child(head);
    if let Some(trailing) = trailing.into_optional_inline_html() {
        row.add_flex_spacer();
        row.add_child(trailing);
    }
    Card::new()
        .padding(0)
        .class("pwt-flex-none pwt-overflow-hidden")
        .with_child(row)
}
