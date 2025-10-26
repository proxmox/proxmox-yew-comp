use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use proxmox_client::ApiResponseData;
use serde_json::Value;

use proxmox_access_control::types::UserWithTokens;
use proxmox_auth_api::types::Username;
use proxmox_schema::api_types::PASSWORD_SCHEMA;
use proxmox_schema::ApiType;

use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::form::{Checkbox, Field, FormContext, InputType};
use pwt::widget::{Button, Dialog, InputPanel, Toolbar};

use crate::form::delete_empty_values;
use crate::percent_encoding::percent_encode_component;
use crate::utils::{epoch_to_input_value, render_epoch_short};
use crate::{
    EditWindow, LoadableComponent, LoadableComponentContext, LoadableComponentMaster,
    PermissionPanel, RealmSelector, SchemaValidation,
};

async fn load_user_list() -> Result<Vec<UserWithTokens>, Error> {
    crate::http_get("/access/users", None).await
}

async fn load_user(userid: Key) -> Result<ApiResponseData<Value>, Error> {
    let url = format!("/access/users/{}", percent_encode_component(&userid));

    let mut resp: ApiResponseData<Value> = crate::http_get_full(&url, None).await?;

    if let Value::Number(number) = &resp.data["expire"] {
        if let Some(epoch) = number.as_f64() {
            resp.data["expire"] = Value::String(epoch_to_input_value(epoch as i64));
        }
    }

    Ok(resp)
}

async fn delete_user(userid: Key) -> Result<(), Error> {
    let url = format!("/access/users/{}", percent_encode_component(&userid));
    crate::http_delete(&url, None).await?;
    Ok(())
}

async fn create_user(form_ctx: FormContext) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();

    let username = form_ctx.read().get_field_text("username");
    let realm = form_ctx.read().get_field_text("realm");
    data["userid"] = Value::String(format!("{username}@{realm}"));

    let expire = form_ctx.read().get_field_text("expire");
    if let Ok(epoch) = proxmox_time::parse_rfc3339(&expire) {
        data["expire"] = epoch.into();
    }

    crate::http_post("/access/users", Some(data)).await
}

async fn update_user(form_ctx: FormContext) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();

    let expire = form_ctx.read().get_field_text("expire");
    if let Ok(epoch) = proxmox_time::parse_rfc3339(&expire) {
        data["expire"] = epoch.into();
    } else {
        data["expire"] = 0i64.into();
    }

    let data = delete_empty_values(&data, &["firstname", "lastname", "email", "comment"], true);

    let userid = form_ctx.read().get_field_text("userid");

    let url = format!("/access/users/{}", percent_encode_component(&userid));

    crate::http_put(&url, Some(data)).await
}

#[derive(PartialEq, Properties)]
pub struct UserPanel {}

impl Default for UserPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl UserPanel {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(PartialEq)]
pub enum ViewState {
    Add,
    Edit,
    ChangePassword,
    ShowPermissions,
}

pub enum Msg {
    SelectionChange,
    RemoveItem,
}

pub struct ProxmoxUserPanel {
    store: Store<UserWithTokens>,
    selection: Selection,
}

impl LoadableComponent for ProxmoxUserPanel {
    type Message = Msg;
    type Properties = UserPanel;
    type ViewState = ViewState;

    fn load(
        &self,
        _ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let store = self.store.clone();
        Box::pin(async move {
            let data = load_user_list().await?;
            store.write().set_data(data);
            Ok(())
        })
    }

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let store = Store::with_extract_key(|record: &UserWithTokens| {
            Key::from(record.user.userid.as_str())
        });

        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::SelectionChange));

        Self { store, selection }
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SelectionChange => true,
            Msg::RemoveItem => {
                if let Some(key) = self.selection.selected_key() {
                    let link = ctx.link();
                    link.clone().spawn(async move {
                        if let Err(err) = delete_user(key).await {
                            link.show_error(tr!("Unable to delete user"), err, true);
                        }
                        link.send_reload();
                    })
                }
                false
            }
        }
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let link = ctx.link();

        let disabled = self.selection.is_empty();

        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                Button::new(tr!("Add"))
                    .onclick(link.change_view_callback(|_| Some(ViewState::Add))),
            )
            .with_spacer()
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(disabled)
                    .onclick(link.change_view_callback(|_| Some(ViewState::Edit))),
            )
            .with_child(
                Button::new(tr!("Remove"))
                    .disabled(disabled)
                    .onclick(link.callback(|_| Msg::RemoveItem)),
            )
            .with_spacer()
            .with_child(
                Button::new(tr!("Change Password"))
                    .disabled(disabled)
                    .onclick(link.change_view_callback(|_| Some(ViewState::ChangePassword))),
            )
            .with_child(
                Button::new(tr!("Show Permissions"))
                    .disabled(disabled)
                    .onclick(link.change_view_callback(|_| Some(ViewState::ShowPermissions))),
            )
            .with_flex_spacer()
            .with_child({
                let loading = ctx.loading();
                let link = ctx.link();
                Button::refresh(loading).onclick(move |_| link.send_reload())
            });

        Some(toolbar.into())
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let link = ctx.link();
        DataTable::new(columns(), self.store.clone())
            .class("pwt-flex-fill pwt-overflow-auto")
            .selection(self.selection.clone())
            .striped(true)
            .on_row_dblclick(move |_: &mut _| {
                link.change_view(Some(ViewState::Edit));
            })
            .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        match view_state {
            ViewState::Add => Some(self.create_add_dialog(ctx)),
            ViewState::Edit => self
                .selection
                .selected_key()
                .map(|key| self.create_edit_dialog(ctx, key)),
            ViewState::ChangePassword => self
                .selection
                .selected_key()
                .map(|key| self.create_change_password_dialog(ctx, key)),
            ViewState::ShowPermissions => self
                .selection
                .selected_key()
                .map(|key| self.create_show_permissions_dialog(ctx, key)),
        }
    }
}

