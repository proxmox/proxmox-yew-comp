use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use proxmox_access_control::types::{ApiToken, UserWithTokens};
use proxmox_auth_api::types::Authid;
use proxmox_client::ApiResponseData;
use serde_json::{json, Value};

use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::form::{Checkbox, DisplayField, Field, FormContext, InputType};
use pwt::widget::{Button, Column, Container, Dialog, InputPanel, Toolbar};

use crate::percent_encoding::percent_encode_component;
use crate::utils::{
    copy_text_to_clipboard, epoch_to_input_value, render_boolean, render_epoch_short,
};
use crate::{
    AuthidSelector, ConfirmButton, EditWindow, LoadableComponent, LoadableComponentContext,
    LoadableComponentLink, LoadableComponentMaster, PermissionPanel,
};

async fn load_api_tokens() -> Result<Vec<ApiToken>, Error> {
    let url = "/access/users/?include_tokens=1";
    let users: Vec<UserWithTokens> = crate::http_get(url, None).await?;

    Ok(users.into_iter().flat_map(|user| user.tokens).collect())
}

async fn create_token(
    form_ctx: FormContext,
    link: LoadableComponentLink<ProxmoxTokenView>,
) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();

    let userid = form_ctx.read().get_field_text("userid");
    let tokenname = form_ctx.read().get_field_text("tokenname");

    let url = token_api_url(&userid, &tokenname);

    let expire = form_ctx.read().get_field_text("expire");

    if let Ok(epoch) = proxmox_time::parse_rfc3339(&expire) {
        data["expire"] = epoch.into();
    }

    let res: Value = crate::http_post(url, Some(data)).await?;

    link.change_view(Some(ViewState::DisplayTokenSecret(res)));

    Ok(())
}

async fn load_token(tokenid: Key) -> Result<ApiResponseData<Value>, Error> {
    let tokenid: Authid = tokenid.parse().unwrap();

    let userid = tokenid.user().to_string();
    let tokenname = tokenid.tokenname().map(|n| n.as_str().to_owned()).unwrap();

    let url = token_api_url(&userid, &tokenname);

    let mut resp: ApiResponseData<Value> = crate::http_get_full(&url, None).await?;

    if let Value::Number(number) = &resp.data["expire"] {
        if let Some(epoch) = number.as_f64() {
            resp.data["expire"] = Value::String(epoch_to_input_value(epoch as i64));
        }
    }
    resp.data["userid"] = userid.into();
    resp.data["tokenname"] = tokenname.into();

    Ok(resp)
}

async fn update_token(form_ctx: FormContext) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();

    let userid = form_ctx.read().get_field_text("userid");
    let tokenname = form_ctx.read().get_field_text("tokenname");

    let url = token_api_url(&userid, &tokenname);

    let expire = form_ctx.read().get_field_text("expire");
    data["expire"] = proxmox_time::parse_rfc3339(&expire).unwrap_or(0).into();

    crate::http_put(url, Some(data)).await
}

#[derive(PartialEq, Properties)]
pub struct TokenPanel {}

impl TokenPanel {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

impl Default for TokenPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, PartialEq)]
enum ViewState {
    AddToken,
    EditToken,
    ShowPermissions,
    DisplayTokenSecret(Value),
}

enum Msg {
    Refresh,
    Remove,
    Regenerate,
}

struct ProxmoxTokenView {
    selection: Selection,
    store: Store<ApiToken>,
    columns: Rc<Vec<DataTableHeader<ApiToken>>>,
}

fn token_api_url(user: &str, tokenname: &str) -> String {
    format!(
        "/access/users/{}/token/{}",
        percent_encode_component(user),
        percent_encode_component(tokenname),
    )
}

impl LoadableComponent for ProxmoxTokenView {
    type Properties = TokenPanel;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let link = ctx.link();
        link.repeated_load(5000);

        let selection = Selection::new().on_select(link.callback(|_| Msg::Refresh));
        let store =
            Store::with_extract_key(|record: &ApiToken| Key::from(record.tokenid.to_string()));

