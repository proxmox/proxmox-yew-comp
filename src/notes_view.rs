use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use pwt::props::LoadCallback;
use serde_json::Value;

use yew::html::IntoPropValue;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{FormContext, TextArea};
use pwt::widget::{Button, Container, Toolbar};

use crate::{
    EditWindow, LoadableComponent, LoadableComponentContext, LoadableComponentMaster, Markdown,
};

async fn update_item(form_ctx: FormContext, url: AttrValue) -> Result<Value, Error> {
    let data = form_ctx.get_submit_data();
    crate::http_put(&*url, Some(data)).await
}

use pwt_macros::builder;

#[derive(PartialEq, Properties)]
#[builder]
pub struct NotesView {
    #[prop_or("/nodes/localhost/config".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url for
    pub base_url: AttrValue,
}

impl NotesView {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[derive(PartialEq)]
pub enum ViewState {
    EditNotes,
}

pub enum Msg {
    Load(String),
}

#[doc(hidden)]
pub struct ProxmoxNotesView {
    text: AttrValue,
    loader: LoadCallback<Value>,
}

impl LoadableComponent for ProxmoxNotesView {
    type Properties = NotesView;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        Self {
            text: "".into(),
            loader: ctx.props().base_url.clone().into(),
        }
    }

    fn load(
        &self,
        ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let loader = self.loader.clone();
        let link = ctx.link();
        Box::pin(async move {
            let data: Value = loader.apply().await?;
            let text = data["description"].as_str().unwrap_or("").to_owned();
            link.send_message(Msg::Load(text));
            Ok(())
        })
    }

    fn update(&mut self, _ctx: &LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Load(text) => {
                self.text = text.into();
                true
            }
        }
    }
    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                Button::new(tr!("Edit")).onclick(
                    ctx.link()
                        .change_view_callback(|_| Some(ViewState::EditNotes)),
                ),
            );

        Some(toolbar.into())
    }

    fn main_view(&self, _ctx: &LoadableComponentContext<Self>) -> Html {
        Container::new()
            .class("pwt-flex-fit")
            .class("pwt-p-2")
            .class("pwt-embedded-html")
            .with_child(Markdown::new().text(self.text.clone()))
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
                    .style("width: 800px; height: 400px;")
                    .on_done(ctx.link().change_view_callback(|_| None))
                    .resizable(true)
                    .loader(self.loader.clone())
                    .on_submit({
                        let url = props.base_url.clone();
                        move |form_ctx: FormContext| update_item(form_ctx.clone(), url.clone())
                    })
                    .renderer(|_form_ctx| {
                        TextArea::new()
                            .padding(2)
                            .name("description")
                            .submit_empty(true)
                            .class("pwt-flex-fit")
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
