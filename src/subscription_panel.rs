use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use serde_json::Value;

use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Field, FormContext};
use pwt::widget::{Button, InputPanel, Toolbar};

use crate::{ConfirmButton, EditWindow, KVGrid, KVGridRow, ProxmoxProduct};
use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster};
use crate::utils::render_epoch;

#[derive(Properties, PartialEq, Clone)]
pub struct SubscriptionPanel {
    product: ProxmoxProduct,
}

impl SubscriptionPanel {
    pub fn new(product: ProxmoxProduct) -> Self {
        Self { product }
    }
}

#[derive(PartialEq)]
pub enum ViewState {
    UploadSubscriptionKey,
}

pub enum Msg {}

pub struct ProxmoxSubscriptionPanel {
    rows: Rc<Vec<KVGridRow>>,
    data: Rc<RefCell<Rc<Value>>>,
}

impl LoadableComponent for ProxmoxSubscriptionPanel {
    type Message = Msg;
    type Properties = SubscriptionPanel;
    type ViewState = ViewState;

    fn create(_ctx: &LoadableComponentContext<Self>) -> Self {
        Self {
            rows: Rc::new(rows()),
            data: Rc::new(RefCell::new(Rc::new(Value::Null))),
        }
    }

    fn load(
        &self,
        _ctx: &crate::LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let data = self.data.clone();
        Box::pin(async move {
            let info = crate::http_get("/nodes/localhost/subscription", None).await?;
            *data.borrow_mut() = Rc::new(info);
            Ok(())
        })
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let toolbar = Toolbar::new()
            .class("pwt-overflow-hidden")
            .with_child(
                Button::new(tr!("Upload Subscription Key"))
                    .icon_class("fa fa-ticket")
                    .onclick(
                        ctx.link()
                            .change_view_callback(|_| Some(ViewState::UploadSubscriptionKey)),
                    ),
            )
            .with_child(Button::new(tr!("Check")).icon_class("fa fa-check-square-o"))
            .with_child(
                ConfirmButton::new(tr!("Remove Subscription"))
                    .icon_class("fa fa-trash-o")
                    .confirm_message(
                        html! {tr!("Are you sure you want to remove the subscription key?")},
                    )
                    .on_activate({
                        let link = ctx.link();
                        move |_| {
                            let link = link.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match crate::http_delete("/nodes/localhost/subscription", None)
                                    .await
                                {
                                    Ok(()) => link.change_view(None),
                                    Err(err) => {
                                        link.show_error(tr!("Error"), err.to_string(), true)
                                    }
                                }
                            })
                        }
                    }),
            )
            .with_spacer()
            .with_child(Button::new(tr!("System Report")).icon_class("fa fa-stethoscope"))
            .with_flex_spacer()
            .with_child({
                let loading = ctx.loading();
                let link = ctx.link();
                Button::refresh(loading).onclick(move |_| link.send_reload())
            });

        Some(toolbar.into())
    }

    fn main_view(&self, _ctx: &LoadableComponentContext<Self>) -> Html {
        KVGrid::new()
            .class("pwt-flex-fit")
            .data(self.data.borrow().clone())
            .rows(Rc::clone(&self.rows))
            .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        match view_state {
            ViewState::UploadSubscriptionKey => Some(self.create_upload_subscription_dialog(ctx)),
        }
    }
}

impl Into<VNode> for SubscriptionPanel {
    fn into(self) -> VNode {
        let comp =
            VComp::new::<LoadableComponentMaster<ProxmoxSubscriptionPanel>>(Rc::new(self), None);
        VNode::from(comp)
    }
}

fn rows() -> Vec<KVGridRow> {
    let unknown_text = tr!("unknown");
    let yes_text = tr!("Yes");
    let no_text = tr!("No");
    vec![
        KVGridRow::new("productname", tr!("Type")),
        KVGridRow::new("key", tr!("Subscription Key")),
        KVGridRow::new("status", tr!("Status"))
            .required(true)
            .renderer(move |_name, value, record| {
                let status = value.as_str().unwrap_or(&unknown_text).to_uppercase();

                let message = record["message"].as_str().unwrap_or("internal error");

                html! {format!("{}: {}", status, message)}
            }),
        KVGridRow::new("serverid", tr!("Server ID")).required(true),
        KVGridRow::new("checktime", tr!("Last checked"))
            .renderer(move |_name, value, _record| {
                match value.as_i64() {
                    Some(checktime) => html!{render_epoch(checktime)},
                    None => html!{"-"},
                }
            }),
        KVGridRow::new("nextduedata", tr!("Next due data")),
        KVGridRow::new("signature", tr!("Signed/Offline"))
            .renderer(move |_name, value, _record| {
                match value.as_bool() {
                    Some(true) => html!{&yes_text},
                    _ => html!{&no_text},
                }
            }),
        KVGridRow::new("url", tr!("Info URL")).renderer(|_name, value, _record| {
            let url = value.as_str().unwrap().to_string();
            html! { <a target="_blank" href={url.clone()}>{url}</a> }
        }),
    ]
}

impl ProxmoxSubscriptionPanel {
    fn create_upload_subscription_dialog(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let input_panel = |_form_state: &FormContext| -> Html {
            InputPanel::new()
                .class("pwt-p-4")
                .with_field(
                    tr!("Subscription Key"),
                    Field::new().name("key").required(true).autofocus(true),
                )
                .into()
        };

        EditWindow::new(tr!("Upload Subscription Key"))
            .renderer(input_panel)
            .on_submit(|form_state: FormContext| async move {
                let data = form_state.get_submit_data();
                crate::http_put("/nodes/localhost/subscription", Some(data)).await
            })
            .on_done(ctx.link().change_view_callback(|_| None))
            .into()
    }
}
