use pwt::props::FieldBuilder as _;
use pwt::tr;
use pwt::widget::form::Field;
use pwt::widget::InputPanel;

use crate::SchemaValidation as _;

mod webauthn;
pub use webauthn::{ProxmoxWebAuthn, WebAuthn};

mod tfa_dialog;
pub use tfa_dialog::{ProxmoxTfaDialog, TfaDialog};

pub(self) mod tfa_view;
pub use tfa_view::{ProxmoxTfaView, TfaView};

mod tfa_edit;
pub use tfa_edit::{ProxmoxTfaEdit, TfaEdit};

mod tfa_add_totp;
pub use tfa_add_totp::{ProxmoxTfaAddTotp, TfaAddTotp};

mod tfa_add_webauthn;
pub use tfa_add_webauthn::{ProxmoxTfaAddWebauthn, TfaAddWebauthn};

mod tfa_add_recovery;
pub use tfa_add_recovery::{ProxmoxTfaAddRecovery, TfaAddRecovery};

pub(self) mod tfa_confirm_remove;

/// If we're not logged in as `root@pam`, the user needs to provide their password as a
/// confirmation when modifying TFA entries, including their own.
pub fn add_password_field(panel: InputPanel, large: bool) -> InputPanel {
    let Some(auth) = crate::http_get_auth() else {
        return panel;
    };
    let userid = auth.userid;

    if userid != "root@pam" {
        let field = Field::new()
            .name("password")
            .required(true)
            .schema(&proxmox_schema::api_types::PASSWORD_SCHEMA)
            .input_type(pwt::widget::form::InputType::Password)
            .placeholder(tr!("Confirm your ({}) password", userid));
        match large {
            true => panel.with_large_field(tr!("Password"), field),
            false => panel.with_field(tr!("Password"), field),
        }
    } else {
        panel
    }
}
