use std::ops::DerefMut;

mod property_grid;
pub use property_grid::{property_grid_columns, PropertyGrid};

mod property_list;
pub use property_list::PropertyList;

use gloo_timers::callback::Timeout;
use serde_json::Value;

use yew::html::Scope;
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
    pub property: EditableProperty,
    pub header: Html,
    pub content: Html,
    pub has_changes: bool,
}

impl ExtractPrimaryKey for PropertyGridRecord {
    fn extract_key(&self) -> Key {
        Key::from(self.key.clone())
    }
}

pub enum PropertyViewMsg<M> {
    Load,
    LoadResult(Result<Value, String>),
    ShowDialog(Option<Html>),
    EditProperty(EditableProperty),
    Select(Option<Key>),
    Custom(M),
}

pub trait PropertyViewScopeExt<M> {
    fn send_custom_message(&self, msg: M);
    fn send_reload(&self);
    fn send_show_dialog(&self, dialog: Option<Html>);
    fn send_edit_property(&self, property: EditableProperty);
}

impl<M, T: 'static + PropertyView<Message = M>> PropertyViewScopeExt<M>
    for Scope<PvePropertyView<T>>
{
    fn send_custom_message(&self, msg: M) {
        self.send_message(PropertyViewMsg::Custom(msg));
    }

    fn send_reload(&self) {
        self.send_message(PropertyViewMsg::Load);
    }

    fn send_show_dialog(&self, dialog: Option<Html>) {
        self.send_message(PropertyViewMsg::ShowDialog(dialog));
    }

    fn send_edit_property(&self, property: EditableProperty) {
        self.send_message(PropertyViewMsg::EditProperty(property));
    }
}

pub trait PropertyView: DerefMut<Target = PropertyViewState> {
    type Properties: Properties;
    type Message;
    const MOBILE: bool;

    fn loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>>;

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>>;

    fn create(ctx: &Context<PvePropertyView<Self>>) -> Self
    where
        Self: 'static + Sized;

    #[allow(unused_variables)]
    fn update(&mut self, ctx: &Context<PvePropertyView<Self>>, msg: Self::Message) -> bool
    where
        Self: 'static + Sized,
    {
        true
    }

    #[allow(unused_variables)]
    fn changed(
        &mut self,
        ctx: &Context<PvePropertyView<Self>>,
        old_props: &Self::Properties,
    ) -> bool
    where
        Self: 'static + Sized,
    {
        true
    }

    #[allow(unused_variables)]
    fn update_data(&mut self, ctx: &Context<PvePropertyView<Self>>)
    where
        Self: 'static + Sized,
    {
    }

    fn view(&self, ctx: &Context<PvePropertyView<Self>>) -> Html
    where
        Self: 'static + Sized;
}

#[derive(Default)]
pub struct PropertyViewState {
    pub data: Option<Value>,
    pub error: Option<String>,
    pub reload_timeout: Option<Timeout>,
    pub load_guard: Option<AsyncAbortGuard>,
    pub dialog: Option<Html>,
}

impl PropertyViewState {
    pub fn set_load_result(&mut self, result: Result<Value, String>) {
        match result {
            Ok(data) => {
                self.error = None;
                self.data = Some(data);
            }
            Err(err) => {
                self.error = Some(err);
            }
        }
    }

    pub fn loading(&self) -> bool {
        self.data.is_none() && self.error.is_none()
    }
}

pub struct PvePropertyView<T> {
    state: T,
}

impl<T: 'static + PropertyView> Component for PvePropertyView<T> {
    type Message = PropertyViewMsg<T::Message>;
    type Properties = T::Properties;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(PropertyViewMsg::Load);

        let mut state = T::create(ctx);
        state.update_data(ctx);

        Self { state }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            PropertyViewMsg::Custom(custom) => {
                return self.state.update(ctx, custom);
            }
            PropertyViewMsg::Select(_key) => { /* just redraw */ }
            PropertyViewMsg::EditProperty(property) => {
                if property.render_input_panel.is_none() {
                    return false;
                }
                let dialog = PropertyEditDialog::from(property.clone())
                    .mobile(T::MOBILE)
                    .on_done(ctx.link().callback(|_| PropertyViewMsg::ShowDialog(None)))
                    .loader(T::loader(props))
                    .on_submit(T::on_submit(props))
                    .into();
                self.state.dialog = Some(dialog);
            }
            PropertyViewMsg::Load => {
                self.state.reload_timeout = None;
                let link = ctx.link().clone();
                if let Some(loader) = T::loader(props) {
                    self.state.load_guard = Some(AsyncAbortGuard::spawn(async move {
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
                self.state.set_load_result(result);

                self.state.update_data(ctx);
                let link = ctx.link().clone();
                self.state.reload_timeout = Some(Timeout::new(3000, move || {
                    link.send_message(PropertyViewMsg::Load);
                }));
            }
            PropertyViewMsg::ShowDialog(dialog) => {
                if dialog.is_none() && self.state.reload_timeout.is_some() {
                    ctx.link().send_message(PropertyViewMsg::Load);
                }
                self.state.dialog = dialog;
            }
        }
        true
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        self.state.changed(ctx, old_props)
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        self.state.view(ctx)
    }
}

/// Render into a [Column] with toolbar, loading indicator and error display
///
/// With optional dialog widget.
pub fn render_loadable_panel(
    class: Classes,
    panel: Html,
    toolbar: Option<Html>,
    dialog: Option<Html>,
    loading: bool,
    error: Option<String>,
) -> Html {
    Column::new()
        .class(class)
        .with_optional_child(toolbar)
        .style("position", "relative")
        .with_child(
            pwt::widget::Progress::new()
                .class("pwt-delay-visibility")
                .style("z-index", "1")
                .style("position", "absolute")
                .style("left", "0")
                .style("right", "0")
                .style("visibility", (!loading).then(|| "hidden")),
        )
        .with_child(panel)
        .with_optional_child(
            error.map(|err| pwt::widget::error_message(&err.to_string()).padding(2)),
        )
        .with_optional_child(dialog)
        .into()
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

    if let Some(renderer) = &property.renderer {
        renderer.apply(&render_name, value.unwrap_or(&Value::Null), record)
    } else {
        match value {
            None::<_> | Some(Value::Null) => {
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
            Some(value) => match value {
                Value::String(value) => value.clone(),
                Value::Bool(value) => render_boolean(*value),
                Value::Number(n) => n.to_string(),
                v => v.to_string(),
            }
            .into(),
        }
    }
}
