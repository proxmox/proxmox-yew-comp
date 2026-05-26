use anyhow::{bail, Error};
use serde_json::Value;
use std::{
    fmt::Debug,
    rc::Rc,
    sync::atomic::{AtomicU32, Ordering},
};
use yew::{html::IntoPropValue, virtual_dom::Key};

use pwt::{
    css::{AlignItems, ColorScheme, FlexFit, FontColor},
    prelude::*,
    props::FieldStdProps,
    props::RenderFn,
    state::Store,
    widget::{
        data_table::{DataTable, DataTableColumn, DataTableHeader},
        form::{
            Field, IntoSubmitValidateFn, ManagedField, ManagedFieldContext, ManagedFieldMaster,
            ManagedFieldScopeExt, ManagedFieldState, SubmitValidateFn,
        },
        ActionIcon, Button, Column, Container, Fa, Row,
    },
};
use pwt_macros::{builder, widget};

#[widget(comp = ManagedFieldMaster<KeyValueListField>, @input)]
#[derive(Clone, PartialEq, Properties)]
#[builder]
/// A [`DataTable`]-based grid to hold a list of user-enterable key-value pairs.
///
/// Displays a [`DataTable`] with three columns; key, value and a delete button, with an add button
/// below to create new rows.
/// Both key and value are modifiable by the user.
pub struct KeyValueList {
    #[builder]
    #[prop_or_default]
    /// Initial value pairs to display.
    pub value: Vec<(String, Value)>,

    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(tr!("Name").into())]
    /// Label for the key column, defaults to "Name".
    pub key_label: AttrValue,

    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    /// Placeholder to display in the key columns fields, default is no placeholder.
    pub key_placeholder: Option<AttrValue>,

    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or(tr!("Value").into())]
    /// Label for the value column.
    pub value_label: AttrValue,

    #[builder]
    #[prop_or(default_value_renderer.into())]
    pub value_renderer: RenderFn<(String, Value, FieldStdProps, Callback<String>), Html>,

    #[builder_cb(IntoSubmitValidateFn, into_submit_validate_fn, Vec<(String, Value)>)]
    #[prop_or_default]
    /// Callback to run on submit on the data in the table.
    pub submit_validate: Option<SubmitValidateFn<Vec<(String, Value)>>>,
}

impl KeyValueList {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    /// Only used as key for the store, since that needs a stable value
    index: u32,
    key: String,
    value: Value,
}

pub struct KeyValueListField {
    state: ManagedFieldState,
    store: Store<Entry>,
    index_counter: AtomicU32,
    columns: Rc<Vec<DataTableHeader<Entry>>>,
}

pwt::impl_deref_mut_property!(KeyValueListField, state, ManagedFieldState);

pub enum Message {
    DataChange,
    UpdateKey(u32, String),
    UpdateValue(u32, Value),
    RemoveEntry(u32),
}

impl KeyValueListField {
    fn set_data(&mut self, data: &[(String, Value)]) {
        self.store.set_data(
            data.iter()
                .enumerate()
                .map(|(i, (k, v))| Entry {
                    index: i as u32,
                    key: k.clone(),
                    value: v.clone(),
                })
                .collect(),
        );
    }

    fn columns(ctx: &ManagedFieldContext<KeyValueListField>) -> Rc<Vec<DataTableHeader<Entry>>> {
        let props = ctx.props().clone();
        let link = ctx.link().clone();

        Rc::new(vec![
            DataTableColumn::new(props.key_label.clone())
                .flex(1)
                .render({
                    let link = link.clone();
                    let props = props.clone();
                    move |item: &Entry| {
                        let index = item.index;
                        Field::new()
                            .on_change(link.callback(move |value| Message::UpdateKey(index, value)))
                            .required(true)
                            .disabled(props.input_props.disabled)
                            .placeholder(props.key_placeholder.clone())
                            .validate(|s: &String| {
                                if s.is_empty() {
                                    bail!("Field may not be empty");
                                } else {
                                    Ok(())
                                }
                            })
                            .value(item.key.clone())
                            .into()
                    }
                })
                .sorter(|a: &Entry, b: &Entry| a.key.cmp(&b.key))
                .into(),
            DataTableColumn::new(props.value_label.clone())
                .flex(1)
                .render({
                    let link = link.clone();
                    let props = props.clone();
                    move |item: &Entry| {
                        let on_change = link.callback({
                            let index = item.index;
                            move |value: String| Message::UpdateValue(index, Value::String(value))
                        });
                        props.value_renderer.apply(&(
                            item.key.clone(),
                            item.value.clone(),
                            props.input_props.clone(),
                            on_change,
                        ))
                    }
                })
                .into(),
            DataTableColumn::new("")
                .width("50px")
                .render(move |item: &Entry| {
                    let index = item.index;
                    ActionIcon::new("fa fa-lg fa-trash-o")
                        .tabindex(0)
                        .on_activate(link.callback(move |_| Message::RemoveEntry(index)))
                        .disabled(props.input_props.disabled)
                        .into()
                })
                .into(),
        ])
    }
}

