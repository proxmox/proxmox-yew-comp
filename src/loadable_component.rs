use anyhow::Error;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use yew_router::scope_ext::RouterScopeExt;

use gloo_timers::callback::Timeout;

use yew::html::Scope;

use pwt::dom::DomVisibilityObserver;
use pwt::prelude::*;
use pwt::state::NavigationContextExt;
use pwt::widget::{AlertDialog, Column};
use pwt::AsyncPool;

use crate::{TaskProgress, TaskViewer};

pub struct LoadableComponentState {
    loading: usize,
    last_load_error: Option<String>,
    repeat_timespan: u32, /* 0 => no repeated loading */
    task_base_url: Option<AttrValue>,
}

pub struct LoadableComponentContext<'a, L: LoadableComponent + Sized + 'static> {
    ctx: &'a Context<LoadableComponentMaster<L>>,
    comp_state: &'a LoadableComponentState,
}

impl<L: LoadableComponent + Sized> LoadableComponentContext<'_, L> {
    pub fn props(&self) -> &L::Properties {
        self.ctx.props()
    }
    pub fn link(&self) -> LoadableComponentLink<L> {
        LoadableComponentLink {
            link: self.ctx.link().clone(),
        }
    }
    pub fn loading(&self) -> bool {
        self.comp_state.loading > 0
    }

    pub fn last_load_errors(&self) -> Option<&str> {
        self.comp_state.last_load_error.as_deref()
    }
}

pub struct LoadableComponentLink<L: LoadableComponent + Sized + 'static> {
    link: Scope<LoadableComponentMaster<L>>,
}

impl<L: LoadableComponent + Sized> Clone for LoadableComponentLink<L> {
    fn clone(&self) -> Self {
        Self {
            link: self.link.clone(),
        }
    }
}

impl<L: LoadableComponent + Sized> LoadableComponentLink<L> {
    pub fn send_message(&self, msg: impl Into<L::Message>) {
        let msg = msg.into();
        self.link.send_message(Msg::ChildMessage(msg));
    }

    pub fn callback<F, IN, M>(&self, function: F) -> Callback<IN>
    where
        M: Into<L::Message>,
        F: Fn(IN) -> M + 'static,
    {
        self.link.callback(move |p: IN| {
            let msg: L::Message = function(p).into();
            Msg::ChildMessage(msg)
        })
    }

    /// Spawn a future using the [AsyncPool] from the component.
    pub fn spawn<Fut>(&self, future: Fut)
    where
        Fut: Future<Output = ()> + 'static,
    {
        self.link.send_message(Msg::Spawn(Box::pin(future)));
    }

    pub fn send_future<Fut, M>(&self, future: Fut)
    where
        M: Into<L::Message>,
        Fut: Future<Output = M> + 'static,
    {
        let link = self.link.clone();
        self.link.send_message(Msg::Spawn(Box::pin(async move {
            let message: L::Message = future.await.into();
            link.send_message(Msg::ChildMessage(message));
        })));
    }

    pub fn callback_future<F, Fut, IN, M>(&self, function: F) -> Callback<IN>
    where
        M: Into<L::Message>,
        Fut: Future<Output = M> + 'static,
        F: Fn(IN) -> Fut + 'static,
    {
        let link = self.clone();

        let closure = move |input: IN| {
            link.send_future(function(input));
        };

        closure.into()
    }

    pub fn send_reload(&self) {
        self.link.send_message(Msg::Load)
    }

    pub fn repeated_load(&self, miliseconds: u32) {
        self.link.send_message(Msg::RepeatedLoad(miliseconds));
    }

    pub fn task_base_url(&self, base_url: impl Into<AttrValue>) {
        self.link.send_message(Msg::TaskBaseUrl(base_url.into()));
    }

    pub fn show_error(
        &self,
        title: impl Into<String>,
        msg: impl std::fmt::Display,
        reload_on_close: bool,
    ) {
        let view_state = ViewState::Error(title.into(), msg.to_string(), reload_on_close);
        self.link.send_message(Msg::ChangeView(false, view_state));
    }

    pub fn show_task_progres(&self, task_id: impl Into<String>) {
        let view_state = ViewState::TaskProgress(task_id.into());
        self.link.send_message(Msg::ChangeView(false, view_state));
    }

    pub fn show_task_log(&self, task_id: impl Into<String>, endtime: Option<i64>) {
        let view_state = ViewState::TaskLog(task_id.into(), endtime);
        self.link.send_message(Msg::ChangeView(false, view_state));
    }

