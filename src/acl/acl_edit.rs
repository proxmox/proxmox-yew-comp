use yew::html::IntoPropValue;

use pwt::prelude::*;
use pwt::widget::form::{Checkbox, FormContext};
use pwt::widget::{FieldLabel, InputPanel};

use pwt_macros::builder;

use crate::EditWindow;
use crate::{AuthidSelector, RoleSelector};

pub trait AclEditWindow: Into<EditWindow> {}

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct AclEdit {
    /// Use API Tokens instead of Users.
    #[prop_or_default]
    #[builder]
    use_tokens: bool,

    /// The endpoint which will be used to create new ACL entries via a PUT request.
    #[prop_or(String::from("/access/acl"))]
    #[builder(IntoPropValue, into_prop_value)]
    acl_api_endpoint: String,

    #[prop_or_default]
    input_panel: InputPanel,
}

impl AclEdit {
    /// Create a new `AclEdit` that takes as input a field and its label which are used to select
    /// the ACL path for a new ACL entry.
    pub fn new(
        path_selector_label: impl Into<FieldLabel>,
        path_selector: impl FieldBuilder,
    ) -> Self {
        let path_selector = path_selector.name("path").required(true);
        let input_panel = InputPanel::new().with_field(path_selector_label, path_selector);
        yew::props!(Self { input_panel })
    }
}

impl From<AclEdit> for EditWindow {
    fn from(value: AclEdit) -> Self {
        let field = AuthidSelector::new().name("auth-id").required(true);

        let (title, authid_label, authid_field) = if value.use_tokens {
            (
                tr!("API Token Permission"),
                tr!("API Token"),
                field.include_users(false),
            )
        } else {
            (
                tr!("User Permission"),
                tr!("User"),
                field.include_tokens(false),
            )
        };

        let input_panel = value
            .input_panel
            .clone()
            .padding(4)
            .with_field(authid_label, authid_field)
            .with_field(tr!("Role"), RoleSelector::new().name("role").required(true))
            .with_field(
                tr!("Propagate"),
                Checkbox::new().name("propagate").required(true),
            );

        let url = value.acl_api_endpoint.to_owned();

        let on_submit = {
            let url = url.clone();
            move |form_ctx: FormContext| {
                let url = url.clone();
                async move {
                    let data = form_ctx.get_submit_data();
                    crate::http_put(url.as_str(), Some(data)).await
                }
            }
        };

        EditWindow::new(title)
            .renderer(move |_form_ctx: &FormContext| input_panel.clone().into())
            .on_submit(on_submit)
    }
}

impl AclEditWindow for AclEdit {}
