use std::rc::Rc;

use serde_json::Value;

use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::state::Loader;
use pwt::widget::form::{Field, FormContext};
use pwt::widget::{AlertDialog, Button, InputPanel, Panel, Toolbar};

use crate::{EditWindow, HelpButton, KVGrid, KVGridRow, ProxmoxProduct, ConfirmButton};

#[derive(Properties, PartialEq, Clone)]
pub struct SubscriptionPanel {
    product: ProxmoxProduct,
}

impl SubscriptionPanel {
    pub fn new(product: ProxmoxProduct) -> Self {
        Self { product }
    }
}

pub enum ViewState {
    Main,
    UploadSubscriptionKey,
    Error(String),
}

pub enum Msg {
    Load,
    DataChange,
    ChangeView(ViewState),
}

pub struct ProxmoxSubscriptionPanel {
    loader: Loader<Value>,
    view_state: ViewState,
}

impl Component for ProxmoxSubscriptionPanel {
    type Message = Msg;
    type Properties = SubscriptionPanel;

    fn create(ctx: &Context<Self>) -> Self {
        let loader = Loader::new()
            .loader("/nodes/localhost/subscription")
            .on_change(ctx.link().callback(|_| Msg::DataChange));

        loader.load();

        Self {
            loader,
            view_state: ViewState::Main,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Load => {
                self.loader.load();
                false
            }
            Msg::DataChange => true,
            Msg::ChangeView(view_state) => {
                self.view_state = view_state;
                self.loader.load();
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let toolbar = self.create_toolbar(ctx);

        let main_view = self.loader.render(|data| self.create_main_view(ctx, &data));

        let dialog = match &self.view_state {
            ViewState::Main => None,
            ViewState::UploadSubscriptionKey => Some(self.create_upload_subscription_dialog(ctx)),
            ViewState::Error(msg) => Some(
                AlertDialog::new(msg)
                    .title(tr!("Error"))
                    .on_close(
                        ctx.link()
                            .callback(move |_| Msg::ChangeView(ViewState::Main)),
                    )
                    .into(),
            ),
        };

        Panel::new()
            .class("pwt-fit")
            .border(false)
            .title("Subscription")
            .with_tool(HelpButton::new().section("subscription"))
            .with_child(toolbar)
            .with_child(main_view)
            .with_optional_child(dialog)
            .into()
    }
}

impl Into<VNode> for SubscriptionPanel {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxSubscriptionPanel>(Rc::new(self), None);
        VNode::from(comp)
    }
}

thread_local! {
    static ROWS: Rc<Vec<KVGridRow>> = Rc::new(vec![
        KVGridRow::new("productname", "Type"),
        KVGridRow::new("key", "Subscription Key"),
        KVGridRow::new("status", "Status")
            .required(true)
            .renderer(|_name, value, record| {
                let status = value.as_str()
                    .unwrap_or("unknown")
                    .to_uppercase();

                let message = record["message"].as_str()
                    .unwrap_or("internal error");

                html!{format!("{}: {}", status, message)}
            }),
        KVGridRow::new("serverid", "Server ID")
            .required(true),
        KVGridRow::new("checktime", "Last checked"),//fixme: renderer
        KVGridRow::new("nextduedata", "Next due data"),
        KVGridRow::new("url", "Info URL")
            .renderer(|_name, value, _record| {
                let url = value.as_str().unwrap().to_string();
                html!{ <a target="_blank" href={url.clone()}>{url}</a> }
            }),
    ]);
}

impl ProxmoxSubscriptionPanel {
    fn create_toolbar(&self, ctx: &Context<Self>) -> Html {
        Toolbar::new()
            .class("pwt-overflow-hidden")
            .with_child({
                let link = ctx.link().clone();
                Button::new("Upload Subscription Key")
                    .icon_class("fa fa-ticket")
                    .onclick(move |_| {
                        link.send_message(Msg::ChangeView(ViewState::UploadSubscriptionKey));
                    })
            })
            .with_child(Button::new("Check").icon_class("fa fa-check-square-o"))
            .with_child(
                ConfirmButton::new("Remove Subscription")
                    .icon_class("fa fa-trash-o")
                    .confirm_message(html!{tr!("Are you sure you want to remove the subscription key?")})
                    .on_activate(ctx.link().callback_future(move |_| async move {
                        match crate::http_delete("/nodes/localhost/subscription", None).await {
                            Ok(()) => Msg::ChangeView(ViewState::Main),
                            Err(err) => Msg::ChangeView(ViewState::Error(err.to_string())),
                        }
                    })),
            )
            .with_spacer()
            .with_child(Button::new("System Report").icon_class("fa fa-stethoscope"))
            .with_flex_spacer()
            .with_child(self.loader.reload_button())
            .into()
    }

    fn create_main_view(&self, _ctx: &Context<Self>, data: &Rc<Value>) -> Html {
        KVGrid::new()
            .data(data.clone())
            .rows(ROWS.with(Rc::clone))
            .into()
    }

    fn create_upload_subscription_dialog(&self, ctx: &Context<Self>) -> Html {
        let input_panel = |_form_state: &FormContext| -> Html {
            InputPanel::new()
                .class("pwt-p-4")
                .with_field(
                    "Subscription Key",
                    Field::new().name("key").required(true).autofocus(true),
                )
                .into()
        };

        EditWindow::new("Upload Subscription Key")
            .renderer(input_panel)
            .on_submit(|form_state: FormContext| async move {
                let data = form_state.get_submit_data();
                crate::http_put("/nodes/localhost/subscription", Some(data)).await
            })
            .on_done(ctx.link().callback(|_| Msg::ChangeView(ViewState::Main)))
            .into()
    }
}