        Self {
            selection,
            store,
            columns: columns(),
        }
    }

    fn load(
        &self,
        _ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
        let store = self.store.clone();
        Box::pin(async move {
            let data = load_api_tokens().await?;
            store.write().set_data(data);
            Ok(())
        })
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let selected_id = self.selection.selected_key().map(|k| k.to_string());
        let disabled = selected_id.is_none();
        let link = ctx.link();

        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .border_top(true)
            .with_child(
                Button::new(tr!("Add"))
                    .on_activate(link.change_view_callback(|_| Some(ViewState::AddToken))),
            )
            .with_spacer()
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(disabled)
                    .on_activate(link.change_view_callback(|_| Some(ViewState::EditToken))),
            )
            .with_child(
                Button::new(tr!("Remove"))
                    .disabled(disabled)
                    .on_activate(link.callback(|_| Msg::Remove)),
            )
            .with_spacer()
            .with_child(
                ConfirmButton::new(tr!("Regenerate Secret"))
                    .confirm_message(tr!(
                        "Do you want to regenerate the secret of the selected API token? \
                        All current usage sites will lose access!"
                    ))
                    .disabled(disabled)
                    .on_activate(link.callback(|_| Msg::Regenerate)),
            )
            .with_spacer()
            .with_child(
                Button::new(tr!("Show Permissions"))
                    .disabled(disabled)
                    .on_activate(link.change_view_callback(|_| Some(ViewState::ShowPermissions))),
            );

        Some(toolbar.into())
    }

    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Refresh => true,
            Msg::Remove => {
                let Some(record) = self.get_selected_record() else {
                    return false;
                };

                let user = record.tokenid.user().to_string();
                let Some(tokenname) = record.tokenid.tokenname() else {
                    log::error!("internal error: API token '{}' has no name", record.tokenid);
                    return true;
                };

                let url = token_api_url(&user, tokenname.as_str());
                let link = ctx.link();
                link.clone().spawn(async move {
                    match crate::http_delete(url, None).await {
                        Ok(()) => {
                            link.send_reload();
                        }
                        Err(err) => {
                            link.show_error("Removing API token failed", err, true);
                        }
                    }
                });
                false
            }
            Msg::Regenerate => {
                let Some(record) = self.get_selected_record() else {
                    return false;
                };
                let user = record.tokenid.user().to_string();
                let Some(tokenname) = record.tokenid.tokenname() else {
                    log::error!("internal error: API token '{}' has no name", record.tokenid);
                    return true;
                };

                let url = token_api_url(&user, tokenname.as_str());
                let link = ctx.link().clone();
                ctx.link().spawn(async move {
                    match crate::http_put(url, Some(json!({"regenerate": true}))).await {
                        Ok(secret) => {
                            link.change_view(Some(ViewState::DisplayTokenSecret(secret)));
                        }
                        Err(err) => {
                            link.show_error("Regenerating API token failed", err, true);
                        }
                    }
                });
                false
            }
        }
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let link = ctx.link();

        DataTable::new(self.columns.clone(), self.store.clone())
            .class("pwt-flex-fit")
            .selection(self.selection.clone())
            .on_row_dblclick(move |_: &mut _| {
                link.change_view(Some(ViewState::EditToken));
            })
            .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        match view_state {
            ViewState::AddToken => Some(self.create_add_dialog(ctx)),
            ViewState::EditToken => self
                .selection
                .selected_key()
                .map(|key| self.create_edit_dialog(ctx, key)),
            ViewState::ShowPermissions => self
                .selection
                .selected_key()
                .map(|key| self.create_show_permissions_dialog(ctx, key)),
            ViewState::DisplayTokenSecret(secret) => Some(self.show_secret_dialog(ctx, secret)),
        }
    }
}

