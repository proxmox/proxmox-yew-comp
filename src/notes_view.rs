use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::props::{IntoSubmitCallback, SubmitCallback};
use pwt::widget::form::{FormContext, Hidden, TextArea};
use pwt::widget::{Button, Column, Container, Toolbar};

use proxmox_client::ApiResponseData;

use crate::{
    ApiLoadCallback, EditWindow, LoadableComponent, LoadableComponentContext,
    LoadableComponentMaster, Markdown,
};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct NotesWithDigest {
    notes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    digest: Option<Value>,
}

async fn load_notes_property(
    url: AttrValue,
    prop_name: AttrValue,
) -> Result<ApiResponseData<String>, Error> {
    let resp: ApiResponseData<Value> = crate::http_get_full(&*url, None).await?;
    let notes = resp.data[&*prop_name].as_str().unwrap_or("").to_owned();
    Ok(ApiResponseData {
        data: notes,
        attribs: resp.attribs,
    })
}

async fn update_notes_property(
    url: AttrValue,
    prop_name: AttrValue,
    data: NotesWithDigest,
) -> Result<(), Error> {
    let mut param = json!({ &*prop_name: data.notes});
    if let Some(digest) = data.digest {
        param["digest"] = digest;
    }
    let _ = crate::http_put(&*url, Some(param)).await?;
    Ok(())
}

use pwt_macros::builder;

#[derive(PartialEq, Properties)]
#[builder]
pub struct NotesView {
    /// The load callback
    pub loader: ApiLoadCallback<String>,

    /// Submit callback.
    #[builder_cb(IntoSubmitCallback, into_submit_callback, NotesWithDigest)]
    #[prop_or_default]
    pub on_submit: Option<SubmitCallback<NotesWithDigest>>,
}

impl NotesView {
    /// Create a new instance
    pub fn new(loader: impl Into<ApiLoadCallback<String>>) -> Self {
        let loader = loader.into();
        yew::props!(Self { loader })
    }
    /// Create a new instance, assume that notes are stored as object property.
    ///
    /// Automatically create a loader and on_submit callback.
    pub fn edit_property(url: impl Into<AttrValue>, prop_name: impl Into<AttrValue>) -> Self {
        let url = url.into();
        let prop_name = prop_name.into();

        let loader = ApiLoadCallback::new({
            let url = url.clone();
            let prop_name = prop_name.clone();
            move || load_notes_property(url.clone(), prop_name.clone())
        });
        let on_submit = SubmitCallback::new({
            let url = url.clone();
            let prop_name = prop_name.clone();
            move |data| update_notes_property(url.clone(), prop_name.clone(), data)
        });
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
    edit_window_loader: ApiLoadCallback<Value>,
}

impl LoadableComponent for ProxmoxNotesView {
    type Properties = NotesView;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let props = ctx.props();
        let loader = props.loader.clone();
        let edit_window_loader = ApiLoadCallback::new(move || {
            let loader = loader.clone();
            async move {
                let resp = loader.apply().await?;
                Ok(ApiResponseData {
                    data: json!({ "notes": resp.data }),
                    attribs: resp.attribs,
                })
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
            let resp = loader.apply().await?;
            let notes = resp.data;
            let digest = resp.attribs.get("digest").cloned();
            link.send_message(Msg::Load(NotesWithDigest { notes, digest }));
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
