use std::rc::Rc;

use serde::{de::DeserializeOwned, Serialize};

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::prelude::*;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::Loader;
use pwt::props::{IntoLoadCallback, LoadCallback, RenderFn};
use pwt::widget::{Dialog};

use pwt_macros::builder;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct DataViewWindow<T: PartialEq> {
    /// Yew component key
    #[prop_or_default]
    pub key: Option<Key>,

    /// Window title
    #[prop_or_default]
    pub title: AttrValue,

    // Form renderer.
    #[prop_or_default]
    pub renderer: Option<RenderFn<T>>,

    /// Form data loader.
    #[builder_cb(IntoLoadCallback, into_load_callback, T)]
    #[prop_or_default]
    pub loader: Option<LoadCallback<T>>,

    /// Determines if the dialog can be moved
    #[prop_or(true)]
    #[builder]
    pub draggable: bool,

    /// Determines if the dialog can be resized
    #[prop_or_default]
    #[builder]
    pub resizable: bool,

    /// Determines if the dialog should be auto centered
    #[prop_or(true)]
    #[builder]
    pub auto_center: bool,

    /// CSS style for the dialog window.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub style: Option<AttrValue>,

    /// Close/Abort callback.
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_done: Option<Callback<()>>,
}

impl<T: 'static + PartialEq> DataViewWindow<T> {
    pub fn new(title: impl Into<AttrValue>) -> Self {
        yew::props!(Self {
            title: title.into(),
        })
    }

    /// Builder style method to set the yew `key` property
    pub fn key(mut self, key: impl IntoOptionalKey) -> Self {
        self.key = key.into_optional_key();
        self
    }

    pub fn renderer(mut self, renderer: impl 'static + Fn(&T) -> Html) -> Self {
        self.renderer = Some(RenderFn::new(renderer));
        self
    }
}

#[doc(hidden)]
pub struct ProxmoxDataViewWindow<T> {
    loader: Loader<T>,
}

impl<T: 'static + Serialize + DeserializeOwned + PartialEq> Component for ProxmoxDataViewWindow<T> {
    type Message = ();
    type Properties = DataViewWindow<T>;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let loader = Loader::new()
            .loader(props.loader.clone())
            .on_change(ctx.link().callback(|_| ()));

        loader.load();

        Self { loader }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let renderer = props.renderer.clone();

        let panel = self.loader.render(move |data| {
            if let Some(renderer) = &renderer {
                renderer.apply(&data)
            } else {
                html! {}
            }
        });

        Dialog::new(props.title.clone())
            .on_close(props.on_done.clone())
            .draggable(props.draggable)
            .resizable(props.resizable)
            .auto_center(props.auto_center)
            .style(props.style.clone())
            .with_child(panel)
            .into()
    }
}

impl<T: 'static + Serialize + DeserializeOwned + PartialEq> From<DataViewWindow<T>> for VNode {
    fn from(props: DataViewWindow<T>) -> VNode {
        let key = props.key.clone();
        let comp = VComp::new::<ProxmoxDataViewWindow<T>>(Rc::new(props), key);
        VNode::from(comp)
    }
}
