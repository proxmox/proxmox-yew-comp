use std::rc::Rc;

use serde_json::Value;

use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::state::Loader;
use pwt::widget::{Button, InputPanel, Panel, Toolbar};
use pwt::widget::form::{Field, FormContext};

use crate::{HelpButton, EditWindow, KVGrid, KVGridRow, ProxmoxProduct};

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
}

pub enum Msg {
    DataChange,
    ChangeView(ViewState),
}

pub struct PwtSubscriptionPanel {
    loader: Loader<Value>,
    view_state: ViewState,
}

impl Component for PwtSubscriptionPanel {
    type Message = Msg;
    type Properties = SubscriptionPanel;

    fn create(ctx: &Context<Self>) -> Self {
        let loader = Loader::new(ctx.link().callback(|_| Msg::DataChange))
            .loader("/nodes/localhost/subscription");

        loader.load();

        Self {
            loader,
            view_state: ViewState::Main,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::DataChange => true,
            Msg::ChangeView(view_state) => {
                self.view_state = view_state;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {

        let toolbar = self.create_toolbar(ctx);

        let main_view = self.loader.render(|data| {
            self.create_main_view(ctx, &data)
        });

        let dialog = match self.view_state {
            ViewState::Main => None,
            ViewState::UploadSubscriptionKey => {
                Some(self.create_upload_subscription_dialog(ctx))
            }
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
        let comp = VComp::new::<PwtSubscriptionPanel>(Rc::new(self), None);
        VNode::from(comp)
    }
}

thread_local!{
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
impl PwtSubscriptionPanel {

    fn create_toolbar(&self, ctx: &Context<Self>) -> Html {
        Toolbar::new()
            .border_bottom(true)
            .with_child({
                let link = ctx.link().clone();
                Button::new("Upload Subscription Key").icon_class("fa fa-ticket")
                    .onclick(move |_| {
                        link.send_message(Msg::ChangeView(ViewState::UploadSubscriptionKey));
                    })
            })
            .with_child(
                Button::new("Check").icon_class("fa fa-check-square-o")
            )
            .with_child(
                Button::new("Remove Subscription").icon_class("fa fa-trash-o")
            )
            .with_spacer()
            .with_child(
                Button::new("System Report").icon_class("fa fa-stethoscope")
            )
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
                .class("pwt-p-2")
                .with_field(
                    "Subscription Key",
                    Field::new()
                        .name("key")
                        .required(true)
                        .autofocus(true)
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
