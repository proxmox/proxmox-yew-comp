mod http_client_wasm;
pub use  http_client_wasm::*;

mod http;
pub use http::*;

mod help_button;
pub use help_button::{HelpButton, PbsHelpButton};

mod calendar_event_selector;
pub use calendar_event_selector::CalendarEventSelector;

mod config_panel;
pub use config_panel::{PwtConfigPanel, ConfigPanel};

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

#[derive(PartialEq, Debug, Clone)]
pub enum ProxmoxProduct {
    PVE,
    PMG,
    PBS,
}

// Bindgen java code from js-helper-module.js

use wasm_bindgen::{self, prelude::*};
#[wasm_bindgen(module = "/js-helper-module.js")]
#[cfg(target_arch="wasm32")]
extern "C" {
    pub fn async_sleep(ms: i32) -> js_sys::Promise;

    pub fn get_cookie() -> String;
    pub fn set_auth_cookie(value: &str);
    pub fn clear_auth_cookie();

    // uPlot binding
    pub fn uplot(opts: &JsValue, data: &JsValue, node: web_sys::Node) -> JsValue;
    pub fn uplot_set_data(uplot: &JsValue, data: &JsValue);
    pub fn uplot_set_size(uplot: &JsValue, width: usize, height: usize);
   
    pub fn render_server_epoch(epoch: f64) -> String;
    pub fn render_timestamp(epoch: f64) -> String;
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
