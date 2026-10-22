use std::rc::Rc;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

use pwt::prelude::*;
use pwt::props::RenderFn;
use pwt::state::Loader;

/// A transaction detail loaded only while its tab is visible.
#[derive(Properties, PartialEq, Clone)]
pub(crate) struct TransactionDetail<T: Clone + PartialEq> {
    pub base_url: AttrValue,
    pub path: AttrValue,
    pub transaction: Option<(i64, u32)>,
    pub visible: bool,
    pub generation: u64,
    #[prop_or_default]
    pub parameters: Map<String, Value>,
    pub renderer: RenderFn<Rc<T>>,
}

pub(crate) struct ProxmoxTransactionDetail<T> {
    loader: Loader<T>,
}

impl<T: 'static + Clone + PartialEq + Serialize + DeserializeOwned> ProxmoxTransactionDetail<T> {
    fn load(&mut self, ctx: &Context<Self>) {
        let props = ctx.props();
        if !props.visible {
            return;
        }
        let Some((start_time, start_index)) = props.transaction else {
            return;
        };
        let url = format!("{}/{}", props.base_url, props.path);
        let mut parameters = props.parameters.clone();
        parameters.insert("start-time".into(), start_time.into());
        parameters.insert("start-index".into(), start_index.into());
        {
            let mut state = self.loader.write();
            state.notify = false;
            state.data = None;
        }
        self.loader.set_loader(move || {
            let url = url.clone();
            let parameters = parameters.clone();
            async move {
                crate::http_get(url, Some(Value::Object(parameters)))
                    .await
                    .map_err(crate::apt_history::api_error_message)
            }
        });
        self.loader.load();
    }
}

impl<T: 'static + Clone + PartialEq + Serialize + DeserializeOwned> Component
    for ProxmoxTransactionDetail<T>
{
    type Message = ();
    type Properties = TransactionDetail<T>;

    fn create(ctx: &Context<Self>) -> Self {
        let mut this = Self {
            loader: Loader::new().on_change(ctx.link().callback(|_| ())),
        };
        this.load(ctx);
        this
    }

    fn changed(&mut self, ctx: &Context<Self>, old: &Self::Properties) -> bool {
        let props = ctx.props();
        let changed = props.transaction != old.transaction
            || props.generation != old.generation
            || props.base_url != old.base_url
            || props.path != old.path
            || props.parameters != old.parameters;
        if changed {
            self.loader = Loader::new().on_change(ctx.link().callback(|_| ()));
        }
        if changed
            || (props.visible
                && !old.visible
                && !self.loader.loading()
                && !self.loader.has_valid_data())
        {
            self.load(ctx);
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if ctx.props().transaction.is_none() {
            return html! {};
        }
        self.loader.render(|data| ctx.props().renderer.apply(&data))
    }
}
