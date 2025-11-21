use pwt::prelude::*;
use pwt::widget::{Column, ConfirmDialog};

use crate::SafeConfirmDialog;

pub fn confirm_detach_entry(name: &str, _mobile: bool) -> ConfirmDialog {
    let message = tr!(
        "Are you sure you want to detach entry {0}",
        format!("'{name}'")
    );
    ConfirmDialog::default().confirm_message(message)
}

pub fn confirm_remove_entry(name: &str, _mobile: bool) -> ConfirmDialog {
    let message = tr!(
        "Are you sure you want to remove entry {0}",
        format!("'{name}'")
    );
    ConfirmDialog::default().confirm_message(message)
}

pub fn confirm_delete_volume(name: &str, volume: &str, mobile: bool) -> SafeConfirmDialog {
    let message1 = tr!("Are you sure you want to delete volume {0}.", volume);
    let message2 = tr!("This will permanently erase all data.");
    let message: Html = Column::new()
        .with_child(message1)
        .with_child(html! {<br/>})
        .with_child(message2)
        .into();

    SafeConfirmDialog::new(name.to_string())
        .mobile(mobile)
        .message(message)
        .submit_text(tr!("Remove"))
}
