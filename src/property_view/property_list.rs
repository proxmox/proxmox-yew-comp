use std::rc::Rc;

use serde_json::Value;

use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoSubmitCallback, SubmitCallback};
use pwt::widget::{List, ListTile};

use crate::{ApiLoadCallback, IntoApiLoadCallback};

use pwt_macros::builder;

use crate::layout::list_tile::form_list_tile;
use crate::EditableProperty;

use super::{PropertyView, PropertyViewMsg, PropertyViewState, PvePropertyView};

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
        property: &EditableProperty,
    ) -> ListTile {
        let value_text = super::render_property_value(record, property);
        let list_tile = form_list_tile(property.title.clone(), value_text, ());

        if property.render_input_panel.is_some() {
            list_tile
                .interactive(true)
                .on_activate(ctx.link().callback({
                    let property = property.clone();
                    move |_| PropertyViewMsg::EditProperty(property.clone())
                }))
        } else {
            list_tile
        }
    }
}

impl PropertyView for PvePropertyList {
    type Properties = PropertyList;
    type Message = ();
    const MOBILE: bool = true;

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

    fn view(&self, ctx: &Context<PvePropertyView<Self>>, view_state: &PropertyViewState) -> Html {
        let props = ctx.props();

        let mut tiles: Vec<ListTile> = Vec::new();

        let record = match &view_state.data {
            Some(data) => data.clone(),
            _ => Value::Null,
        };

        for item in props.properties.iter() {
            let name = match item.get_name() {
                Some(name) => name.clone(),
                None::<_> => {
                    log::error!("property list: skiping property without name");
                    continue;
                }
            };
            let value = record.get(&*name);
            if !item.required && (value.is_none() || value == Some(&Value::Null)) {
                continue;
            }

            let mut list_tile = self.property_tile(ctx, &record, item);
            list_tile.set_key(name);

            tiles.push(list_tile);
        }

        let panel = List::from_tiles(tiles)
            .virtual_scroll(Some(false))
            .grid_template_columns("1fr auto")
            .class(pwt::css::FlexFit)
            .into();

        let loading = view_state.loading();

        let class = props.class.clone();
        let dialog = view_state.dialog.clone();
        let error = view_state.error.clone();

        super::render_loadable_panel(class, panel, None, dialog, loading, error)
    }
}

impl From<PropertyList> for VNode {
    fn from(props: PropertyList) -> Self {
        let comp = VComp::new::<PvePropertyView<PvePropertyList>>(Rc::new(props), None);
        VNode::from(comp)
    }
}
