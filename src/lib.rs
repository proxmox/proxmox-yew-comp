pub mod acme;

mod api_load_callback;
pub use api_load_callback::{ApiLoadCallback, IntoApiLoadCallback};

#[cfg(feature = "apt")]
mod apt_package_manager;
#[cfg(feature = "apt")]
pub use apt_package_manager::{AptPackageManager, ProxmoxAptPackageManager};

#[cfg(feature = "apt")]
mod apt_repositories;
#[cfg(feature = "apt")]
pub use apt_repositories::{AptRepositories, ProxmoxAptRepositories};

mod auth_view;
pub use auth_view::{AuthView, ProxmoxAuthView};

mod auth_edit_openid;
pub use auth_edit_openid::{AuthEditOpenID, ProxmoxAuthEditOpenID};

mod auth_edit_ldap;
pub use auth_edit_ldap::{AuthEditLDAP, ProxmoxAuthEditLDAP};

mod authid_selector;
pub use authid_selector::AuthidSelector;

mod acl;
pub use acl::{AclEdit, AclView};

mod bandwidth_selector;
pub use bandwidth_selector::{BandwidthSelector, ProxmoxBandwidthSelector};

mod bond_mode_selector;
pub use bond_mode_selector::{BondModeSelector, ProxmoxBondModeSelector};

mod bond_xmit_hash_policy_selector;
pub use bond_xmit_hash_policy_selector::{
    BondXmitHashPolicySelector, ProxmoxBondXmitHashPolicySelector,
};

pub mod common_api_types;

mod confirm_button;
pub use confirm_button::default_confirm_remove_message;
pub use confirm_button::{ConfirmButton, ProxmoxConfirmButton};

mod data_view_window;
pub use data_view_window::{DataViewWindow, ProxmoxDataViewWindow};

pub mod form;

pub mod gauge;
pub use gauge::{Gauge, ProxmoxGauge};

mod http_client_wasm;
pub use http_client_wasm::*;

pub mod http_stream;

mod http_helpers;
pub use http_helpers::*;

mod help_button;
pub use help_button::{HelpButton, PbsHelpButton};

mod calendar_event_selector;
pub use calendar_event_selector::CalendarEventSelector;

pub mod configuration;

mod edit_window;
pub use edit_window::{EditWindow, PwtEditWindow};

mod edit_dialog;
pub use edit_dialog::EditDialog;

mod editable_property;
pub use editable_property::{EditableProperty, PropertyEditorState, RenderPropertyInputPanelFn};

mod key_value_grid;
pub use key_value_grid::{KVGrid, KVGridRow, PwtKVGrid, RenderKVGridRecordFn};

pub mod layout;

mod loadable_component;
pub use loadable_component::{
    LoadableComponent, LoadableComponentContext, LoadableComponentLink, LoadableComponentMaster,
};

mod node_info;
pub use node_info::{node_info, NodeStatus};

mod notes_view;
pub use notes_view::{NotesView, NotesWithDigest, ProxmoxNotesView};

mod object_grid;
pub use object_grid::{
    ObjectGrid, ObjectGridController, ObjectGridRow, PwtObjectGrid, RenderObjectGridItemFn,
};

mod permission_panel;
pub use permission_panel::{PermissionPanel, ProxmoxPermissionPanel};

mod property_list;
pub use property_list::PropertyList;

mod pending_property_list;
pub use pending_property_list::PendingPropertyList;

mod pending_property_grid;
pub use pending_property_grid::PendingPropertyGrid;

pub mod pve_api_types;

mod realm_selector;
pub use realm_selector::RealmSelector;

mod role_selector;
pub use role_selector::RoleSelector;

#[cfg(feature = "rrd")]
mod rrd;
#[cfg(feature = "rrd")]
pub use rrd::{RRDGraph, Series};

#[cfg(feature = "rrd")]
mod rrd_grid;
#[cfg(feature = "rrd")]
pub use rrd_grid::RRDGrid;

#[cfg(feature = "rrd")]
mod rrd_timeframe_selector;
#[cfg(feature = "rrd")]
pub use rrd_timeframe_selector::{RRDTimeframe, RRDTimeframeSelector};

mod running_tasks;
pub use running_tasks::{ProxmoxRunningTasks, RunningTasks};

mod running_tasks_button;
pub use running_tasks_button::{ProxmoxRunningTasksButton, RunningTasksButton};

mod safe_confirm_dialog;
pub use safe_confirm_dialog::{ProxmoxSafeConfirmDialog, SafeConfirmDialog};

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

mod status;
pub use status::{GuestState, NodeState, Status, StorageState};

mod status_row;
pub use status_row::{ProxmoxStatusRow, StatusRow};

mod submit_value_callback;
pub use submit_value_callback::{IntoSubmitValueCallback, SubmitValueCallback};

mod subscription_alert;
pub use subscription_alert::{ProxmoxSubscriptionAlert, SubscriptionAlert};

mod subscription_panel;
pub use subscription_panel::{ProxmoxSubscriptionPanel, SubscriptionPanel};

mod subscription_info;
pub use subscription_info::{subscription_note, ProxmoxSubscriptionInfo, SubscriptionInfo};

mod syslog;
pub use syslog::{ProxmoxSyslog, Syslog};

pub mod tfa;

mod time_zone_selector;
pub use time_zone_selector::{ProxmoxTimezoneSelector, TimezoneSelector};

mod theme_dialog;
pub use theme_dialog::{ProxmoxThemeDialog, ThemeDialog};

