use anyhow::Error;
use std::future::Future;
use std::pin::Pin;

use gloo_timers::callback::Timeout;

use yew::html::Scope;

use pwt::prelude::*;
use pwt::widget::{AlertDialog, Column};

use crate::TaskProgress;

pub struct LoadableComponentState {
    loading: usize,
    last_load_error: Option<String>,
    repeat_timespan: u32, /* 0 => no repeated loading */
}

pub struct LoadableComponentContext<'a, L: LoadableComponent + Sized + 'static> {
    ctx: &'a Context<LoadableComponentMaster<L>>,
    comp_state: &'a LoadableComponentState,
}

impl<'a, L: LoadableComponent + Sized> LoadableComponentContext<'a, L> {
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

    pub fn send_future<Fut, M>(&self, future: Fut)
    where
        M: Into<L::Message>,
        Fut: Future<Output = M> + 'static,
    {
        let link = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let message: L::Message = future.await.into();
            link.send_message(message);
        });
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

    pub fn show_error(
        &self,
        title: impl Into<String>,
        msg: impl std::fmt::Display,
        reload_on_close: bool,
    ) {
        let view_state = ViewState::Error(title.into(), msg.to_string(), reload_on_close);
        self.link.send_message(Msg::ChangeView(false, view_state));
    }

    pub fn show_task(&self, task_id: impl Into<String>) {
        let view_state = ViewState::Task(task_id.into());
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

    pub fn start_task(&self, command_path: impl Into<String>) {
        let command_path: String = command_path.into();
        let link = self.clone();
        let command_future = crate::http_post::<String>(command_path, None);
        wasm_bindgen_futures::spawn_local(async move {
            match command_future.await {
                Ok(task_id) => {
                    link.send_reload();
                    link.show_task(task_id);
                }
                Err(err) => {
                    log::error!("error {err}");
                    link.show_error("Start command failed", err, true);
                }
            }
        });
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

    fn update(&mut self, _ctx: &LoadableComponentContext<Self>, _msg: Self::Message) -> bool {
        true
    }

    fn toolbar(&self, _ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        None
    }

    fn main_view(&self, _ctx: &LoadableComponentContext<Self>) -> Html;

    fn dialog_view(
        &self,
        _ctx: &LoadableComponentContext<Self>,
        _view_state: &Self::ViewState,
    ) -> Option<Html> {
        None
    }
}

#[derive(Clone, PartialEq)]
pub enum ViewState<V: PartialEq> {
    Main,
    /// Show the dialog returned by dialog_view
    Dialog(V),
    /// Show proxmox api task status
    Task(String),
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
}

pub struct LoadableComponentMaster<L: LoadableComponent> {
    state: L,
    comp_state: LoadableComponentState,
    view_state: ViewState<L::ViewState>,
    reload_timeout: Option<Timeout>,
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
        };

        let sub_context = LoadableComponentContext {
            ctx,
            comp_state: &comp_state,
        };

        let state = L::create(&sub_context);

        ctx.link().send_message(Msg::Load);

        Self {
            state,
            comp_state,
            view_state: ViewState::Main,
            reload_timeout: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::DataChange => true,
            Msg::Load => {
                self.comp_state.loading += 1;
                let link = ctx.link().clone();
                let sub_context = LoadableComponentContext {
                    ctx,
                    comp_state: &self.comp_state,
                };
                let load_future = self.state.load(&sub_context);
                wasm_bindgen_futures::spawn_local(async move {
                    let data = load_future.await;
                    link.send_message(Msg::LoadResult(data));
                });
                true
            }
            Msg::RepeatedLoad(timespan) => {
                self.comp_state.repeat_timespan = timespan;
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
                                ViewState::Error("Load failed".into(), err.to_string(), false);
                        }
                    }
                }

                self.reload_timeout = None;
                if self.comp_state.loading == 0 {
                    /* no outstanding loads */
                    if self.comp_state.repeat_timespan > 0 {
                        let link = ctx.link().clone();
                        self.reload_timeout =
                            Some(Timeout::new(self.comp_state.repeat_timespan, move || {
                                link.send_message(Msg::Load);
                            }));
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
        }
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
                ViewState::Task(task_id) => Some(
                    TaskProgress::new(task_id)
                        .on_close(
                            ctx.link()
                                .callback(move |_| Msg::ChangeView(true, ViewState::Main)),
                        )
                        .into(),
                ),
            };

        let toolbar = self.state.toolbar(&sub_context);

        let mut alert_msg = None;

        if dialog.is_none() {
            if let Some(msg) = &self.comp_state.last_load_error {
                alert_msg = Some(pwt::widget::error_message(&msg, "pwt-border-top"));
            }
        }

        Column::new()
            .class("pwt-flex-fill pwt-overflow-auto")
            .with_optional_child(toolbar)
            .with_child(main_view)
            .with_optional_child(alert_msg)
            .with_optional_child(dialog)
            .into()
    }
}
