use std::rc::Rc;

use serde_json::Value;

use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoOptionalInlineHtml, IntoSubmitCallback, SubmitCallback};
use pwt::widget::{ActionIcon, Fa, List, ListTile, Row};

use crate::{ApiLoadCallback, IntoApiLoadCallback};

use pwt_macros::builder;

use crate::layout::list_tile::title_subtitle_column;
use crate::EditableProperty;

use super::{
    PendingPropertyView, PendingPropertyViewMsg, PendingPropertyViewState, PvePendingConfiguration,
    PvePendingPropertyView,
};

/// Render a list of pending changes ([PvePendingConfiguration])
#[derive(Properties, Clone, PartialEq)]
#[builder]
pub struct PendingPropertyList {
    /// CSS class
    #[prop_or_default]
    pub class: Classes,

    /// List of property definitions
    pub properties: Rc<Vec<EditableProperty>>,

    /// Load property list with pending changes information.
    #[builder_cb(IntoApiLoadCallback, into_api_load_callback, PvePendingConfiguration)]
    #[prop_or_default]
    pub pending_loader: Option<ApiLoadCallback<PvePendingConfiguration>>,

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
        property: &EditableProperty,
    ) -> ListTile {
        let on_revert = Callback::from({
            ctx.link().callback({
                let property = property.clone();
                move |_: Event| PendingPropertyViewMsg::RevertProperty(property.clone())
            })
        });

        let list_tile =
            PendingPropertyList::render_list_tile(current, pending, property, (), on_revert);

        if property.render_input_panel.is_some() {
            list_tile
                .interactive(true)
                .on_activate(ctx.link().callback({
                    let property = property.clone();
                    move |_| PendingPropertyViewMsg::EditProperty(property.clone(), None)
                }))
        } else {
            list_tile
        }
    }
}

impl PendingPropertyView for PvePendingPropertyList {
    type Properties = PendingPropertyList;
    type Message = ();

    const MOBILE: bool = true;

    fn editor_loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>> {
        props.editor_loader.clone()
    }

    fn pending_loader(
        props: &Self::Properties,
    ) -> Option<ApiLoadCallback<PvePendingConfiguration>> {
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
        view_state: &PendingPropertyViewState,
    ) -> Html {
        let props = ctx.props();

        let mut tiles: Vec<ListTile> = Vec::new();

        let PvePendingConfiguration {
            current,
            pending,
            keys,
        } = match &view_state.data {
            Some(data) => data,
            _ => &PvePendingConfiguration::new(),
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
                let mut tile = self.property_tile(ctx, &current, &pending, item);
                tile.set_key(name);
                tiles.push(tile);
            }
        }

        let panel = List::from_tiles(tiles)
            .virtual_scroll(Some(false))
            .grid_template_columns("1fr")
            .class(pwt::css::FlexFit)
            .into();

        let loading = view_state.loading();

        let class = props.class.clone();
        let dialog = view_state.dialog.clone();
        let error = view_state.error.clone();

        crate::property_view::render_loadable_panel(class, panel, None, dialog, loading, error)
    }
}

impl From<PendingPropertyList> for VNode {
    fn from(props: PendingPropertyList) -> Self {
        let comp =
            VComp::new::<PvePendingPropertyView<PvePendingPropertyList>>(Rc::new(props), None);
        VNode::from(comp)
    }
}