mod task_viewer;
pub use task_viewer::*;

mod task_progress;
pub use task_progress::TaskProgress;

mod task_status_selector;
pub use task_status_selector::{ProxmoxTaskStatusSelector, TaskStatusSelector};

mod task_type_selector;
pub use task_type_selector::{ProxmoxTaskTypeSelector, TaskTypeSelector};

mod tasks;
pub use tasks::{ProxmoxTasks, Tasks};

pub mod percent_encoding;

mod proxmox_product;
pub use proxmox_product::{ExistingProduct, ProjectInfo};

mod wizard;
pub use wizard::{PwtWizard, Wizard, WizardPageRenderInfo};

mod user_panel;
pub use user_panel::UserPanel;

mod token_panel;
pub use token_panel::TokenPanel;

pub mod utils;

mod xtermjs;
pub use xtermjs::{ConsoleType, ProxmoxXTermJs, XTermJs};

use pwt::gettext_noop;
use pwt::state::{LanguageInfo, TextDirection};

// Bindgen javascript code from js-helper-module.js

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{self, prelude::*};
#[wasm_bindgen(module = "/js-helper-module.js")]
#[cfg(target_arch = "wasm32")]
extern "C" {
    pub fn async_sleep(ms: i32) -> js_sys::Promise;

    pub fn get_cookie() -> String;
    pub fn set_cookie(value: &str);
    pub fn clear_auth_cookie(name: &str);
}

// Create wrapper which panics if called from target_arch!=wasm32
// This allows us to build with "cargo build" and run tests with "cargo test".
#[cfg(not(target_arch = "wasm32"))]
pub use panic_wrapper::*;
#[cfg(not(target_arch = "wasm32"))]
mod panic_wrapper {
    pub fn async_sleep(_ms: i32) -> js_sys::Promise {
        unreachable!()
    }
    pub fn get_cookie() -> String {
        unreachable!()
    }
    pub fn set_cookie(_value: &str) {
        unreachable!()
    }
    pub fn clear_auth_cookie(_name: &str) {
        unreachable!()
    }
}

pub fn store_csrf_token(crsf_token: &str) {
    if let Some(store) = pwt::state::session_storage() {
        if store.set_item("CSRFToken", crsf_token).is_err() {
            log::error!("store_csrf_token: store.set_item() failed");
        }
    }
}

pub fn load_csrf_token() -> Option<String> {
    pwt::state::session_storage().and_then(|store| store.get_item("CSRFToken").unwrap_or(None))
}

/// Returns the list of available languages for Proxmox Products.
///
/// # Note
///
/// All Proxmox products should support those languages.
pub fn available_language_list() -> Vec<LanguageInfo> {
    vec![
        LanguageInfo::new("ar", "العربية", gettext_noop("Arabic")).direction(TextDirection::Rtl),
        LanguageInfo::new("bg", "Български", gettext_noop("Bulgarian")),
        LanguageInfo::new("ca", "Català", gettext_noop("Catalan")),
        LanguageInfo::new("cs", "Czech", gettext_noop("Czech")),
        LanguageInfo::new("da", "Dansk", gettext_noop("Danish")),
        LanguageInfo::new("de", "Deutsch", gettext_noop("German")),
        LanguageInfo::new("en", "English", gettext_noop("English")),
        LanguageInfo::new("es", "Español", gettext_noop("Spanish")),
        LanguageInfo::new("eu", "Euskera (Basque)", gettext_noop("Euskera (Basque)")),
        LanguageInfo::new("fa", "فارسی", gettext_noop("Persian (Farsi)"))
            .direction(TextDirection::Rtl),
        LanguageInfo::new("fr", "Français", gettext_noop("French")),
        LanguageInfo::new("he", "עברית", gettext_noop("Hebrew")).direction(TextDirection::Rtl),
        LanguageInfo::new("hr", "Hrvatski", gettext_noop("Croatian")),
        LanguageInfo::new("it", "Italiano", gettext_noop("Italian")),
        LanguageInfo::new("ja", "日本語", gettext_noop("Japanese")),
        LanguageInfo::new("ka", "ქართული", gettext_noop("Georgian")),
        LanguageInfo::new("ko", "한국어", gettext_noop("Korean")),
        LanguageInfo::new("nb", "Bokmål", gettext_noop("Norwegian (Bokmal)")),
        LanguageInfo::new("nl", "Nederlands", gettext_noop("Dutch")),
        LanguageInfo::new("nn", "Nynorsk", gettext_noop("Norwegian (Nynorsk)")),
        LanguageInfo::new("pl", "Polski", gettext_noop("Polish")),
        LanguageInfo::new(
            "pt_BR",
            "Português Brasileiro",
            gettext_noop("Portuguese (Brazil)"),
        ),
        LanguageInfo::new("ru", "Русский", gettext_noop("Russian")),
        LanguageInfo::new("sl", "Slovenščina", gettext_noop("Slovenian")),
        LanguageInfo::new("sv", "Svenska", gettext_noop("Swedish")),
        LanguageInfo::new("tr", "Türkçe", gettext_noop("Turkish")),
        LanguageInfo::new("ukr", "Українська", gettext_noop("Ukrainian")),
        LanguageInfo::new(
            "zh_CN",
            "中文（简体）",
            gettext_noop("Chinese (Simplified)"),
        ),
        LanguageInfo::new(
            "zh_TW",
            "中文（繁體）",
            gettext_noop("Chinese (Traditional)"),
        ),
    ]
}
