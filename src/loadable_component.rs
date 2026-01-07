use std::future::Future;
use std::ops::DerefMut;
use std::pin::Pin;

use anyhow::Error;
use gloo_timers::callback::Timeout;

use serde_json::Value;
use yew::html::Scope;

use pwt::dom::DomVisibilityObserver;
use pwt::prelude::*;
use pwt::widget::{AlertDialog, Column};
use pwt::AsyncPool;

#[cfg(doc)]
#[cfg(doc)]
use pwt::widget::Dialog;

use crate::{TaskProgress, TaskViewer};

pub type LoadableComponentContext<L> = Context<LoadableComponentMaster<L>>;
pub type LoadableComponentScope<L> = Scope<LoadableComponentMaster<L>>;

/// Loadable Components
///
/// - Load data using an async function [LoadableComponent::load]
/// - repeated load possible
/// - pause repeated load when component is not visible (uses [DomVisibilityObserver])
/// - display the loaded data [LoadableComponent::main_view]
/// - display an optional toolbar [LoadableComponent::toolbar]
/// - display any errors from failed load.
/// - display additional dialogs depening on [LoadableComponent::ViewState]
///
/// The [LoadableComponentScopeExt] defines available control function on the scope.
///
/// The [LoadableComponentState] provides acces to load status informations and add the ability
/// to spawn tasks.
///
/// ```
/// use proxmox_yew_comp::{LoadableComponent, LoadableComponentState, LoadableComponentContext};
/// // include the scope extension for (for `change_view`, `send_custom_message`, ...)
/// use proxmox_yew_comp::LoadableComponentScopeExt;
/// # use std::pin::Pin;
/// # use std::rc::Rc;
/// # use std::future::Future;
/// # use pwt::prelude::*;
/// # use proxmox_yew_comp::http_get;
/// # use yew::virtual_dom::{VComp, VNode, Key};
///
/// // define the component properties
/// #[derive(Clone, PartialEq, Properties)]
/// pub struct MyComponent {
///     key: Option<Key>,
///     /* add whatever you need  */
/// };
///
/// // define your view states
/// #[derive(PartialEq)]
/// pub enum ViewState { Add, Edit }
///
/// // define the component message type
/// pub enum Msg { UpdateData(String) }
///
/// // define the component state
/// pub struct MyComponentState {
///     // you need to incluce a LoadableComponentState
///     state: LoadableComponentState<ViewState>,
///     // Add any other data you need
///     loaded_data: Option<String>,
/// }
///
/// // implement DerefMut
/// pwt::impl_deref_mut_property!(
///     MyComponentState,
///     state,
///     LoadableComponentState<ViewState>
/// );
///
/// impl LoadableComponent for MyComponentState {
///     type Properties = MyComponent;
///     type Message = Msg; // component message type
///     type ViewState = ViewState;
///
///     fn create(ctx: &LoadableComponentContext<Self>) -> Self {
///         Self {
///             state: LoadableComponentState::new(),
///             loaded_data: None,
///         }
///     }
///
///     fn load(
///         &self,
///         ctx: &LoadableComponentContext<Self>,
///     ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
///         let link = ctx.link().clone();
///         Box::pin(async move {
///             let data = http_get("/something", None).await?; // load something here
///             link.send_message(Msg::UpdateData(data));
///             Ok(())
///         })
///     }
///
///     fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
///         match msg {
///             Msg::UpdateData(data) => self.loaded_data = Some(data),
///         }
///         true
///     }
///
///     fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
///         let text: String  = if let Some(data) = &self.loaded_data {
///             data.clone()
///         } else {
///             "no data".into()
///         };
///         html!{text}
///     }
/// }
///
/// // add ability to generate the yew Component (provided by [LoadableComponentMaster])
/// use proxmox_yew_comp::LoadableComponentMaster;
/// impl From<MyComponent> for VNode {
///     fn from(props: MyComponent) -> VNode {
///         let key =  props.key.clone();
///         let comp = VComp::new::<LoadableComponentMaster<MyComponentState>>(Rc::new(props), key);
///         VNode::from(comp)
///     }
/// }
///
/// ```
pub trait LoadableComponent:
    Sized + DerefMut<Target = LoadableComponentState<Self::ViewState>> + 'static
{
    /// The yew component properties.
    type Properties: Properties;
    /// The yew component message type.
    type Message: 'static;
    /// The view state
    ///
    /// The view state of the component can be changed with [LoadableComponentScopeExt::change_view].
    /// The value is then passed to the [LoadableComponent::dialog_view] function which can render
    /// different dialogs.
    type ViewState: 'static + PartialEq;

    /// Create a new instance
    fn create(ctx: &LoadableComponentContext<Self>) -> Self;

    /// Async Load
    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>>;

    /// Yew component update function (see [Component::update])
    #[allow(unused_variables)]
    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        true
    }

    /// Yew component changed function (see [Component::changed])
    #[allow(unused_variables)]
    fn changed(
        &mut self,
        ctx: &LoadableComponentContext<Self>,
        _old_props: &Self::Properties,
    ) -> bool {
        true
    }

    /// Optional toolbar
    #[allow(unused_variables)]
    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        None
    }

    /// Main view (see [Component::view])
    ///
    /// The difference is that we render the result into a [Column], with an optional
    /// toolbar on the top.
    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html;

    /// ViewState dependent dialogs
    ///
    /// The result is rendered below the main view. Usually some kind of [Dialog] window.
    ///
    /// The view state can be changed with `link.change_view(..)` and `link.change_view_callback(...)`.
    #[allow(unused_variables)]
    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        None
    }

    /// Yew component rendered function (see [Component::rendered])
    #[allow(unused_variables)]
    fn rendered(&mut self, ctx: &LoadableComponentContext<Self>, first_render: bool) {}
}

