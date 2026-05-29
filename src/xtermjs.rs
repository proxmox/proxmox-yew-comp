use std::rc::Rc;

use serde_json::json;

use yew::html::IntoPropValue;
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt_macros::builder;

use crate::json_object_to_query;

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct XTermJs {
    /// Yew key property.
    #[prop_or_default]
    pub key: Option<Key>,

    #[prop_or("localhost".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The node name.
    pub node_name: AttrValue,

    #[prop_or(ConsoleType::LoginShell)]
    #[builder(IntoPropValue, into_prop_value)]
    pub console_type: ConsoleType,

    /// Use NoVNC instead of XtermJS
    #[prop_or_default]
    #[builder]
    pub vnc: bool,
}

impl Default for XTermJs {
    fn default() -> Self {
        Self::new()
    }
}

impl XTermJs {
    /// Create a new terminal panel (iframe)
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    // FIXME: separate noVNC and xterm.js, this is not a nice interface!
    /// Open a new terminal window.
    pub fn open_xterm_js_viewer(console_type: ConsoleType, node_name: &str, vnc: bool) {
        let url = xtermjs_url(&console_type, node_name, vnc);
        let target = "_blank";
        let features =
            "toolbar=no,location=no,status=no,menubar=no,resizable=yes,width=800,height=420";

        match gloo_utils::window().open_with_url_and_target_and_features(&url, target, features) {
            Ok(Some(new_window)) => {
                let _ = new_window.focus();
            }
            Ok(None) => {
                log::error!("unable to open window");
            }
            Err(err) => {
                log::error!("unable to open window: {err:?}");
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum ConsoleType {
    KVM(u64),
    LXC(u64),
    UpgradeShell,
    LoginShell,
    RemotePveLoginShell(String),
    RemotePbsLoginShell(String),
    RemotePveLXC(String, u64),
    RemotePveKVM(String, u64),
}

fn xtermjs_url(console_type: &ConsoleType, node_name: &str, vnc: bool) -> String {
    let mut param = json!({
        "node": node_name,
    });

    if vnc {
        param["novnc"] = 1.into();
        //param["resize"] = "scale".into();
    } else {
        param["xtermjs"] = 1.into();
    }

    match console_type {
        ConsoleType::KVM(vmid) => {
            param["console"] = "kvm".into();
            param["vmid"] = (*vmid).into();
        }
        ConsoleType::LXC(vmid) => {
            param["console"] = "lxc".into();
            param["vmid"] = (*vmid).into();
        }
        ConsoleType::UpgradeShell => {
            param["console"] = "upgrade".into();
        }
        ConsoleType::LoginShell => {
            param["console"] = "shell".into();
            param["cmd"] = "login".into();
        }
        ConsoleType::RemotePbsLoginShell(remote_name) => {
            param["console"] = "shell".into();
            param["cmd"] = "login".into();
            param["remote-type"] = "pbs".into();
            param["remote"] = remote_name.as_str().into();
        }
        ConsoleType::RemotePveLoginShell(remote_name) => {
            param["console"] = "shell".into();
            param["cmd"] = "login".into();
            param["remote-type"] = "pve".into();
            param["remote"] = remote_name.as_str().into();
        }
        ConsoleType::RemotePveLXC(remote_name, vmid) => {
            param["console"] = "lxc".into();
            param["remote-type"] = "pve".into();
            param["remote"] = remote_name.as_str().into();
            param["vmid"] = (*vmid).into();
        }
        ConsoleType::RemotePveKVM(remote_name, vmid) => {
            param["console"] = "kvm".into();
            param["remote-type"] = "pve".into();
            param["remote"] = remote_name.as_str().into();
            param["vmid"] = (*vmid).into();
        }
    }

    format!("?{}", json_object_to_query(param).unwrap())
}

pub struct ProxmoxXTermJs {}

impl Component for ProxmoxXTermJs {
    type Message = ();
    type Properties = XTermJs;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let url = xtermjs_url(&props.console_type, &props.node_name, props.vnc);
        html! {<iframe class="pwt-flex-fit" src={format!("/{url}")}/>}
    }
}

impl From<XTermJs> for VNode {
    fn from(val: XTermJs) -> Self {
        let key = val.key.clone();
        let comp = VComp::new::<ProxmoxXTermJs>(Rc::new(val), key);
        VNode::from(comp)
    }
}