fn check_confirm_password(form_ctx: FormContext) {
    let pw = form_ctx.read().get_field_text("password");
    let confirm = form_ctx.read().get_field_text("confirm_password");
    if !confirm.is_empty() {
        let valid = if pw == confirm {
            Ok(confirm.into())
        } else {
            Err(tr!("Passwords do not match!"))
        };
        form_ctx.write().set_field_valid("confirm_password", valid);
    }
}

impl ProxmoxUserPanel {
    fn create_add_dialog(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        EditWindow::new(tr!("Add") + ": " + &tr!("User"))
            .renderer(add_user_input_panel)
            .on_submit(create_user)
            .on_done(ctx.link().change_view_callback(|_| None))
            .on_change(check_confirm_password)
            .into()
    }

    fn create_edit_dialog(&self, ctx: &LoadableComponentContext<Self>, key: Key) -> Html {
        EditWindow::new(tr!("Edit") + ": " + &tr!("User"))
            .renderer(edit_user_input_panel)
            .on_submit(update_user)
            .on_done(ctx.link().change_view_callback(|_| None))
            .loader(move || load_user(key.clone()))
            .into()
    }

    fn create_change_password_dialog(
        &self,
        ctx: &LoadableComponentContext<Self>,
        key: Key,
    ) -> Html {
        EditWindow::new(tr!("Change Password"))
            .renderer(password_change_input_panel)
            .on_submit(update_user)
            .on_done(ctx.link().change_view_callback(|_| None))
            .on_change(check_confirm_password)
            .loader(move || load_user(key.clone()))
            .into()
    }

    fn create_show_permissions_dialog(
        &self,
        ctx: &LoadableComponentContext<Self>,
        key: Key,
    ) -> Html {
        Dialog::new(key.to_string() + " - " + &tr!("Granted Permissions"))
            .resizable(true)
            .width(840)
            .height(600)
            .with_child(PermissionPanel::new().auth_id(key.to_string()))
            .on_close(ctx.link().change_view_callback(|_| None))
            .into()
    }
}

impl From<UserPanel> for VNode {
    fn from(val: UserPanel) -> Self {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxUserPanel>>(Rc::new(val), None);
        VNode::from(comp)
    }
}

thread_local! {
    static COLUMNS: Rc<Vec<DataTableHeader<UserWithTokens>>> = Rc::new(vec![
        DataTableColumn::new(tr!("Username"))
            .flex(1)
            .justify("flex-start")
            .render(|item: &UserWithTokens| {
                html!{item.user.userid.name().as_str()}
            })
            .sorter(|a: &UserWithTokens, b: &UserWithTokens| {
                a.user.userid.cmp(&b.user.userid)
            })
            .sort_order(true)
            .into(),

        DataTableColumn::new(tr!("Realm"))
            .render(|item: &UserWithTokens| {
                html!{item.user.userid.realm().as_str()}
            })
            .sorter(|a: &UserWithTokens, b: &UserWithTokens| {
                a.user.userid.realm().as_str().cmp(b.user.userid.realm().as_str())
            })
            .into(),

        DataTableColumn::new(tr!("Enabled"))
            .justify("center")
            .render({
                let yes_text = tr!("Yes");
                let no_text = tr!("No");

                move |item: &UserWithTokens| html!{
                    {
                        match item.user.enable {
                            Some(true) => &yes_text,
                            Some(false) => &no_text,
                            None => &yes_text,
                        }
                    }
                }
            })
            .sorter(|a: &UserWithTokens, b: &UserWithTokens| {
                a.user.enable.cmp(&b.user.enable)
            })
            .into(),

        DataTableColumn::new(tr!("Expire"))
            .render({
                let never_text = tr!("never");
                move |item: &UserWithTokens| html!{
                    {
                        match item.user.expire {
                            Some(epoch) if epoch != 0 => render_epoch_short(epoch),
                            _ => never_text.clone(),
                        }
                    }
                }
            })
            .sorter(|a: &UserWithTokens, b: &UserWithTokens| {
                let a = if let Some(0) = a.user.expire { None } else { a.user.expire };
                let b = if let Some(0) = b.user.expire { None } else { b.user.expire };
                a.cmp(&b)
            })
            .into(),

        DataTableColumn::new(tr!("Name"))
            .flex(1)
            .render(|item: &UserWithTokens| {
                html!{
                    {
                        match (&item.user.firstname, &item.user.lastname) {
                            (Some(f), Some(l)) => format!("{} {}", f, l),
                            (Some(f), None) => f.clone(),
                            (None, Some(l)) => l.clone(),
                            (None, None) => String::new(),
                        }
                    }
                }
            })
            .sorter(|a: &UserWithTokens, b: &UserWithTokens| {
                let a = match (&a.user.firstname, &a.user.lastname) {
                    (Some(f), Some(l)) => format!("{} {}", f, l),
                    (Some(f), None) => f.clone(),
                    (None, Some(l)) => l.clone(),
                    (None, None) => String::new(),
                };
                let b = match (&b.user.firstname, &b.user.lastname) {
                    (Some(f), Some(l)) => format!("{} {}", f, l),
                    (Some(f), None) => f.clone(),
                    (None, Some(l)) => l.clone(),
                    (None, None) => String::new(),
                };
                a.cmp(&b)
            })
            .into(),

        DataTableColumn::new(tr!("Email"))
            .flex(1)
            .render(|item: &UserWithTokens| {
                html!{ { item.user.email.clone().unwrap_or_default() } }
            })
            .sorter(|a: &UserWithTokens, b: &UserWithTokens| {
                a.user.email.cmp(&b.user.email)
            })
            .into(),

        DataTableColumn::new(tr!("Comment"))
            .flex(1)
            .render(|item: &UserWithTokens| {
                html!{ { item.user.comment.clone().unwrap_or_default() } }
            })
            .sorter(|a: &UserWithTokens, b: &UserWithTokens| {
                a.user.comment.cmp(&b.user.comment)
            })
            .into(),
    ]);
}