impl ProxmoxTokenView {
    fn get_selected_record(&self) -> Option<ApiToken> {
        self.selection
            .selected_key()
            .map(|key| self.store.read().lookup_record(&key).cloned())
            .flatten()
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

    fn show_secret_dialog(&self, ctx: &LoadableComponentContext<Self>, secret: &Value) -> Html {
        let secret = secret.clone();

        Dialog::new(tr!("Token Secret"))
            .with_child(
                Column::new()
                    .with_child(
                        InputPanel::new()
                            .padding(4)
                            .with_large_field(
                                tr!("Token ID"),
                                DisplayField::new()
                                    .value(AttrValue::from(
                                        secret["tokenid"].as_str().unwrap_or("").to_owned(),
                                    ))
                                    .border(true),
                            )
                            .with_large_field(
                                tr!("Secret"),
                                DisplayField::new()
                                    .value(AttrValue::from(
                                        secret["value"].as_str().unwrap_or("").to_owned(),
                                    ))
                                    .border(true),
                            ),
                    )
                    .with_child(
                        Container::new()
                            .style("opacity", "0")
                            .with_child(AttrValue::from(
                                secret["value"].as_str().unwrap_or("").to_owned(),
                            )),
                    )
                    .with_child(
                        Container::new()
                            .padding(4)
                            .class(pwt::css::FlexFit)
                            .class("pwt-bg-color-warning-container")
                            .class("pwt-color-on-warning-container")
                            .with_child(tr!(
                                "Please record the API token secret - it will only be displayed now"
                            )),
                    )
                    .with_child(
                        Toolbar::new()
                            .class("pwt-bg-color-surface")
                            .with_flex_spacer()
                            .with_child(
                                Button::new(tr!("Copy Secret Value"))
                                    .icon_class("fa fa-clipboard")
                                    .class("pwt-scheme-primary")
                                    .on_activate({
                                        move |_| {
                                            copy_text_to_clipboard(
                                                secret["value"].as_str().unwrap_or(""),
                                            )
                                        }
                                    }),
                            ),
                    ),
            )
            .on_close(ctx.link().change_view_callback(|_| None))
            .into()
    }

    fn create_add_dialog(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let link = ctx.link().clone();
        EditWindow::new(tr!("Add") + ": " + &tr!("Token"))
            .renderer(add_input_panel)
            .on_submit(move |form_ctx| {
                let link = link.clone();
                create_token(form_ctx, link)
            })
            .on_close(ctx.link().change_view_callback(|_| None))
            .into()
    }

    fn create_edit_dialog(&self, ctx: &LoadableComponentContext<Self>, key: Key) -> Html {
        EditWindow::new(tr!("Edit") + ": " + &tr!("Token"))
            .renderer(edit_input_panel)
            .on_submit(update_token)
            .on_done(ctx.link().change_view_callback(|_| None))
            .loader(move || load_token(key.clone()))
            .into()
    }
}

fn edit_input_panel(_form_ctx: &FormContext) -> Html {
    InputPanel::new()
        .padding(4)
        .with_field(
            tr!("User"),
            Field::new()
                .name("userid")
                .required(true)
                .disabled(true)
                .submit(false),
        )
        .with_right_field(
            tr!("Expire"),
            Field::new()
                .name("expire")
                .placeholder(tr!("never"))
                .input_type(InputType::DatetimeLocal),
        )
        .with_field(
            tr!("Token Name"),
            Field::new()
                .name("tokenname")
                .submit(false)
                .disabled(true)
                .required(true),
        )
        .with_right_field(tr!("Enabled"), Checkbox::new().name("enable").default(true))
        .with_large_field(
            tr!("Comment"),
            Field::new().name("comment").submit_empty(true),
        )
        .into()
}

fn add_input_panel(_form_ctx: &FormContext) -> Html {
    InputPanel::new()
        .padding(4)
        .with_field(
            tr!("User"),
            AuthidSelector::new()
                .name("userid")
                .required(true)
                .submit(false)
                .include_tokens(false),
        )
        .with_right_field(
            tr!("Expire"),
            Field::new()
                .name("expire")
                .placeholder(tr!("never"))
                .input_type(InputType::DatetimeLocal),
        )
        .with_field(
            tr!("Token Name"),
            Field::new().name("tokenname").submit(false).required(true),
        )
        .with_right_field(tr!("Enabled"), Checkbox::new().name("enable").default(true))
        .with_large_field(tr!("Comment"), Field::new().name("comment"))
        .into()
}

fn columns() -> Rc<Vec<DataTableHeader<ApiToken>>> {
    Rc::new(vec![
        DataTableColumn::new(tr!("User"))
            .width("200px")
            .render(|item: &ApiToken| {
                html! {&item.tokenid.user()}
            })
            .sorter(|a: &ApiToken, b: &ApiToken| a.tokenid.user().cmp(b.tokenid.user()))
            .sort_order(true)
            .into(),
        DataTableColumn::new(tr!("Token name"))
            .width("100px")
            .render(|item: &ApiToken| {
                let name = item
                    .tokenid
                    .tokenname()
                    .map(|name| name.as_str())
                    .unwrap_or("");
                html! {name}
            })
            .sorter(|a: &ApiToken, b: &ApiToken| {
                let a = a
                    .tokenid
                    .tokenname()
                    .map(|name| name.as_str())
                    .unwrap_or("");
                let b = b
                    .tokenid
                    .tokenname()
                    .map(|name| name.as_str())
                    .unwrap_or("");
                a.cmp(b)
            })
            .sort_order(true)
            .into(),
        DataTableColumn::new(tr!("Enable"))
            .width("80px")
            .render(|item: &ApiToken| {
                html! {render_boolean(item.enable.unwrap_or(true))}
            })
            .sorter(|a: &ApiToken, b: &ApiToken| a.enable.cmp(&b.enable))
            .into(),
        DataTableColumn::new(tr!("Expire"))
            .width("80px")
            .render({
                let never_text = tr!("never");
                move |item: &ApiToken| {
                    html! {
                        {
                            match item.expire {
                                Some(epoch) if epoch != 0 => render_epoch_short(epoch),
                                _ => never_text.clone(),
                            }
                        }
                    }
                }
            })
            .sorter(|a: &ApiToken, b: &ApiToken| {
                let a = if let Some(0) = a.expire {
                    None
                } else {
                    a.expire
                };
                let b = if let Some(0) = b.expire {
                    None
                } else {
                    b.expire
                };
                a.cmp(&b)
            })
            .into(),
        DataTableColumn::new("Comment")
            .flex(1)
            .render(|item: &ApiToken| item.comment.as_deref().unwrap_or_default().into())
            .into(),
    ])
}

impl From<TokenPanel> for VNode {
    fn from(value: TokenPanel) -> Self {
        VComp::new::<LoadableComponentMaster<ProxmoxTokenView>>(Rc::new(value), None).into()
    }
}
