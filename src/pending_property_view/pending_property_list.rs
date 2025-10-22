use std::collections::HashSet;
use std::rc::Rc;

use serde_json::Value;

use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoOptionalInlineHtml, IntoSubmitCallback, SubmitCallback};
use pwt::widget::{ActionIcon, Fa, List, ListTile, Row};

use crate::{ApiLoadCallback, IntoApiLoadCallback};

use pwt_macros::builder;

use crate::layout::list_tile::title_subtitle_column;
use crate::pve_api_types::QemuPendingConfigValue;
use crate::EditableProperty;

use super::{PendingPropertyView, PendingPropertyViewMsg, PvePendingPropertyView};

/// Render a list of pending changes ([`Vec<QemuPendingConfigValue>`])
#[derive(Properties, Clone, PartialEq)]
#[builder]
pub struct PendingPropertyList {
    /// CSS class
    #[prop_or_default]
    pub class: Classes,

    /// List of property definitions
    pub properties: Rc<Vec<EditableProperty>>,

    /// Load property list with pending changes information.
    #[builder_cb(IntoApiLoadCallback, into_api_load_callback, Vec<QemuPendingConfigValue>)]
    #[prop_or_default]
    pub pending_loader: Option<ApiLoadCallback<Vec<QemuPendingConfigValue>>>,

    /// Loader passed to the EditDialog
    #[builder_cb(IntoApiLoadCallback, into_api_load_callback, Value)]
    #[prop_or_default]
    pub editor_loader: Option<ApiLoadCallback<Value>>,

    /// Submit callback.
    #[builder_cb(IntoSubmitCallback, into_submit_callback, Value)]
    #[prop_or_default]
    pub on_submit: Option<SubmitCallback<Value>>,
}

impl PendingPropertyList {
    pub fn new(properties: Rc<Vec<EditableProperty>>) -> Self {
        yew::props!(Self { properties })
    }

    pwt::impl_class_prop_builder!();

    /// Render a ListTile with a single child.
    ///
    /// Suitable for a "grid-template-columns: 1fr".
    pub fn render_list_tile(
        current: &Value,
        pending: &Value,
        property: &EditableProperty,
        trailing: impl IntoOptionalInlineHtml,
        on_revert: Callback<Event>,
    ) -> ListTile {
        Self::render_list_tile_internal(current, pending, property, None, trailing, on_revert)
    }

    /// Render a ListTile with a two children, icon + rest.
    ///
    /// Suitable for a "grid-template-columns: "auto 1fr".
    pub fn render_icon_list_tile(
        current: &Value,
        pending: &Value,
        property: &EditableProperty,
        icon: Fa,
        trailing: impl IntoOptionalInlineHtml,
        on_revert: Callback<Event>,
    ) -> ListTile {
        Self::render_list_tile_internal(current, pending, property, Some(icon), trailing, on_revert)
    }

    // Note: We do not use 3 columns so that we do not waste space on the right side.
    fn render_list_tile_internal(
        current: &Value,
        pending: &Value,
        property: &EditableProperty,
        icon: Option<Fa>,
        trailing: impl IntoOptionalInlineHtml,
        on_revert: Callback<Event>,
    ) -> ListTile {
        let (value, new_value) =
            crate::pending_property_view::render_pending_property_value(current, pending, property);

        let revert: Html = ActionIcon::new("fa fa-undo")
            .on_activate(on_revert.clone())
            .into();

        if let Some(new_value) = new_value {
            let subtitle = html! {<><div>{value}</div><div style="line-height: 1.4em;" class="pwt-color-warning">{new_value}</div></>};
            let content: Html = Row::new()
                .class(pwt::css::AlignItems::Center)
                .class(pwt::css::JustifyContent::End)
                .gap(1)
                .with_child(title_subtitle_column(property.title.clone(), subtitle))
                .with_flex_spacer()
                .with_child(revert)
                .with_optional_child(trailing.into_optional_inline_html())
                .into();

            ListTile::new()
                .class(pwt::css::AlignItems::Center)
                .class("pwt-gap-2")
                .border_bottom(true)
                .with_optional_child(icon.map(|i| i.fixed_width().large_2x()))
                .with_child(content)
        } else {
            let trailing = trailing.into_optional_inline_html();
            let content: Html = Row::new()
                .class(pwt::css::AlignItems::Center)
                .class(pwt::css::JustifyContent::End)
                .gap(1)
                .with_child(title_subtitle_column(property.title.clone(), value))
                .with_flex_spacer()
                .with_optional_child(trailing.into_optional_inline_html())
                .into();

            ListTile::new()
                .class(pwt::css::AlignItems::Center)
                .class("pwt-gap-2")
                .border_bottom(true)
                .with_optional_child(icon.map(|i| i.fixed_width().large_2x()))
                .with_child(content)
        }
    }
}

pub struct PvePendingPropertyList {}

