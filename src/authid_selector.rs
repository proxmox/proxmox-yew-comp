use std::rc::Rc;

use anyhow::{format_err, Error};
use serde::{Deserialize, Serialize};

use yew::html::IntoPropValue;
use yew::virtual_dom::Key;

use proxmox_auth_api::types::{Authid, Userid};

use pwt::prelude::*;
use pwt::props::{CssLength, FieldBuilder, RenderFn, WidgetBuilder, WidgetStyleBuilder};
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::form::{Selector, SelectorRenderArgs, ValidateFn};
use pwt::widget::GridPicker;

use pwt_macros::{builder, widget};

#[derive(Clone, PartialEq)]
struct AuthidListEntry {
    authid: Authid,
    /// The user's first and last name, empty for API tokens.
    name: String,
    comment: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ApiToken {
    pub tokenid: Authid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct UserWithTokens {
    pub userid: Userid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firstname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastname: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tokens: Vec<ApiToken>,
}

async fn load_users(
    url: AttrValue,
    include_users: bool,
    include_tokens: bool,
) -> Result<Vec<AuthidListEntry>, Error> {
    let users: Vec<UserWithTokens> = crate::http_get(&*url, None).await?;
    let mut list: Vec<AuthidListEntry> = Vec::new();

    // fixme: only active users, needs User::is_active()

    for user in users.into_iter() {
        if include_users {
            let name = [user.firstname, user.lastname]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            list.push(AuthidListEntry {
                authid: Authid::from(user.userid),
                name,
                comment: user.comment,
            });
        }
        if include_tokens {
            for token in user.tokens.into_iter() {
                //if token.is_active() {
                list.push(AuthidListEntry {
                    authid: token.tokenid,
                    name: String::new(),
                    comment: token.comment,
                });
                // }
            }
        }
    }

    Ok(list)
}

/// The picker's columns. The type column only tells users and tokens apart, so it is left out when
/// the list holds just one kind, and only users have a name.
fn columns(users: bool, tokens: bool) -> Rc<Vec<DataTableHeader<AuthidListEntry>>> {
    let mut columns = Vec::new();
    if users && tokens {
        columns.push(
            DataTableColumn::new("Type")
                .width("100px")
                .show_menu(false)
                .render(|item: &AuthidListEntry| {
                    if item.authid.is_token() {
                        html! {"API Token"}
                    } else {
                        html! {"User"}
                    }
                })
                .sorter(|a: &AuthidListEntry, b: &AuthidListEntry| {
                    a.authid.is_token().cmp(&b.authid.is_token())
                })
                .sort_order(true)
                .into(),
        );
    }
    columns.push(
        DataTableColumn::new("Auth ID")
            .flex(2)
            .show_menu(false)
            .render(|item: &AuthidListEntry| html! {item.authid.clone()})
            .sorter(|a: &AuthidListEntry, b: &AuthidListEntry| a.authid.cmp(&b.authid))
            .sort_order(true)
            .into(),
    );
    if users {
        columns.push(
            DataTableColumn::new("Name")
                .flex(2)
                .show_menu(false)
                .render(|item: &AuthidListEntry| html! {item.name.clone()})
                .sorter(|a: &AuthidListEntry, b: &AuthidListEntry| a.name.cmp(&b.name))
                .into(),
        );
    }
    columns.push(
        DataTableColumn::new("Comment")
            .flex(1)
            .show_menu(false)
            .render(|item: &AuthidListEntry| html! {item.comment.clone().unwrap_or_default()})
            .into(),
    );
    Rc::new(columns)
}

fn picker(users: bool, tokens: bool) -> RenderFn<SelectorRenderArgs<Store<AuthidListEntry>>> {
    let columns = columns(users, tokens);
    RenderFn::new(move |args: &SelectorRenderArgs<Store<AuthidListEntry>>| {
        // Wider than the field, so IDs and names fit, but never wider than the window.
        let table = DataTable::new(columns.clone(), args.store.clone())
            .class("pwt-flex-fit")
            .width(640)
            .max_width(CssLength::Fraction(1.0));

        GridPicker::new(table)
            .filter(|item: &AuthidListEntry, query: &str| {
                let query = query.to_lowercase();
                item.authid.to_string().to_lowercase().contains(&query)
                    || item.name.to_lowercase().contains(&query)
            })
            .selection(args.selection.clone())
            .on_select(args.controller.on_select_callback())
            .into()
    })
}

#[widget(comp=PbsAuthidSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct AuthidSelector {
    /// The default value.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default: Option<AttrValue>,

    /// Include API Tokens.
    #[prop_or(true)]
    #[builder]
    pub include_tokens: bool,

    // Include normal Users.
    #[prop_or(true)]
    #[builder]
    pub include_users: bool,
}

impl Default for AuthidSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthidSelector {
    /// Create a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct PbsAuthidSelector {
    store: Store<AuthidListEntry>,
    validate: ValidateFn<(String, Store<AuthidListEntry>)>,
    picker: RenderFn<SelectorRenderArgs<Store<AuthidListEntry>>>,
}
impl Component for PbsAuthidSelector {
    type Message = ();
    type Properties = AuthidSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let store =
            Store::with_extract_key(|item: &AuthidListEntry| Key::from(item.authid.to_string()))
                .on_change(ctx.link().callback(|_| ())); // trigger redraw

        let validate = ValidateFn::new(|(authid, store): &(String, Store<AuthidListEntry>)| {
            store
                .read()
                .data()
                .iter()
                .find(|item| &item.authid.to_string() == authid)
                .map(drop)
                .ok_or_else(|| format_err!("no such user"))
        });

        let props = ctx.props();
        let picker = picker(props.include_users, props.include_tokens);

        Self {
            store,
            validate,
            picker,
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if props.include_users != old_props.include_users
            || props.include_tokens != old_props.include_tokens
        {
            self.picker = picker(props.include_users, props.include_tokens);
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let url = format!(
            "/access/users/?include_tokens={}",
            if props.include_tokens { 1 } else { 0 },
        );

        let include_users = props.include_users;
        let include_tokens = props.include_tokens;

        Selector::new(self.store.clone(), self.picker.clone())
            .with_std_props(&props.std_props)
            .with_input_props(&props.input_props)
            .default(&props.default)
            .loader((
                move |url| load_users(url, include_users, include_tokens),
                url,
            ))
            .validate(self.validate.clone())
            .into()
    }
}