    pub fn change_view(&self, child_view_state: Option<L::ViewState>) {
        let view_state = if let Some(child_view_state) = child_view_state {
            ViewState::Dialog(child_view_state)
        } else {
            ViewState::Main
        };
        self.link.send_message(Msg::ChangeView(false, view_state));
    }

    pub fn change_view_callback<F, IN, M>(&self, function: F) -> Callback<IN>
    where
        M: Into<Option<L::ViewState>>,
        F: Fn(IN) -> M + 'static,
    {
        self.link.callback(move |p: IN| {
            let state: Option<L::ViewState> = function(p).into();
            if let Some(state) = state {
                Msg::ChangeView(true, ViewState::Dialog(state))
            } else {
                Msg::ChangeView(true, ViewState::Main)
            }
        })
    }

    pub fn start_task(&self, command_path: impl Into<String>, data: Option<Value>, short: bool) {
        let command_path: String = command_path.into();
        let link = self.clone();
        let command_future = crate::http_post::<String>(command_path, data);
        self.link.send_message(Msg::Spawn(Box::pin(async move {
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

    /// Returns the original [`yew::html::Scope`] of the master component.
    ///
    /// This is useful when e.g. trying to get an higher level context
    pub fn yew_link(&self) -> &Scope<LoadableComponentMaster<L>> {
        &self.link
    }
}

impl<L: LoadableComponent + Sized> RouterScopeExt for LoadableComponentLink<L> {
    fn navigator(&self) -> Option<yew_router::prelude::Navigator> {
        self.link.navigator()
    }

    fn location(&self) -> Option<yew_router::prelude::Location> {
        self.link.location()
    }

    fn route<R>(&self) -> Option<R>
    where
        R: yew_router::Routable + 'static,
    {
        self.link.route()
    }

    fn add_location_listener(
        &self,
        cb: Callback<yew_router::prelude::Location>,
    ) -> Option<yew_router::prelude::LocationHandle> {
        self.link.add_location_listener(cb)
    }

    fn add_navigator_listener(
        &self,
        cb: Callback<yew_router::prelude::Navigator>,
    ) -> Option<yew_router::prelude::NavigatorHandle> {
        self.link.add_navigator_listener(cb)
    }
}

impl<L: LoadableComponent + Sized> NavigationContextExt for LoadableComponentLink<L> {
    fn nav_context(&self) -> Option<pwt::state::NavigationContext> {
        self.link.nav_context()
    }

    fn full_path(&self) -> Option<String> {
        self.link.full_path()
    }

    fn push_relative_route(&self, path: &str) {
        self.link.push_relative_route(path)
    }
}

pub trait LoadableComponent: Sized {
    type Properties: Properties;
    type Message: 'static;
    type ViewState: 'static + PartialEq;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self;

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>>;

    #[allow(unused_variables)]
    fn update(&mut self, ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        true
    }

    #[allow(unused_variables)]
    fn changed(
        &mut self,
        ctx: &LoadableComponentContext<Self>,
        _old_props: &Self::Properties,
    ) -> bool {
        true
    }

    #[allow(unused_variables)]
    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        None
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html;

    #[allow(unused_variables)]
    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        None
    }

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
    TaskBaseUrl(AttrValue),
    Visible(bool),
    Spawn(Pin<Box<dyn Future<Output = ()>>>),
}

pub struct LoadableComponentMaster<L: LoadableComponent> {
    state: L,
    comp_state: LoadableComponentState,
    view_state: ViewState<L::ViewState>,
    reload_timeout: Option<Timeout>,
    visible: bool,
    visibitlity_observer: Option<DomVisibilityObserver>,
    node_ref: NodeRef,
    async_pool: AsyncPool,
}

impl<L: LoadableComponent + 'static> Component for LoadableComponentMaster<L> {
    type Message = Msg<L::Message, L::ViewState>;
    type Properties = L::Properties;

    fn create(ctx: &Context<Self>) -> Self {
        let loading = 0;

        let comp_state = LoadableComponentState {
            loading,
            last_load_error: None,
            repeat_timespan: 0,
            task_base_url: None,
        };

        let sub_context = LoadableComponentContext {
            ctx,
            comp_state: &comp_state,
        };

        // Send Msg::Load first (before any Msg::RepeatedLoad in create), so that we
        // can avoid multiple loads at startup
        ctx.link().send_message(Msg::Load);

        let state = L::create(&sub_context);

        Self {
            state,
            comp_state,
            view_state: ViewState::Main,
            reload_timeout: None,
            visible: true,
            visibitlity_observer: None,
            node_ref: NodeRef::default(),
            async_pool: AsyncPool::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Spawn(future) => {
                self.async_pool.spawn(future);
                false
            }
            Msg::DataChange => true,
            Msg::Load => {
                self.comp_state.loading += 1;
                let link = ctx.link().clone();
                let sub_context = LoadableComponentContext {
                    ctx,
                    comp_state: &self.comp_state,
                };
                let load_future = self.state.load(&sub_context);
                self.async_pool.spawn(async move {
                    let data = load_future.await;
                    link.send_message(Msg::LoadResult(data));
                });
                true
            }
            Msg::RepeatedLoad(timespan) => {
                self.comp_state.repeat_timespan = timespan;
                self.reload_timeout = None;
                if self.comp_state.loading == 0 {
                    <Self as yew::Component>::update(self, ctx, Msg::Load);
                }
                false
            }
            Msg::LoadResult(data) => {
                self.comp_state.loading -= 1;
                match data {
                    Ok(()) => {
                        self.comp_state.last_load_error = None;
                    }
                    Err(err) => {
                        let this_is_the_first_error = self.comp_state.last_load_error.is_none();
                        self.comp_state.last_load_error = Some(err.to_string());
                        if this_is_the_first_error {
                            self.view_state =
                                ViewState::Error(tr!("Load failed"), err.to_string(), false);
                        }
                    }
                }

                self.reload_timeout = None;
                if self.comp_state.loading == 0 {
                    /* no outstanding loads */
                    if self.comp_state.repeat_timespan > 0 {
                        let link = ctx.link().clone();
                        if self.visible {
                            self.reload_timeout =
                                Some(Timeout::new(self.comp_state.repeat_timespan, move || {
                                    link.send_message(Msg::Load);
                                }));
                        }
                    }
                }
                true
            }
            Msg::ChangeView(reload_data, view_state) => {
                if self.view_state == view_state {
                    return false;
                }

                if reload_data {
                    ctx.link().send_message(Msg::Load);
                }

                self.view_state = view_state;
                true
            }
            Msg::ChildMessage(child_msg) => {
                let sub_context = LoadableComponentContext {
                    ctx,
                    comp_state: &self.comp_state,
                };
                self.state.update(&sub_context, child_msg);
                true
            }
            Msg::TaskBaseUrl(base_url) => {
                self.comp_state.task_base_url = Some(base_url);
                false
            }
            Msg::Visible(visible) => {
                if self.visible == visible {
                    return false;
                }
                self.visible = visible;
                if self.comp_state.loading == 0 && self.visible {
                    /* no outstanding loads */
                    if self.comp_state.loading == 0 {
                        <Self as yew::Component>::update(self, ctx, Msg::Load);
                    }
                }
                false
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        let sub_context = LoadableComponentContext {
            ctx,
            comp_state: &self.comp_state,
        };

        self.state.changed(&sub_context, _old_props)
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let sub_context = LoadableComponentContext {
            ctx,
            comp_state: &self.comp_state,
        };

        let main_view = self.state.main_view(&sub_context);

        let dialog: Option<Html> =
            match &self.view_state {
                ViewState::Main => None,
                ViewState::Dialog(view_state) => self.state.dialog_view(&sub_context, view_state),
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

                    if let Some(base_url) = &self.comp_state.task_base_url {
                        task_progress.set_base_url(base_url);
                    }

                    Some(task_progress.into())
                }
                ViewState::TaskLog(task_id, endtime) => {
                    let mut task_viewer = TaskViewer::new(task_id).endtime(endtime).on_close(
                        ctx.link()
                            .callback(move |_| Msg::ChangeView(true, ViewState::Main)),
                    );

                    if let Some(base_url) = &self.comp_state.task_base_url {
                        task_viewer.set_base_url(base_url);
                    }
                    Some(task_viewer.into())
                }
            };

        let toolbar = self.state.toolbar(&sub_context);

        let mut alert_msg = None;

        if dialog.is_none() {
            if let Some(msg) = &self.comp_state.last_load_error {
                alert_msg = Some(pwt::widget::error_message(msg).class("pwt-border-top"));
            }
        }

        Column::new()
            .node_ref(self.node_ref.clone())
            .class("pwt-flex-fill pwt-overflow-auto")
            .with_optional_child(toolbar)
            .with_child(main_view)
            .with_optional_child(alert_msg)
            .with_optional_child(dialog)
            .into()
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if self.visibitlity_observer.is_none() && self.reload_timeout.is_some() {
            if let Some(el) = self.node_ref.cast::<web_sys::Element>() {
                self.visibitlity_observer = Some(DomVisibilityObserver::new(
                    &el,
                    ctx.link().callback(Msg::Visible),
                ))
            }
        }
        let sub_context = LoadableComponentContext {
            ctx,
            comp_state: &self.comp_state,
        };

        self.state.rendered(&sub_context, first_render);
    }
}