#[derive(Clone, PartialEq)]
pub enum ViewState<V: PartialEq> {
    Main,
    /// Show the dialog returned by dialog_view
    Dialog(V),
    /// Show proxmox api task status
    TaskProgress(String),
    /// Show proxmox api task log
    TaskLog(String, Option<i64>),
    /// Show an error message dialog
    Error(String, String, /* reload_on_close */ bool),
}

pub enum Msg<M, V: PartialEq> {
    DataChange,
    Load,
    RepeatedLoad(u32 /* repeat time in miliseconds */),
    LoadResult(Result<(), Error>),
    ChangeView(/*reload*/ bool, ViewState<V>),
    ChildMessage(M),
    Visible(bool),
    Spawn(Pin<Box<dyn Future<Output = ()>>>),
}

impl<M, V: PartialEq> From<M> for Msg<M, V> {
    fn from(value: M) -> Self {
        Msg::ChildMessage(value)
    }
}

pub trait LoadableComponentScopeExt<M, V: PartialEq> {
    fn send_reload(&self);
    fn send_redraw(&self);
    fn repeated_load(&self, miliseconds: u32);

    fn change_view(&self, child_view_state: Option<V>);
    fn change_view_callback<C, F, IN>(&self, function: F) -> Callback<IN>
    where
        C: Into<Option<V>>,
        F: Fn(IN) -> C + 'static;

    /// Spawn a future using the [AsyncPool] from the component.
    fn spawn<Fut>(&self, future: Fut)
    where
        Fut: Future<Output = ()> + 'static;

    fn show_error(
        &self,
        title: impl Into<String>,
        msg: impl std::fmt::Display,
        reload_on_close: bool,
    );

    fn show_task_progres(&self, task_id: impl Into<String>);

    fn show_task_log(&self, task_id: impl Into<String>, endtime: Option<i64>);

    fn start_task(&self, command_path: impl Into<String>, data: Option<Value>, short: bool);
}

