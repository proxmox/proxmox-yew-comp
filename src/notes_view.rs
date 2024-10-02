use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoSubmitCallback, LoadCallback, SubmitCallback};
use pwt::widget::form::{FormContext, Hidden, TextArea};
use pwt::widget::{Button, Column, Container, Toolbar};

use proxmox_client::ApiResponseData;

use crate::{
    EditWindow, LoadableComponent, LoadableComponentContext, LoadableComponentMaster, Markdown,
};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct NotesWithDigest {
    notes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    digest: Option<Value>,
}

async fn load_pve_notes() -> Result<NotesWithDigest, Error> {
    let resp: ApiResponseData<Value> =
        crate::http_get_full("/nodes/localhost/config", None).await?;
    let notes = resp.data["description"].as_str().unwrap_or("").to_owned();
    let digest = resp.attribs.get("digest").cloned();
    Ok(NotesWithDigest { notes, digest })
}

async fn update_pve_notes(data: NotesWithDigest) -> Result<(), Error> {
    let mut param = json!({ "description": data.notes});
    if let Some(digest) = data.digest {
        param["digest"] = digest;
    }
    let _ = crate::http_put("/nodes/localhost/config", Some(param)).await?;
    Ok(())
}

use pwt_macros::builder;

#[derive(PartialEq, Properties)]
#[builder]
pub struct NotesView {
    /// The load callback
    pub loader: LoadCallback<NotesWithDigest>,

    /// Submit callback.
    #[builder_cb(IntoSubmitCallback, into_submit_callback, NotesWithDigest)]
    #[prop_or_default]
    pub on_submit: Option<SubmitCallback<NotesWithDigest>>,
}

impl NotesView {
    pub fn new(loader: impl Into<LoadCallback<NotesWithDigest>>) -> Self {
        let loader = loader.into();
        yew::props!(Self { loader })
    }

    pub fn pve_compatible() -> Self {
        let loader = LoadCallback::new(load_pve_notes);
        let on_submit = SubmitCallback::new(update_pve_notes);
        yew::props!(Self {
            loader,
            on_submit: Some(on_submit)
        })
    }
}

#[derive(PartialEq)]
pub enum ViewState {
    EditNotes,
}

pub enum Msg {
    Load(NotesWithDigest),
}

#[doc(hidden)]
pub struct ProxmoxNotesView {
    data: NotesWithDigest,
    edit_window_loader: LoadCallback<Value>,
}

impl LoadableComponent for ProxmoxNotesView {
    type Properties = NotesView;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let props = ctx.props();
        let loader = props.loader.clone();
        let edit_window_loader = LoadCallback::new(move || {
            let loader = loader.clone();
            async move {
                let data = loader.apply().await?;
                let data = serde_json::to_value(data)?;
                Ok(data)
            }
        });
        Self {
            data: NotesWithDigest {
                notes: String::new(),
                digest: None,
            },
            edit_window_loader,
        }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let loader = ctx.props().loader.clone();
        let link = ctx.link();
        Box::pin(async move {
            let data: NotesWithDigest = loader.apply().await?;
            link.send_message(Msg::Load(data));
            Ok(())
        })
    }

    fn update(&mut self, _ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Load(data) => {
                self.data = data;
                true
            }
        }
    }
    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let props = ctx.props();
        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(props.on_submit.is_none())
                    .onclick(
                        ctx.link()
                            .change_view_callback(|_| Some(ViewState::EditNotes)),
                    ),
            );

        Some(toolbar.into())
    }

    fn main_view(&self, _ctx: &LoadableComponentContext<Self>) -> Html {
        Container::new()
            .padding(2)
            .class("pwt-flex-fit")
            .class("pwt-embedded-html")
            .with_child(Markdown::new().text(self.data.notes.clone()))
            .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        let props = ctx.props();
        match view_state {
            ViewState::EditNotes => {
                let dialog = EditWindow::new(tr!("Edit") + ": " + &tr!("Notes"))
                    .width(800)
                    .height(400)
                    .on_done(ctx.link().change_view_callback(|_| None))
                    .resizable(true)
                    .loader(self.edit_window_loader.clone())
                    .on_submit({
                        let on_submit = props.on_submit.clone();
                        move |form_ctx: FormContext| {
                            let on_submit = on_submit.clone();
                            async move {
                                if let Some(on_submit) = &on_submit {
                                    let data = form_ctx.read().get_submit_data();
                                    let data: NotesWithDigest = serde_json::from_value(data)?;
                                    on_submit.apply(data).await?;
                                }
                                Ok(())
                            }
                        }
                    })
                    .renderer(|_form_ctx| {
                        Column::new()
                            .class(pwt::css::FlexFit)
                            .with_child(
                                TextArea::new()
                                    .padding(2)
                                    .name("notes")
                                    .submit_empty(true)
                                    .class(pwt::css::FlexFit),
                            )
                            .with_child(Hidden::new().name("digest").submit_empty(false))
                            .into()
                    });

                Some(dialog.into())
            }
        }
    }
}

impl Into<VNode> for NotesView {
    fn into(self) -> VNode {
        let comp = VComp::new::<LoadableComponentMaster<ProxmoxNotesView>>(Rc::new(self), None);
        VNode::from(comp)
    }
}
