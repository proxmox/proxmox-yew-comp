mod http_client_wasm;
pub use  http_client_wasm::*;

mod http;
pub use http::*;

mod help_button;
pub use help_button::{HelpButton, PbsHelpButton};

mod calendar_event_selector;
pub use calendar_event_selector::CalendarEventSelector;

mod edit_window;
pub use edit_window::{EditWindow, PwtEditWindow};

mod key_value_grid;
pub use key_value_grid::{KVGrid, KVGridRow, PwtKVGrid, RenderKVGridRecordFn};

mod object_grid;
pub use object_grid::{ObjectGrid, ObjectGridRow, PwtObjectGrid, RenderObjectGridItemFn};

mod realm_selector;
pub use realm_selector::RealmSelector;

mod rrd_graph;
pub use rrd_graph::RRDGraph;

mod login_panel;
pub use login_panel::LoginPanel;

mod log_view;
pub use log_view::LogView;

mod journal_view;
pub use journal_view::JournalView;

mod subscription_panel;
pub use subscription_panel::{SubscriptionPanel, PwtSubscriptionPanel};

mod time_zone_selector;
pub use time_zone_selector::{TimezoneSelector, ProxmoxTimezoneSelector};

mod task_viewer;
pub use task_viewer::*;

pub mod percent_encoding;

mod proxmox_product;
pub use proxmox_product::ProxmoxProduct;

// Bindgen javascript code from js-helper-module.js

#[cfg(target_arch="wasm32")]
use wasm_bindgen::{self, prelude::*};
#[wasm_bindgen(module = "/js-helper-module.js")]
#[cfg(target_arch="wasm32")]
extern "C" {
    pub fn async_sleep(ms: i32) -> js_sys::Promise;

    pub fn get_cookie() -> String;
    pub fn set_auth_cookie(name: &str, value: &str);
    pub fn clear_auth_cookie(name: &str);

    // uPlot binding
    pub fn uplot(opts: &JsValue, data: &JsValue, node: web_sys::Node) -> JsValue;
    pub fn uplot_set_data(uplot: &JsValue, data: &JsValue);
    pub fn uplot_set_size(uplot: &JsValue, width: usize, height: usize);

    pub fn render_server_epoch(epoch: f64) -> String;
    pub fn render_timestamp(epoch: f64) -> String;
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
    pub fn set_auth_cookie(_name: &str,_value: &str) { unreachable!() }
    pub fn clear_auth_cookie(_name: &str) { unreachable!() }
    pub fn uplot(_opts: &JsValue, _data: &JsValue, _node: web_sys::Node) -> JsValue { unreachable!() }
    pub fn uplot_set_data(_uplot: &JsValue, _data: &JsValue) { unreachable!() }
    pub fn uplot_set_size(_uplot: &JsValue, _width: usize, _height: usize) { unreachable!() }
    pub fn render_server_epoch(_epoch: f64) -> String { unreachable!() }
    pub fn render_timestamp(_epoch: f64) -> String { unreachable!() }
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
