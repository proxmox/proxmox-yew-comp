use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use serde_json::{json, Value};

use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::form::{Field, FormContext};
use pwt::widget::{Button, Container, InputPanel, Toolbar};

use crate::utils::render_epoch;
use crate::{ConfirmButton, DataViewWindow, EditWindow, KVGrid, KVGridRow, ProxmoxProduct};
use crate::{LoadableComponent, LoadableComponentContext, LoadableComponentMaster};

#[derive(Properties, PartialEq, Clone)]
pub struct SubscriptionPanel {
    product: ProxmoxProduct,
}

impl SubscriptionPanel {
    pub fn new(product: ProxmoxProduct) -> Self {
        yew::props!(Self { product })
    }
}

#[derive(PartialEq)]
pub enum ViewState {
    UploadSubscriptionKey,
    SystemReport,
}

pub enum Msg {}

pub struct ProxmoxSubscriptionPanel {
    rows: Rc<Vec<KVGridRow>>,
    data: Rc<RefCell<Rc<Value>>>,
}

fn base_url(product: ProxmoxProduct) -> AttrValue {
    match product {
        _ => AttrValue::Static("/nodes/localhost/subscription"),
    }
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
        ctx: &crate::LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let data = self.data.clone();
        let base_url = base_url(ctx.props().product);
        Box::pin(async move {
            let info = crate::http_get(&*base_url, None).await?;
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
            .with_child(
                Button::new(tr!("Check"))
                    .icon_class("fa fa-check-square-o")
                    .onclick({
                        let link = ctx.link();
                        let base_url = base_url(ctx.props().product);
                        move |_| {
                            let link = link.clone();
                            let base_url = base_url.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match crate::http_post(&*base_url, Some(json!({"force": true})))
                                    .await
                                {
                                    Ok(()) => link.send_reload(),
                                    Err(err) => {
                                        link.show_error(tr!("Error"), err.to_string(), true)
                                    }
                                }
                            })
                        }
                    }),
            )
            .with_child(
                ConfirmButton::new(tr!("Remove Subscription"))
                    .icon_class("fa fa-trash-o")
                    .confirm_message(
                        html! {tr!("Are you sure you want to remove the subscription key?")},
                    )
                    .on_activate({
                        let link = ctx.link();
                        let base_url = base_url(ctx.props().product);
                        move |_| {
                            let link = link.clone();
                            let base_url = base_url.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match crate::http_delete(&*base_url, None).await {
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
            .with_child(
                Button::new(tr!("System Report"))
                    .icon_class("fa fa-stethoscope")
                    .onclick(
                        ctx.link()
                            .change_view_callback(|_| Some(ViewState::SystemReport)),
                    ),
            )
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
            ViewState::SystemReport => Some(self.create_system_report_dialog(ctx)),
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
        KVGridRow::new("checktime", tr!("Last checked")).renderer(move |_name, value, _record| {
            match value.as_i64() {
                Some(checktime) => html! {render_epoch(checktime)},
                None => html! {"-"},
            }
        }),
        KVGridRow::new("nextduedata", tr!("Next due data")),
        KVGridRow::new("signature", tr!("Signed/Offline")).renderer(
            move |_name, value, _record| match value.as_str() {
                Some(_) => html! {&yes_text},
                _ => html! {&no_text},
            },
        ),
        KVGridRow::new("url", tr!("Info URL")).renderer(|_name, value, _record| {
            let url = value.as_str().unwrap().to_string();
            html! { <a target="_blank" href={url.clone()}>{url}</a> }
        }),
    ]
}

impl ProxmoxSubscriptionPanel {
    fn create_system_report_dialog(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        DataViewWindow::new(tr!("System Report"))
            .width(800)
            .height(600)
            .loader("/nodes/localhost/report")
            .renderer(|report: &String| {
                Container::new()
                    .tag("pre")
                    .class("pwt-flex-fit pwt-font-monospace")
                    .padding(2)
                    .style("line-height", "normal")
                    .with_child(report)
                    .into()
            })
            .on_done(ctx.link().change_view_callback(|_| None))
            .into()
    }

    fn create_upload_subscription_dialog(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let input_panel = |_form_state: &FormContext| -> Html {
            InputPanel::new()
                .padding(4)
                .with_field(
                    tr!("Subscription Key"),
                    Field::new().name("key").required(true).autofocus(true),
                )
                .into()
        };

        EditWindow::new(tr!("Upload Subscription Key"))
            .renderer(input_panel)
            .on_submit({
                let base_url = base_url(ctx.props().product);
                move |form_state: FormContext| {
                    let base_url = base_url.clone();
                    async move {
                        let data = form_state.get_submit_data();
                        crate::http_put(&*base_url, Some(data)).await
                    }
                }
            })
            .on_done(ctx.link().change_view_callback(|_| None))
            .into()
    }
}