impl PvePendingPropertyList {
    fn property_tile(
        &self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        current: &Value,
        pending: &Value,
        name: Key,
        property: &EditableProperty,
    ) -> ListTile {
        let on_revert = Callback::from({
            ctx.link().callback({
                let name = name.clone();
                move |_: Event| PendingPropertyViewMsg::Revert(name.clone())
            })
        });

        let list_tile =
            PendingPropertyList::render_list_tile(current, pending, property, (), on_revert);

        if property.render_input_panel.is_some() {
            list_tile.interactive(true).on_activate(
                ctx.link()
                    .callback(move |_| PendingPropertyViewMsg::Edit(name.clone())),
            )
        } else {
            list_tile
        }
    }
}

impl PendingPropertyView for PvePendingPropertyList {
    type Properties = PendingPropertyList;
    type Message = ();

    const MOBILE: bool = true;

    fn class(props: &Self::Properties) -> &Classes {
        &props.class
    }

    fn properties(props: &Self::Properties) -> &Rc<Vec<EditableProperty>> {
        &props.properties
    }

    fn editor_loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>> {
        props.editor_loader.clone()
    }

    fn pending_loader(
        props: &Self::Properties,
    ) -> Option<ApiLoadCallback<Vec<QemuPendingConfigValue>>> {
        props.pending_loader.clone()
    }

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>> {
        props.on_submit.clone()
    }

    fn create(_ctx: &Context<PvePendingPropertyView<Self>>) -> Self {
        Self {}
    }

    fn view(
        &self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        data: Option<&(Value, Value, HashSet<String>)>,
        _error: Option<&str>,
    ) -> Html {
        let props = ctx.props();

        let mut tiles: Vec<ListTile> = Vec::new();

        let (current, pending, keys): (Value, Value, HashSet<String>) = match data {
            Some(data) => data.clone(),
            _ => (Value::Null, Value::Null, HashSet::new()),
        };

        for item in props.properties.iter() {
            let name = match item.get_name() {
                Some(name) => name.to_string(),
                None::<_> => {
                    log::error!("pending property list: skiping property without name");
                    continue;
                }
            };
            if item.required || keys.contains(&name) {
                let mut tile = self.property_tile(ctx, &current, &pending, Key::from(&*name), item);
                tile.set_key(name);
                tiles.push(tile);
            }
        }

        List::from_tiles(tiles)
            .virtual_scroll(Some(false))
            .grid_template_columns("1fr")
            .class(pwt::css::FlexFit)
            .into()
    }
}

/*
impl Component for PvePendingPropertyList {
    type Message = Msg;
    type Properties = PendingPropertyList;


    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::Revert(property) => {
                let link = ctx.link().clone();
                let keys = match property.revert_keys.as_deref() {
                    Some(keys) => keys.iter().map(|a| a.to_string()).collect(),
                    None::<_> => {
                        if let Some(name) = property.get_name() {
                            vec![name.to_string()]
                        } else {
                            log::error!(
                                "pending property list: cannot revert property without name",
                            );
                            return false;
                        }
                    }
                };
                if let Some(on_submit) = props.on_submit.clone() {
                    let param = json!({ "revert": keys });
                    self.revert_guard = Some(AsyncAbortGuard::spawn(async move {
                        let result = on_submit.apply(param).await;
                        link.send_message(Msg::RevertResult(result));
                    }));
                }
            }
            Msg::RevertResult(result) => {
                if let Err(err) = result {
                    ctx.link().show_snackbar(
                        SnackBar::new()
                            .message(tr!("Revert property failed") + " - " + &err.to_string()),
                    );
                }
                if self.reload_timeout.is_some() {
                    ctx.link().send_message(Msg::Load);
                }
            }
            Msg::EditProperty(property) => {
                let dialog = PropertyEditDialog::from(property.clone())
                    .mobile(true)
                    .on_done(ctx.link().callback(|_| Msg::ShowDialog(None)))
                    .loader(props.editor_loader.clone())
                    .on_submit(props.on_submit.clone())
                    .into();
                self.dialog = Some(dialog);
            }
            Msg::Load => {
                self.reload_timeout = None;
                let link = ctx.link().clone();
                if let Some(loader) = props.pending_loader.clone() {
                    self.load_guard = Some(AsyncAbortGuard::spawn(async move {
                        let result = loader.apply().await;
                        let data = match result {
                            Ok(result) => Ok(result.data),
                            Err(err) => Err(err.to_string()),
                        };
                        link.send_message(Msg::LoadResult(data));
                    }));
                }
            }
            Msg::LoadResult(result) => {
                let result = result.and_then(|data| {
                    PendingPropertyList::pve_pending_config_array_to_objects(data)
                        .map_err(|err| err.to_string())
                });
                match result {
                    Ok(data) => {
                        self.data = Some(data);
                        self.error = None;
                    }
                    Err(err) => self.error = Some(err),
                }
                let link = ctx.link().clone();
                self.reload_timeout = Some(Timeout::new(3000, move || {
                    link.send_message(Msg::Load);
                }));
            }
            Msg::ShowDialog(dialog) => {
                if dialog.is_none() && self.reload_timeout.is_some() {
                    ctx.link().send_message(Msg::Load);
                }
                self.dialog = dialog;
            }
        }
        true
    }

}
*/

impl From<PendingPropertyList> for VNode {
    fn from(props: PendingPropertyList) -> Self {
        let comp =
            VComp::new::<PvePendingPropertyView<PvePendingPropertyList>>(Rc::new(props), None);
        VNode::from(comp)
    }
}