impl ManagedField for KeyValueListField {
    type Message = Message;
    type Properties = KeyValueList;
    type ValidateClosure = (bool, Option<SubmitValidateFn<Vec<(String, Value)>>>);

    fn create(ctx: &ManagedFieldContext<Self>) -> Self {
        let store = Store::with_extract_key(|entry: &Entry| Key::from(entry.index))
            .on_change(ctx.link().callback(|_| Message::DataChange));

        // put the default value through the validator fn, to allow for correct dirty checking
        let default = if let Some(f) = &ctx.props().submit_validate {
            f.apply(&ctx.props().value).unwrap_or_default()
        } else {
            serde_json::to_value(ctx.props().value.clone()).unwrap_or_default()
        };

        let mut this = Self {
            state: ManagedFieldState::new(Value::Null, default),
            store,
            index_counter: AtomicU32::new(ctx.props().value.len() as u32),
            columns: Self::columns(ctx),
        };
        this.set_data(&ctx.props().value);
        this
    }

    fn validation_args(props: &Self::Properties) -> Self::ValidateClosure {
        (props.input_props.required, props.submit_validate.clone())
    }

    fn validator(props: &Self::ValidateClosure, value: &Value) -> Result<Value, Error> {
        let data = serde_json::from_value::<Vec<(String, Value)>>(value.clone())?;

        if data.is_empty() && props.0 {
            bail!(tr!("at least one entry required!"));
        }

        if data.iter().any(|(k, _)| k.is_empty()) {
            bail!(tr!("name must not be empty!"));
        }

        if let Some(cb) = &props.1 {
            cb.apply(&data)
        } else {
            Ok(value.clone())
        }
    }

    fn changed(&mut self, ctx: &ManagedFieldContext<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if old_props.value != props.value {
            let data: Value = props
                .value
                .iter()
                .filter_map(|n| serde_json::to_value(n).ok())
                .collect();

            ctx.link().update_default(data.clone());
        }
        self.columns = Self::columns(ctx);
        true
    }

    fn value_changed(&mut self, _ctx: &ManagedFieldContext<Self>) {
        match &self.state.value {
            Value::Null => {
                let data =
                    serde_json::from_value::<Vec<(String, Value)>>(self.state.default.clone())
                        .unwrap();
                self.set_data(&data);
            }
            Value::Object(map) => {
                let values: Vec<(String, Value)> = map
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();

                self.set_data(&values);
            }
            value => {
                let data = serde_json::from_value::<Vec<(String, Value)>>(value.clone()).unwrap();
                self.set_data(&data);
            }
        }
    }

    fn update(&mut self, ctx: &ManagedFieldContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Message::DataChange => {
                let list: Vec<(String, Value)> = self
                    .store
                    .read()
                    .iter()
                    .map(|Entry { key, value, .. }| (key.clone(), value.clone()))
                    .collect();

                ctx.link().update_value(serde_json::to_value(list).unwrap());
                true
            }
            Message::RemoveEntry(index) => {
                self.store.write().retain(|item| item.index != index);
                true
            }
            Message::UpdateKey(index, new_name) => {
                let mut data = self.store.write();
                if let Some(item) = data.iter_mut().find(|item| item.index == index) {
                    item.key = new_name;
                }
                true
            }
            Message::UpdateValue(index, value) => {
                let mut data = self.store.write();
                if let Some(item) = data.iter_mut().find(|item| item.index == index) {
                    item.value = value;
                }
                true
            }
        }
    }

    fn view(&self, ctx: &ManagedFieldContext<Self>) -> Html {
        let props = ctx.props();

        let table = DataTable::new(Rc::clone(&self.columns), self.store.clone())
            .border(true)
            .class(FlexFit);

        let button_row = Row::new()
            .with_child(
                Button::new(tr!("Add"))
                    .class(ColorScheme::Primary)
                    .icon_class("fa fa-plus-circle")
                    .disabled(props.input_props.disabled)
                    .on_activate({
                        let store = self.store.clone();
                        let index = self.index_counter.fetch_add(1, Ordering::Relaxed);
                        move |_| {
                            store.write().push(Entry {
                                index,
                                key: String::new(),
                                value: String::new().into(),
                            });
                        }
                    }),
            )
            .with_flex_spacer()
            .with_optional_child(self.state.result.clone().err().map(|err| {
                Row::new()
                    .class(AlignItems::Center)
                    .gap(2)
                    .with_child(Fa::new("exclamation-triangle").class(FontColor::Error))
                    .with_child(err)
            }));

        Column::new()
            .class(FlexFit)
            .gap(2)
            .with_child(
                Container::from_widget_props(ctx.props().std_props.clone(), None)
                    .class(FlexFit)
                    .with_child(table),
            )
            .with_child(button_row)
            .into()
    }
}

fn default_value_renderer(
    (_key, value, input_props, on_change): &(String, Value, FieldStdProps, Callback<String>),
) -> Html {
    Field::new()
        .value(match value {
            Value::String(s) => s.to_owned(),
            Value::Number(n) => n.as_i64().unwrap_or_default().to_string(),
            other => other.to_string(),
        })
        .disabled(input_props.disabled)
        .on_change(on_change)
        .into()
}
