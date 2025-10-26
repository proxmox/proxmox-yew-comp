pub mod card;
pub mod list_tile;
pub mod mobile_form;

use pwt::prelude::*;

/// Render data, error, or progress indicator
pub fn render_loaded_data<T, E: std::fmt::Display, F: Fn(&T) -> Html>(
    data: &Option<Result<T, E>>,
    renderer: F,
) -> Html {
    match data {
        None => pwt::widget::Progress::new()
            .class("pwt-delay-visibility")
            .into(),
        Some(Err(err)) => pwt::widget::error_message(&err.to_string())
            .padding(2)
            .into(),
        Some(Ok(data)) => renderer(&data),
    }
}
