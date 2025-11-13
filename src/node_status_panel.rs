use std::future::Future;
use std::rc::Rc;

use anyhow::Error;
use html::IntoPropValue;
use pwt::css::{AlignItems, ColorScheme, FlexFit};
use pwt::widget::form::DisplayField;
use yew::virtual_dom::{VComp, VNode};

use pwt::prelude::*;
use pwt::widget::{error_message, Fa, Panel, Row, Tooltip};
use pwt::widget::{Button, Dialog};
use pwt_macros::builder;

use proxmox_node_status::{NodePowerCommand, NodeStatus};

use crate::utils::copy_text_to_clipboard;
use crate::{
    http_get, http_post, node_info, ConfirmButton, LoadableComponent, LoadableComponentContext,
    LoadableComponentMaster,
};

#[derive(Properties, Clone, PartialEq)]
#[builder]
pub struct NodeStatusPanel {
    /// URL path to load the node's status from.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    status_base_url: Option<AttrValue>,

    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    fingerprint_button: bool,

    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    power_management_buttons: bool,
}

impl NodeStatusPanel {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

impl Default for NodeStatusPanel {
    fn default() -> Self {
        Self::new()
    }
}

enum Msg {
    Error(Error),
    Loaded(Rc<NodeStatus>),
    RebootOrShutdown(NodePowerCommand),
    Reload,
}

#[derive(PartialEq)]
enum ViewState {
    FingerprintDialog,
}

struct ProxmoxNodeStatusPanel {
    node_status: Option<Rc<NodeStatus>>,
    error: Option<Error>,
}

impl ProxmoxNodeStatusPanel {
    fn change_power_state(&self, ctx: &LoadableComponentContext<Self>, command: NodePowerCommand) {
        let Some(url) = ctx.props().status_base_url.clone() else {
            return;
        };
        let link = ctx.link().clone();

        ctx.link().spawn(async move {
            let data = Some(serde_json::json!({
                "command": command,
            }));

            match http_post(url.as_str(), data).await {
                Ok(()) => link.send_message(Msg::Reload),
                Err(err) => link.send_message(Msg::Error(err)),
            }
        });
    }

    fn fingerprint_dialog(
        &self,
        ctx: &LoadableComponentContext<Self>,
        fingerprint: &str,
    ) -> Dialog {
        let link = ctx.link();
        let link_button = ctx.link();
        let fingerprint = fingerprint.to_owned();

        Dialog::new(tr!("Fingerprint"))
            .resizable(true)
            .min_width(500)
            .on_close(move |_| link.change_view(None))
            .with_child(
                Row::new()
                    .gap(2)
                    .margin_start(2)
                    .margin_end(2)
                    .with_child(
                        DisplayField::new()
                            .class(pwt::css::FlexFit)
                            .value(fingerprint.clone())
                            .border(true),
                    )
                    .with_child(
                        Tooltip::new(
                            Button::new_icon("fa fa-clipboard")
                                .class(ColorScheme::Primary)
                                .on_activate(move |_| copy_text_to_clipboard(&fingerprint)),
                        )
                        .tip(tr!("Copy token secret to clipboard.")),
                    ),
            )
            .with_child(
                Row::new()
                    .padding(2)
                    .with_flex_spacer()
                    .with_child(
                        Button::new(tr!("OK")).on_activate(move |_| link_button.change_view(None)),
                    )
                    .with_flex_spacer(),
            )
    }
}

impl LoadableComponent for ProxmoxNodeStatusPanel {
    type Message = Msg;
    type ViewState = ViewState;
    type Properties = NodeStatusPanel;

    fn create(ctx: &crate::LoadableComponentContext<Self>) -> Self {
        ctx.link().repeated_load(5000);

        Self {
            node_status: None,
            error: None,
        }
    }

    fn load(
        &self,
        ctx: &crate::LoadableComponentContext<Self>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let url = ctx.props().status_base_url.clone();
        let link = ctx.link().clone();

        Box::pin(async move {
            if let Some(url) = url {
                match http_get(url.as_str(), None).await {
                    Ok(res) => link.send_message(Msg::Loaded(Rc::new(res))),
                    Err(err) => link.send_message(Msg::Error(err)),
                }
            }
            Ok(())
        })
    }

    fn update(&mut self, ctx: &crate::LoadableComponentContext<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Error(err) => {
                self.error = Some(err);
                true
            }
            Msg::Loaded(status) => {
                self.node_status = Some(status);
                self.error = None;
                true
            }
            Msg::RebootOrShutdown(command) => {
                self.change_power_state(ctx, command);
                false
            }
            Msg::Reload => true,
        }
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        if view_state == &ViewState::FingerprintDialog {
            if let Some(ref node_status) = self.node_status {
                return Some(
                    self.fingerprint_dialog(ctx, &node_status.info.fingerprint)
                        .into(),
                );
            }
        }
        None
    }

    fn main_view(&self, ctx: &crate::LoadableComponentContext<Self>) -> Html {
        let status = self
            .node_status
            .as_ref()
            .map(|r| crate::NodeStatus::Common(r));

        let mut panel = Panel::new()
            .border(false)
            .class(FlexFit)
            .title(
                Row::new()
                    .class(AlignItems::Center)
                    .gap(2)
                    .with_child(Fa::new("book"))
                    .with_child(tr!("Node Status"))
                    .into_html(),
            )
            .with_child(node_info(status))
            .with_optional_child(self.error.as_ref().map(|e| error_message(&e.to_string())));

        if ctx.props().power_management_buttons {
            panel.add_tool(
                ConfirmButton::new(tr!("Reboot"))
                    .confirm_message(tr!("Are you sure you want to reboot the node?"))
                    .on_activate(
                        ctx.link()
                            .callback(|_| Msg::RebootOrShutdown(NodePowerCommand::Reboot)),
                    )
                    .icon_class("fa fa-undo"),
            );
            panel.add_tool(
                ConfirmButton::new(tr!("Shutdown"))
                    .confirm_message(tr!("Are you sure you want to shut down the node?"))
                    .on_activate(
                        ctx.link()
                            .callback(|_| Msg::RebootOrShutdown(NodePowerCommand::Shutdown)),
                    )
                    .icon_class("fa fa-power-off"),
            );
        }

        if ctx.props().fingerprint_button {
            panel.add_tool(
                Button::new(tr!("Show Fingerprint"))
                    .icon_class("fa fa-hashtag")
                    .class(ColorScheme::Primary)
                    .on_activate(
                        ctx.link()
                            .change_view_callback(|_| ViewState::FingerprintDialog),
                    ),
            );
        }

        panel.into()
    }
}

impl From<NodeStatusPanel> for VNode {
    fn from(value: NodeStatusPanel) -> Self {
        VComp::new::<LoadableComponentMaster<ProxmoxNodeStatusPanel>>(Rc::new(value), None).into()
    }
}