impl<M, V: PartialEq, T: 'static + LoadableComponent<Message = M, ViewState = V>>
    LoadableComponentScopeExt<M, V> for Scope<LoadableComponentMaster<T>>
{
    fn send_reload(&self) {
        self.send_message(Msg::Load);
    }

    fn send_redraw(&self) {
        self.send_message(Msg::DataChange);
    }

    fn repeated_load(&self, miliseconds: u32) {
        self.send_message(Msg::RepeatedLoad(miliseconds));
    }

    fn change_view(&self, child_view_state: Option<V>) {
        let view_state = if let Some(child_view_state) = child_view_state {
            ViewState::Dialog(child_view_state)
        } else {
            ViewState::Main
        };
        self.send_message(Msg::ChangeView(false, view_state));
    }

    fn change_view_callback<C, F, IN>(&self, function: F) -> Callback<IN>
    where
        C: Into<Option<V>>,
        F: Fn(IN) -> C + 'static,
    {
        self.callback(move |p: IN| {
            let state: Option<V> = function(p).into();
            if let Some(state) = state {
                Msg::ChangeView(true, ViewState::Dialog(state))
            } else {
                Msg::ChangeView(true, ViewState::Main)
            }
        })
    }

    fn spawn<Fut>(&self, future: Fut)
    where
        Fut: Future<Output = ()> + 'static,
    {
        self.send_message(Msg::Spawn(Box::pin(future)));
    }

    fn show_error(
        &self,
        title: impl Into<String>,
        msg: impl std::fmt::Display,
        reload_on_close: bool,
    ) {
        let view_state = ViewState::Error(title.into(), msg.to_string(), reload_on_close);
        self.send_message(Msg::ChangeView(false, view_state));
    }

    fn show_task_progres(&self, task_id: impl Into<String>) {
        let view_state = ViewState::TaskProgress(task_id.into());
        self.send_message(Msg::ChangeView(false, view_state));
    }

    fn show_task_log(&self, task_id: impl Into<String>, endtime: Option<i64>) {
        let view_state = ViewState::TaskLog(task_id.into(), endtime);
        self.send_message(Msg::ChangeView(false, view_state));
    }

    fn start_task(&self, command_path: impl Into<String>, data: Option<Value>, short: bool) {
        let command_path: String = command_path.into();
        let link = self.clone();
        let command_future = crate::http_post::<String>(command_path, data);
        self.send_message(Msg::Spawn(Box::pin(async move {
            match command_future.await {
                Ok(task_id) => {
                    link.send_reload();
                    if short {
                        link.show_task_progres(task_id);
                    } else {
                        link.show_task_log(task_id, None);
                    }
                }
                Err(err) => {
                    log::error!("error {err}");
                    link.show_error("Start command failed", err, true);
                }
            }
        })));
    }
}

/// Base state for [LoadableComponent] implementations.
///
/// The struct provides the following features:
///
/// - access to load status informations
/// - setup task base url
/// - spawn tasks: includes an [AsyncPool], so that any [LoadableComponent] can spawn
///   task via this pool.
///
/// The [LoadableComponent] trait requires access to this struct via [DerefMut]. The
/// macro [impl_deref_mut_property] provides an easy way to
/// implement that.
///
/// ```
/// use proxmox_yew_comp::LoadableComponentState;
/// # #[derive(PartialEq)]
/// # pub enum ViewState { Add, Edit }
/// pub struct MyComponentState {
///     state: LoadableComponentState<ViewState>,
///     // Add any other data you need
///     other_data: String,
/// }
/// // implement DerefMut
/// pwt::impl_deref_mut_property!(MyComponentState, state, LoadableComponentState<ViewState>);
/// ```
pub struct LoadableComponentState<V: PartialEq> {
    loading: usize,
    last_load_error: Option<String>,
    repeat_timespan: u32, /* 0 => no repeated loading */
    task_base_url: Option<AttrValue>,
    view_state: ViewState<V>,
    reload_timeout: Option<Timeout>,
    visible: bool,
    visibility_observer: Option<DomVisibilityObserver>,
    node_ref: NodeRef,
    async_pool: AsyncPool,
}

impl<V: PartialEq> LoadableComponentState<V> {
    pub fn new() -> Self {
        Self {
            loading: 0,
            last_load_error: None,
            repeat_timespan: 0,
            task_base_url: None,
            view_state: ViewState::Main,
            reload_timeout: None,
            visible: true,
            visibility_observer: None,
            node_ref: NodeRef::default(),
            async_pool: AsyncPool::new(),
        }
    }

    pub fn loading(&self) -> bool {
        self.loading > 0
    }

    pub fn last_load_errors(&self) -> Option<&str> {
        self.last_load_error.as_deref()
    }

    pub fn set_task_base_url(&mut self, base_url: AttrValue) {
        self.task_base_url = Some(base_url);
    }

    /// Spawn a future using the [AsyncPool] from the component.
    pub fn spawn<Fut>(&self, future: Fut)
    where
        Fut: Future<Output = ()> + 'static,
    {
        self.async_pool.spawn(future);
    }
}

#[doc(hidden)]
pub struct LoadableComponentMaster<L: LoadableComponent> {
    state: L,
}

