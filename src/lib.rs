pub mod acme;

mod apt_package_manager;
pub use apt_package_manager::{AptPackageManager, ProxmoxAptPackageManager};

pub(crate) mod apt_api_types;

mod apt_repositories;
pub use apt_repositories::{AptRepositories, ProxmoxAptRepositories};

mod auth_view;
pub use auth_view::{AuthView, ProxmoxAuthView};

mod auth_edit_openid;
pub use auth_edit_openid::{AuthEditOpenID, ProxmoxAuthEditOpenID};

mod auth_edit_ldap;
pub use auth_edit_ldap::{AuthEditLDAP, ProxmoxAuthEditLDAP};

mod authid_selector;
pub use authid_selector::AuthidSelector;

mod bandwidth_selector;
pub use bandwidth_selector::{BandwidthSelector, ProxmoxBandwidthSelector};

mod bond_mode_selector;
pub use bond_mode_selector::{BondModeSelector, ProxmoxBondModeSelector};

mod bond_xmit_hash_policy_selector;
pub use bond_xmit_hash_policy_selector::{BondXmitHashPolicySelector, ProxmoxBondXmitHashPolicySelector};

pub mod common_api_types;

mod confirm_button;
pub use confirm_button::{ConfirmButton, ProxmoxConfirmButton};
pub use confirm_button::default_confirm_remove_message;

mod data_view_window;
pub use data_view_window::{DataViewWindow, ProxmoxDataViewWindow};

pub mod gauge;
pub use gauge::{Gauge, ProxmoxGauge};

mod http_client_wasm;
pub use  http_client_wasm::*;

mod http_helpers;
pub use http_helpers::*;

mod help_button;
pub use help_button::{HelpButton, PbsHelpButton};

mod calendar_event_selector;
pub use calendar_event_selector::CalendarEventSelector;

pub mod configuration;

mod edit_window;
pub use edit_window::{EditWindow, PwtEditWindow};

mod key_value_grid;
pub use key_value_grid::{KVGrid, KVGridRow, PwtKVGrid, RenderKVGridRecordFn};

mod loadable_component;
pub use loadable_component::{LoadableComponent, LoadableComponentMaster, LoadableComponentContext, LoadableComponentLink};

mod notes_view;
pub use notes_view::{NotesView, ProxmoxNotesView};

mod object_grid;
pub use object_grid::{ObjectGrid, ObjectGridController, ObjectGridRow, PwtObjectGrid, RenderObjectGridItemFn};

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

mod running_tasks;
pub use running_tasks::{RunningTasks, ProxmoxRunningTasks};

mod running_tasks_button;
pub use running_tasks_button::{RunningTasksButton, ProxmoxRunningTasksButton};

mod safe_confirm_dialog;
pub use safe_confirm_dialog::{SafeConfirmDialog, ProxmoxSafeConfirmDialog};

mod language_dialog;
pub use language_dialog::{LanguageDialog, ProxmoxLanguageDialog};

mod login_panel;
pub use login_panel::LoginPanel;

mod log_view;
pub use log_view::LogView;

mod markdown;
pub use markdown::{Markdown, ProxmoxMarkdown};

mod journal_view;
pub use journal_view::JournalView;

mod meter_label;
pub use meter_label::{MeterLabel, ProxmoxMeterLabel};

mod sanitize_html;
pub use sanitize_html::sanitize_html;

mod schema_validation;
pub use schema_validation::*;

mod status_row;
pub use status_row::{StatusRow, ProxmoxStatusRow};

mod subscription_alert;
pub use subscription_alert::{SubscriptionAlert, ProxmoxSubscriptionAlert};

mod subscription_panel;
pub use subscription_panel::{SubscriptionPanel, ProxmoxSubscriptionPanel};

mod subscription_info;
pub use subscription_info::{SubscriptionInfo, ProxmoxSubscriptionInfo};

mod syslog;
pub use syslog::{Syslog, ProxmoxSyslog};

pub mod tfa;

mod time_zone_selector;
pub use time_zone_selector::{TimezoneSelector, ProxmoxTimezoneSelector};

mod theme_dialog;
pub use theme_dialog:: {ThemeDialog, ProxmoxThemeDialog};

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
pub use proxmox_product::{ExistingProduct, ProjectInfo};

pub mod utils;

use pwt::gettext_noop;
use pwt::state::LanguageInfo;

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

/// Returns the list of available languages for Proxmox Products.
///
/// # Note
///
/// All Proxmox products should support those languages.
pub fn available_language_list() -> Vec<LanguageInfo> {
    vec![
        LanguageInfo::new("ar", "العربية", gettext_noop("Arabic")),
        LanguageInfo::new("ca", "Català", gettext_noop("Catalan")),
        LanguageInfo::new("da", "Dansk", gettext_noop("Danish")),
        LanguageInfo::new("de", "Deutsch", gettext_noop("German")),
        LanguageInfo::new("en", "English", gettext_noop("English")),
        LanguageInfo::new("es", "Español", gettext_noop("Spanish")),
        LanguageInfo::new("eu", "Euskera (Basque)", gettext_noop("Euskera (Basque)")),
        LanguageInfo::new("fa", "فارسی", gettext_noop("Persian (Farsi)")),
        LanguageInfo::new("fr", "Français", gettext_noop("French")),
        LanguageInfo::new("he", "עברית", gettext_noop("Hebrew")),
        LanguageInfo::new("it", "Italiano", gettext_noop("Italian")),
        LanguageInfo::new("ja", "日本語", gettext_noop("Japanese")),
        LanguageInfo::new("kr", "한국어", gettext_noop("Korean")),
        LanguageInfo::new("nb", "Bokmål", gettext_noop("Norwegian (Bokmal)")),
        LanguageInfo::new("nl", "Nederlands", gettext_noop("Dutch")),
        LanguageInfo::new("nn", "Nynorsk", gettext_noop("Norwegian (Nynorsk)")),
        LanguageInfo::new("pl", "Polski", gettext_noop("Polish")),
        LanguageInfo::new("pt_BR", "Português Brasileiro", gettext_noop("Portuguese (Brazil)")),
        LanguageInfo::new("ru", "Русский", gettext_noop("Russian")),
        LanguageInfo::new("sl", "Slovenščina", gettext_noop("Slovenian")),
        LanguageInfo::new("sv", "Svenska", gettext_noop("Swedish")),
        LanguageInfo::new("tr", "Türkçe", gettext_noop("Turkish")),
        LanguageInfo::new("zh_CN", "中文（简体）", gettext_noop("Chinese (Simplified)")),
        LanguageInfo::new("zh_TW", "中文（繁體）", gettext_noop("Chinese (Traditional)")),
    ]
}
