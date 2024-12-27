use std::rc::Rc;

use anyhow::{format_err, Error};
use serde::{Deserialize, Serialize};

use yew::html::IntoPropValue;
use yew::virtual_dom::Key;

use proxmox_auth_api::types::{Authid, Userid};

use pwt::prelude::*;
use pwt::props::{FieldBuilder, RenderFn, WidgetBuilder};
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::form::{Selector, SelectorRenderArgs, ValidateFn};
use pwt::widget::GridPicker;

use pwt_macros::{builder, widget};

#[derive(Clone, PartialEq)]
struct AuthidListEntry {
    authid: Authid,
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
            list.push(AuthidListEntry {
                authid: Authid::from(user.userid),
                comment: user.comment,
            });
        }
        if include_tokens {
            for token in user.tokens.into_iter() {
                //if token.is_active() {
                list.push(AuthidListEntry {
                    authid: token.tokenid,
                    comment: token.comment,
                });
                // }
            }
        }
    }

    Ok(list)
}

thread_local! {
    static COLUMNS: Rc<Vec<DataTableHeader<AuthidListEntry>>> = Rc::new(vec![
        DataTableColumn::new("Type")
            .width("100px")
            .show_menu(false)
            .render(|item: &AuthidListEntry| {
                if item.authid.is_token() { html!{"API Token"} } else { html!{"User"}}
            })
            .sorter(|a: &AuthidListEntry, b: &AuthidListEntry| {
                a.authid.is_token().cmp(&b.authid.is_token())
            })
            .sort_order(true)
            .into(),
        DataTableColumn::new("Auth ID")
            .width("100px")
            .show_menu(false)
            .render(|item: &AuthidListEntry| html!{item.authid.clone()})
            .sorter(|a: &AuthidListEntry, b: &AuthidListEntry| {
                a.authid.cmp(&b.authid)
            })
            .sort_order(true)
            .into(),
        DataTableColumn::new("Comment")
            .width("300px")
            .show_menu(false)
            .render(|item: &AuthidListEntry| {
                html!{item.comment.clone().unwrap_or_default()}
            })
            .into(),
    ]);
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

        let picker = RenderFn::new(|args: &SelectorRenderArgs<Store<AuthidListEntry>>| {
            let table =
                DataTable::new(COLUMNS.with(Rc::clone), args.store.clone()).class("pwt-fit");

            GridPicker::new(table)
                .selection(args.selection.clone())
                .on_select(args.controller.on_select_callback())
                .into()
        });

        Self {
            store,
            validate,
            picker,
        }
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