fn columns() -> Rc<Vec<DataTableHeader<UserWithTokens>>> {
    COLUMNS.with(Rc::clone)
}

fn password_change_input_panel(_form_ctx: &FormContext) -> Html {
    InputPanel::new()
        .padding(4)
        .with_field(
            tr!("User name"),
            Field::new()
                .name("userid")
                .required(true)
                .disabled(true)
                .schema(&Username::API_SCHEMA)
                .submit(false),
        )
        .with_field(
            tr!("Password"),
            Field::new()
                .name("password")
                .required(true)
                .schema(&PASSWORD_SCHEMA)
                .input_type(InputType::Password),
        )
        // fixme: validate confirmation
        .with_field(
            tr!("Confirm password"),
            Field::new()
                .name("confirm_password")
                .required(true)
                .submit(false)
                .input_type(InputType::Password),
        )
        .into()
}

fn add_user_input_panel(_form_ctx: &FormContext) -> Html {
    InputPanel::new()
        .padding(4)
        .with_field(
            tr!("User name"),
            Field::new()
                .name("username")
                .required(true)
                .autofocus(true)
                .schema(&Username::API_SCHEMA)
                .submit(false),
        )
        .with_field(
            tr!("Realm"),
            RealmSelector::new()
                .name("realm")
                .required(true)
                .submit(false),
        )
        .with_field(
            tr!("Password"),
            Field::new()
                .name("password")
                .required(true)
                .schema(&PASSWORD_SCHEMA)
                .input_type(InputType::Password),
        )
        // fixme: validate confirmation
        .with_field(
            tr!("Confirm password"),
            Field::new()
                .name("confirm_password")
                .required(true)
                .submit(false)
                .input_type(InputType::Password),
        )
        .with_field(
            tr!("Expire"),
            Field::new()
                .name("expire")
                .input_type(InputType::DatetimeLocal),
        )
        .with_field(tr!("Enabled"), Checkbox::new().name("enable").default(true))
        .with_right_field(tr!("First name"), Field::new().name("firstname"))
        .with_right_field(tr!("Last name"), Field::new().name("lastname"))
        .with_right_field(tr!("EMail"), Field::new().name("email"))
        .with_large_field(tr!("Comment"), Field::new().name("comment"))
        .into()
}

fn edit_user_input_panel(_form_ctx: &FormContext) -> Html {
    InputPanel::new()
        .padding(4)
        .with_field(
            tr!("User name"),
            Field::new()
                .name("userid")
                .required(true)
                .disabled(true)
                .schema(&Username::API_SCHEMA)
                .submit(false),
        )
        .with_right_field(tr!("First name"), Field::new().name("firstname"))
        .with_field(
            tr!("Expire"),
            Field::new()
                .name("expire")
                .input_type(InputType::DatetimeLocal),
        )
        .with_right_field(tr!("Last name"), Field::new().name("lastname"))
        .with_field(tr!("EMail"), Field::new().name("email"))
        .with_right_field(tr!("Enabled"), Checkbox::new().name("enable").default(true))
        .with_large_field(tr!("Comment"), Field::new().name("comment").autofocus(true))
        .into()
}
