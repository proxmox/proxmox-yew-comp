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
}

impl XTermJs {
    /// Create a new terminal panel (iframe)
    pub fn new() -> Self {
        yew::props!(Self {})
    }

    /// Open a new terminal window.
    pub fn open_xterm_js_viewer(console_type: ConsoleType, node_name: &str) {
        let url = xtermjs_url(console_type, node_name);
        let target = "_blank";
        let features =
            "toolbar=no,location=no,status=no,menubar=no,resizable=yes,width=800,height=420";

        let window = web_sys::window().unwrap();

        match window.open_with_url_and_target_and_features(&url, target, features) {
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

#[derive(Copy, Clone, PartialEq)]
pub enum ConsoleType {
    KVM(u64),
    LXC(u64),
    UpgradeShell,
    LoginShell,
}

fn xtermjs_url(console_type: ConsoleType, node_name: &str) -> String {
    let console = match console_type {
        ConsoleType::KVM(_vmid) => "kvm",
        ConsoleType::LXC(_vmid) => "lxc",
        ConsoleType::UpgradeShell => "shell",
        ConsoleType::LoginShell => "shell",
    };

    let mut param = json!({
        "console": console,
        "xtermjs": 1,
        "node": node_name,
    });

    match console_type {
        ConsoleType::KVM(vmid) => {
            param["vmid"] = vmid.into();
        }
        ConsoleType::LXC(vmid) => {
            param["vmid"] = vmid.into();
        }
        ConsoleType::UpgradeShell => {
            param["cmd"] = "upgrade".into();
        }
        ConsoleType::LoginShell => {
            param["cmd"] = "login".into();
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
        let url = xtermjs_url(props.console_type, &*props.node_name);
        html! {<iframe class="pwt-flex-fit" src={format!("/{url}")}/>}
    }
}

impl Into<VNode> for XTermJs {
    fn into(self) -> VNode {
        let key = self.key.clone();
        let comp = VComp::new::<ProxmoxXTermJs>(Rc::new(self), key);
        VNode::from(comp)
    }
}
