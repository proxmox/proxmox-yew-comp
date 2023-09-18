mod auth_view;
pub use auth_view::{AuthView, ProxmoxAuthView};

mod auth_edit_openid;
pub use auth_edit_openid::{AuthEditOpenID, ProxmoxAuthEditOpenID};

mod auth_edit_ldap;
pub use auth_edit_ldap::{AuthEditLDAP, ProxmoxAuthEditLDAP};

pub mod common_api_types;

mod http_client_wasm;
pub use  http_client_wasm::*;

mod http_helpers;
pub use http_helpers::*;

mod help_button;
pub use help_button::{HelpButton, PbsHelpButton};

mod calendar_event_selector;
pub use calendar_event_selector::CalendarEventSelector;

mod edit_window;
pub use edit_window::{EditWindow, PwtEditWindow};

mod key_value_grid;
pub use key_value_grid::{KVGrid, KVGridRow, PwtKVGrid, RenderKVGridRecordFn};

mod loadable_component;
pub use loadable_component::{LoadableComponent, LoadableComponentMaster, LoadableComponentContext};

mod object_grid;
pub use object_grid::{ObjectGrid, ObjectGridRow, PwtObjectGrid, RenderObjectGridItemFn};

mod permission_panel;
pub use permission_panel::{PermissionPanel, ProxmoxPermissionPanel};

mod realm_selector;
pub use realm_selector::RealmSelector;

mod role_selector;
pub use role_selector::RoleSelector;

mod rrd_graph_new;
pub use rrd_graph_new::{RRDGraph, Series};

mod rrd_grid;
pub use rrd_grid::RRDGrid;

mod rrd_timeframe_selector;
pub use rrd_timeframe_selector::{RRDTimeframe, RRDTimeframeSelector};

mod login_panel;
pub use login_panel::LoginPanel;

mod log_view;
pub use log_view::LogView;

mod journal_view;
pub use journal_view::JournalView;

mod schema_validation;
pub use schema_validation::*;

mod subscription_alert;
pub use subscription_alert::{SubscriptionAlert, ProxmoxSubscriptionAlert};

mod subscription_panel;
pub use subscription_panel::{SubscriptionPanel, PwtSubscriptionPanel};

mod syslog;
pub use syslog::{Syslog, ProxmoxSyslog};

mod tfa;
pub use tfa::{TfaDialog, PbsTfaDialog};

mod time_zone_selector;
pub use time_zone_selector::{TimezoneSelector, ProxmoxTimezoneSelector};

mod task_viewer;
pub use task_viewer::*;

mod task_progress;
pub use task_progress::TaskProgress;

mod task_status_selector;
pub use task_status_selector::{TaskStatusSelector, ProxmoxTaskStatusSelector};

mod task_type_selector;
pub use task_type_selector::{TaskTypeSelector, ProxmoxTaskTypeSelector};

mod tasks;
pub use tasks::{Tasks, ProxmoxTasks};

pub mod percent_encoding;

mod proxmox_product;
pub use proxmox_product::ProxmoxProduct;

pub mod utils;

// Bindgen javascript code from js-helper-module.js

#[cfg(target_arch="wasm32")]
use wasm_bindgen::{self, prelude::*};
#[wasm_bindgen(module = "/js-helper-module.js")]
#[cfg(target_arch="wasm32")]
extern "C" {
    pub fn async_sleep(ms: i32) -> js_sys::Promise;

    pub fn get_cookie() -> String;
    pub fn set_cookie(value: &str);
    pub fn clear_auth_cookie(name: &str);

    // uPlot binding
    pub fn uplot(opts: &JsValue, data: &JsValue, node: web_sys::Node) -> JsValue;
    pub fn uplot_set_data(uplot: &JsValue, data: &JsValue);
    pub fn uplot_set_size(uplot: &JsValue, width: usize, height: usize);
}

// Create wrapper which panics if called from target_arch!=wasm32
// This allows us to build with "cargo build" and run tests with "cargo test".
#[cfg(not(target_arch="wasm32"))]
pub use panic_wrapper::*;
#[cfg(not(target_arch="wasm32"))]
mod panic_wrapper {
    use wasm_bindgen::JsValue;
    pub fn async_sleep(_ms: i32) -> js_sys::Promise { unreachable!() }
    pub fn get_cookie() -> String { unreachable!() }
    pub fn set_cookie(_value: &str) { unreachable!() }
    pub fn clear_auth_cookie(_name: &str) { unreachable!() }
    pub fn uplot(_opts: &JsValue, _data: &JsValue, _node: web_sys::Node) -> JsValue { unreachable!() }
    pub fn uplot_set_data(_uplot: &JsValue, _data: &JsValue) { unreachable!() }
    pub fn uplot_set_size(_uplot: &JsValue, _width: usize, _height: usize) { unreachable!() }
}

pub fn store_csrf_token(crsf_token: &str) {
    if let Some(store) = pwt::state::session_storage() {
        if let Err(_) = store.set_item("CSRFToken", crsf_token) {
            log::error!("store_csrf_token: store.set_item() failed");
        }
    }
}

pub fn load_csrf_token() -> Option<String>{
    pwt::state::session_storage()
        .and_then(|store| store.get_item("CSRFToken").unwrap_or(None))
}
