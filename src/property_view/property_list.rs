use std::rc::Rc;

use serde_json::Value;

use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoSubmitCallback, SubmitCallback};
use pwt::widget::{List, ListTile};

use crate::{ApiLoadCallback, IntoApiLoadCallback};

use pwt_macros::builder;

use crate::layout::list_tile::form_list_tile;
use crate::EditableProperty;

use super::{PropertyView, PropertyViewMsg, PvePropertyView};

/// Render object properties as [List]
#[derive(Properties, Clone, PartialEq)]
#[builder]
pub struct PropertyList {
    /// CSS class
    #[prop_or_default]
    pub class: Classes,

    /// List of property definitions
    pub properties: Rc<Vec<EditableProperty>>,

    /// Data loader.
    #[builder_cb(IntoApiLoadCallback, into_api_load_callback, Value)]
    #[prop_or_default]
    pub loader: Option<ApiLoadCallback<Value>>,

    /// Submit callback.
    #[builder_cb(IntoSubmitCallback, into_submit_callback, Value)]
    #[prop_or_default]
    pub on_submit: Option<SubmitCallback<Value>>,
}

impl PropertyList {
    pub fn new(properties: Rc<Vec<EditableProperty>>) -> Self {
        yew::props!(Self { properties })
    }

    pwt::impl_class_prop_builder!();
}

pub struct PvePropertyList {}

impl PvePropertyList {
    fn property_tile(
        &self,
        ctx: &Context<PvePropertyView<Self>>,
        record: &Value,
        name: Key,
        property: &EditableProperty,
    ) -> ListTile {
        let value_text = super::render_property_value(record, property);
        let list_tile = form_list_tile(property.title.clone(), value_text, ());

        if property.render_input_panel.is_some() {
            list_tile.interactive(true).on_activate(
                ctx.link()
                    .callback(move |_| PropertyViewMsg::EditProperty(name.clone())),
            )
        } else {
            list_tile
        }
    }
}

impl PropertyView for PvePropertyList {
    type Properties = PropertyList;
    const MOBILE: bool = true;

    fn class(props: &Self::Properties) -> &Classes {
        &props.class
    }

    fn properties(props: &Self::Properties) -> &Rc<Vec<EditableProperty>> {
        &props.properties
    }

    fn loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>> {
        props.loader.clone()
    }

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>> {
        props.on_submit.clone()
    }

    fn create(_ctx: &Context<PvePropertyView<Self>>) -> Self
    where
        Self: 'static + Sized,
    {
        Self {}
    }

    fn update_data(
        &mut self,
        _ctx: &Context<PvePropertyView<Self>>,
        _data: Option<&Value>,
        _error: Option<&str>,
    ) where
        Self: 'static + Sized,
    {
        /* do nothing */
    }

    fn view(
        &self,
        ctx: &Context<PvePropertyView<Self>>,
        data: Option<&Value>,
        _error: Option<&str>,
    ) -> Html {
        let props = ctx.props();

        let mut tiles: Vec<ListTile> = Vec::new();

        let record = match data {
            Some(data) => data.clone(),
            _ => Value::Null,
        };

        for item in props.properties.iter() {
            let name = match item.get_name() {
                Some(name) => name.clone(),
                None => {
                    log::error!("property list: skiping property without name");
                    continue;
                }
            };
            let value = record.get(&*name);
            if !item.required && (value.is_none() || value == Some(&Value::Null)) {
                continue;
            }

            let mut list_tile = self.property_tile(ctx, &record, Key::from(&*name), item);
            list_tile.set_key(name);

            tiles.push(list_tile);
        }
        List::from_tiles(tiles)
            .virtual_scroll(Some(false))
            .grid_template_columns("1fr auto")
            .class(pwt::css::FlexFit)
            .into()
    }
}

impl From<PropertyList> for VNode {
    fn from(props: PropertyList) -> Self {
        let comp = VComp::new::<PvePropertyView<PvePropertyList>>(Rc::new(props), None);
        VNode::from(comp)
    }
}
