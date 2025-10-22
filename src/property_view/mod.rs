mod property_grid;
pub use property_grid::{property_grid_columns, PropertyGrid};

mod property_list;
pub use property_list::PropertyList;

use std::rc::Rc;

use gloo_timers::callback::Timeout;
use serde_json::Value;

use yew::virtual_dom::Key;

use pwt::prelude::*;
use pwt::props::{ExtractPrimaryKey, SubmitCallback};
use pwt::widget::{Column, Container};
use pwt::AsyncAbortGuard;

use crate::utils::render_boolean;
use crate::{ApiLoadCallback, EditableProperty, PropertyEditDialog};

#[derive(Clone, PartialEq)]
pub struct PropertyGridRecord {
    pub key: Key,
    pub header: Html,
    pub content: Html,
    pub has_changes: bool,
}

impl ExtractPrimaryKey for PropertyGridRecord {
    fn extract_key(&self) -> Key {
        Key::from(self.key.clone())
    }
}

pub enum PropertyViewMsg {
    Load,
    LoadResult(Result<Value, String>),
    ShowDialog(Option<Html>),
    EditProperty(Key),
    Select(Option<Key>),
}

pub trait PropertyView {
    type Properties: Properties;
    const MOBILE: bool;

    fn class(props: &Self::Properties) -> &Classes;

    fn properties(props: &Self::Properties) -> &Rc<Vec<EditableProperty>>;

    fn loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>>;

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>>;

    fn create(ctx: &Context<PvePropertyView<Self>>) -> Self
    where
        Self: 'static + Sized;

    fn update_data(
        &mut self,
        _ctx: &Context<PvePropertyView<Self>>,
        _data: Option<&Value>,
        _error: Option<&str>,
    ) where
        Self: 'static + Sized,
    {
    }

    fn toolbar(
        &self,
        _ctx: &Context<PvePropertyView<Self>>,
        _data: Option<&Value>,
        _error: Option<&str>,
    ) -> Option<Html>
    where
        Self: 'static + Sized,
    {
        None
    }

    fn view(
        &self,
        ctx: &Context<PvePropertyView<Self>>,
        data: Option<&Value>,
        error: Option<&str>,
    ) -> Html
    where
        Self: 'static + Sized;
}

pub struct PvePropertyView<T> {
    data: Option<Value>,
    error: Option<String>,
    reload_timeout: Option<Timeout>,
    load_guard: Option<AsyncAbortGuard>,
    dialog: Option<Html>,
    view_state: T,
}

impl<T: 'static + PropertyView> Component for PvePropertyView<T> {
    type Message = PropertyViewMsg;
    type Properties = T::Properties;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(PropertyViewMsg::Load);

        let mut me = Self {
            data: None,
            error: None,
            reload_timeout: None,
            load_guard: None,
            dialog: None,
            view_state: T::create(ctx),
        };
        me.view_state.update_data(ctx, None, None);
        me
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            PropertyViewMsg::Select(_key) => { /* just redraw */ }
            PropertyViewMsg::EditProperty(key) => {
                let property = match lookup_property(T::properties(props), &key) {
                    Some(property) => property,
                    None::<_> => return false,
                };

                let dialog = PropertyEditDialog::from(property.clone())
                    .mobile(T::MOBILE)
                    .on_done(ctx.link().callback(|_| PropertyViewMsg::ShowDialog(None)))
                    .loader(T::loader(props))
                    .on_submit(T::on_submit(props))
                    .into();
                self.dialog = Some(dialog);
            }
            PropertyViewMsg::Load => {
                self.reload_timeout = None;
                let link = ctx.link().clone();
                if let Some(loader) = T::loader(props) {
                    self.load_guard = Some(AsyncAbortGuard::spawn(async move {
                        let result = loader.apply().await;
                        let data = match result {
                            Ok(result) => Ok(result.data),
                            Err(err) => Err(err.to_string()),
                        };
                        link.send_message(PropertyViewMsg::LoadResult(data));
                    }));
                }
            }
            PropertyViewMsg::LoadResult(result) => {
                match result {
                    Ok(data) => {
                        self.data = Some(data);
                        self.error = None;
                    }
                    Err(err) => self.error = Some(err),
                }
                self.view_state
                    .update_data(ctx, self.data.as_ref(), self.error.as_deref());
                let link = ctx.link().clone();
                self.reload_timeout = Some(Timeout::new(3000, move || {
                    link.send_message(PropertyViewMsg::Load);
                }));
            }
            PropertyViewMsg::ShowDialog(dialog) => {
                if dialog.is_none() && self.reload_timeout.is_some() {
                    ctx.link().send_message(PropertyViewMsg::Load);
                }
                self.dialog = dialog;
            }
        }
        true
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if T::properties(props) != T::properties(old_props) {
            self.view_state
                .update_data(ctx, self.data.as_ref(), self.error.as_deref());
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let panel = self
            .view_state
            .view(ctx, self.data.as_ref(), self.error.as_deref());

        let loading = self.data.is_none() && self.error.is_none();

        Column::new()
            .class(T::class(props).clone())
            .with_optional_child(self.view_state.toolbar(
                ctx,
                self.data.as_ref(),
                self.error.as_deref(),
            ))
            .with_optional_child(
                loading.then(|| pwt::widget::Progress::new().class("pwt-delay-visibility")),
            )
            .with_child(panel)
            .with_optional_child(
                self.error
                    .as_deref()
                    .map(|err| pwt::widget::error_message(&err.to_string()).padding(2)),
            )
            .with_optional_child(self.dialog.clone())
            .into()
    }
}

fn lookup_property<'a>(
    properties: &'a [EditableProperty],
    key: &Key,
) -> Option<&'a EditableProperty> {
    let property_name: AttrValue = key.to_string().into();
    properties
        .iter()
        .find(|p| p.get_name() == Some(&property_name))
}

pub fn render_property_value(record: &Value, property: &EditableProperty) -> Html {
    let (render_name, value);

    if let Some(name) = property.get_name() {
        value = record.get(name.as_str());
        render_name = name.to_string();
    } else {
        // simply pass empty string as property name to the renderer
        render_name = String::new();
        value = None;
    }

    match (value, &property.renderer) {
        (None::<_> | Some(Value::Null), _) => {
            let placeholder = if let Some(placeholder) = &property.placeholder {
                placeholder.to_string().into()
            } else {
                String::from("-")
            };
            Container::new()
                .class(pwt::css::Opacity::Half)
                .with_child(placeholder)
                .into()
        }

        (Some(value), None::<_>) => match value {
            Value::String(value) => value.clone(),
            Value::Bool(value) => render_boolean(*value),
            Value::Number(n) => n.to_string(),
            v => v.to_string(),
        }
        .into(),
        (Some(value), Some(renderer)) => renderer.apply(&render_name, &value, &record),
    }
}