impl<L: LoadableComponent + 'static> Component for LoadableComponentMaster<L> {
    type Message = Msg<L::Message, L::ViewState>;
    type Properties = L::Properties;

    fn create(ctx: &Context<Self>) -> Self {
        // Send Msg::Load first (before any Msg::RepeatedLoad in create), so that we
        // can avoid multiple loads at startup
        ctx.link().send_message(Msg::Load);

        let mut state = L::create(ctx);
        state.visible = true;

        Self { state }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Spawn(future) => {
                self.state.async_pool.spawn(future);
                false
            }
            Msg::DataChange => true,
            Msg::Load => {
                let load_future = self.state.load(ctx);
                self.state.loading += 1;
                let link = ctx.link().clone();
                self.state.async_pool.spawn(async move {
                    let data = load_future.await;
                    link.send_message(Msg::LoadResult(data));
                });
                true
            }
            Msg::RepeatedLoad(timespan) => {
                self.state.repeat_timespan = timespan;
                self.state.reload_timeout = None;
                if self.state.loading == 0 {
                    <Self as yew::Component>::update(self, ctx, Msg::Load);
                }
                false
            }
            Msg::LoadResult(data) => {
                self.state.loading -= 1;
                match data {
                    Ok(()) => {
                        self.state.last_load_error = None;
                    }
                    Err(err) => {
                        let this_is_the_first_error = self.state.last_load_error.is_none();
                        self.state.last_load_error = Some(err.to_string());
                        if this_is_the_first_error {
                            self.state.view_state =
                                ViewState::Error(tr!("Load failed"), err.to_string(), false);
                        }
                    }
                }

                self.state.reload_timeout = None;
                if self.state.loading == 0 {
                    /* no outstanding loads */
                    if self.state.repeat_timespan > 0 {
                        let link = ctx.link().clone();
                        if self.state.visible {
                            self.state.reload_timeout =
                                Some(Timeout::new(self.state.repeat_timespan, move || {
                                    link.send_message(Msg::Load);
                                }));
                        }
                    }
                }
                true
            }
            Msg::ChangeView(reload_data, view_state) => {
                if self.state.view_state == view_state {
                    return false;
                }

                if reload_data {
                    ctx.link().send_message(Msg::Load);
                }

                self.state.view_state = view_state;
                true
            }
            Msg::ChildMessage(child_msg) => {
                self.state.update(ctx, child_msg);
                true
            }
            Msg::Visible(visible) => {
                if self.state.visible == visible {
                    return false;
                }
                self.state.visible = visible;
                if self.state.loading == 0 && self.state.visible {
                    /* no outstanding loads */
                    if self.state.loading == 0 {
                        <Self as yew::Component>::update(self, ctx, Msg::Load);
                    }
                }
                false
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        self.state.changed(ctx, _old_props)
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let main_view = self.state.main_view(ctx);

        let dialog: Option<Html> =
            match &self.state.view_state {
                ViewState::Main => None,
                ViewState::Dialog(view_state) => self.state.dialog_view(ctx, view_state),
                ViewState::Error(title, msg, reload_on_close) => {
                    let reload_on_close = *reload_on_close;
                    Some(
                        AlertDialog::new(msg)
                            .title(title.clone())
                            .on_close(ctx.link().callback(move |_| {
                                Msg::ChangeView(reload_on_close, ViewState::Main)
                            }))
                            .into(),
                    )
                }
                ViewState::TaskProgress(task_id) => {
                    let mut task_progress = TaskProgress::new(task_id).on_close(
                        ctx.link()
                            .callback(move |_| Msg::ChangeView(true, ViewState::Main)),
                    );

                    if let Some(base_url) = &self.state.task_base_url {
                        task_progress.set_base_url(base_url);
                    }

                    Some(task_progress.into())
                }
                ViewState::TaskLog(task_id, endtime) => {
                    let mut task_viewer = TaskViewer::new(task_id).endtime(endtime).on_close(
                        ctx.link()
                            .callback(move |_| Msg::ChangeView(true, ViewState::Main)),
                    );

                    if let Some(base_url) = &self.state.task_base_url {
                        task_viewer.set_base_url(base_url);
                    }
                    Some(task_viewer.into())
                }
            };

        let toolbar = self.state.toolbar(ctx);

        let mut alert_msg = None;

        if dialog.is_none() {
            if let Some(msg) = &self.state.last_load_error {
                alert_msg = Some(pwt::widget::error_message(msg).class("pwt-border-top"));
            }
        }

        Column::new()
            .class("pwt-flex-fill pwt-overflow-auto")
            .with_optional_child(toolbar)
            .with_child(main_view)
            .with_optional_child(alert_msg)
            .with_optional_child(dialog)
            .into_html_with_ref(self.state.node_ref.clone())
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if self.state.visibility_observer.is_none() && self.state.reload_timeout.is_some() {
            if let Some(el) = self.state.node_ref.cast::<web_sys::Element>() {
                self.state.visibility_observer = Some(DomVisibilityObserver::new(
                    &el,
                    ctx.link().callback(Msg::Visible),
                ))
            }
        }
        self.state.rendered(ctx, first_render);
    }
}
