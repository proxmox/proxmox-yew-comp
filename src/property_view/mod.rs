mod property_grid;
pub use property_grid::{property_grid_columns, PropertyGrid};

mod property_list;
pub use property_list::PropertyList;

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

pub trait PropertyView {
    type Properties: Properties;
    type Message;
    const MOBILE: bool;

    fn loader(props: &Self::Properties) -> Option<ApiLoadCallback<Value>>;

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>>;

    fn create(ctx: &Context<PvePropertyView<Self>>) -> Self
    where
        Self: 'static + Sized;

    #[allow(unused_variables)]
    fn update(
        &mut self,
        ctx: &Context<PvePropertyView<Self>>,
        view_state: &mut PropertyViewState,
        msg: Self::Message,
    ) -> bool
    where
        Self: 'static + Sized,
    {
        true
    }

    #[allow(unused_variables)]
    fn changed(
        &mut self,
        ctx: &Context<PvePropertyView<Self>>,
        view_state: &mut PropertyViewState,
        old_props: &Self::Properties,
    ) -> bool
    where
        Self: 'static + Sized,
    {
        true
    }

    #[allow(unused_variables)]
    fn update_data(
        &mut self,
        ctx: &Context<PvePropertyView<Self>>,
        view_state: &mut PropertyViewState,
    ) where
        Self: 'static + Sized,
    {
    }

    fn view(&self, ctx: &Context<PvePropertyView<Self>>, view_state: &PropertyViewState) -> Html
    where
        Self: 'static + Sized;
}

pub struct PropertyViewState {
    pub data: Option<Value>,
    pub error: Option<String>,
    pub reload_timeout: Option<Timeout>,
    pub load_guard: Option<AsyncAbortGuard>,
    pub dialog: Option<Html>,
}

impl PropertyViewState {
    pub fn update(&mut self, result: Result<Value, String>) {
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
    view_state: PropertyViewState,
    child_state: T,
}

impl<T: 'static + PropertyView> Component for PvePropertyView<T> {
    type Message = PropertyViewMsg<T::Message>;
    type Properties = T::Properties;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(PropertyViewMsg::Load);

        let mut me = Self {
            view_state: PropertyViewState {
                data: None,
                error: None,
                reload_timeout: None,
                load_guard: None,
                dialog: None,
            },
            child_state: T::create(ctx),
        };
        me.child_state.update_data(ctx, &mut me.view_state);
        me
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            PropertyViewMsg::Custom(custom) => {
                return self.child_state.update(ctx, &mut self.view_state, custom);
            }
            PropertyViewMsg::Select(_key) => { /* just redraw */ }
            PropertyViewMsg::EditProperty(property) => {
                let dialog = PropertyEditDialog::from(property.clone())
                    .mobile(T::MOBILE)
                    .on_done(ctx.link().callback(|_| PropertyViewMsg::ShowDialog(None)))
                    .loader(T::loader(props))
                    .on_submit(T::on_submit(props))
                    .into();
                self.view_state.dialog = Some(dialog);
            }
            PropertyViewMsg::Load => {
                self.view_state.reload_timeout = None;
                let link = ctx.link().clone();
                if let Some(loader) = T::loader(props) {
                    self.view_state.load_guard = Some(AsyncAbortGuard::spawn(async move {
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
                self.view_state.update(result);

                self.child_state.update_data(ctx, &mut self.view_state);
                let link = ctx.link().clone();
                self.view_state.reload_timeout = Some(Timeout::new(3000, move || {
                    link.send_message(PropertyViewMsg::Load);
                }));
            }
            PropertyViewMsg::ShowDialog(dialog) => {
                if dialog.is_none() && self.view_state.reload_timeout.is_some() {
                    ctx.link().send_message(PropertyViewMsg::Load);
                }
                self.view_state.dialog = dialog;
            }
        }
        true
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        self.child_state
            .changed(ctx, &mut self.view_state, old_props)
        /*
        let props = ctx.props();
         if T::properties(props) != T::properties(old_props) {
             self.child_state.update_data(ctx, &mut self.view_state);
         }
         true
         */
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        self.child_state.view(ctx, &self.view_state)
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
        .with_optional_child(
            loading.then(|| pwt::widget::Progress::new().class("pwt-delay-visibility")),
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
