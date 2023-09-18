use std::rc::Rc;

use pwt::props::ExtractPrimaryKey;
use pwt::state::{Selection, Store};
use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::{Dropdown, GridPicker};

use pwt_macros::builder;

#[derive(Clone, PartialEq)]
pub struct LanguageInfo {
    lang: String,            // id (de, en, ...)
    text: String,            // Language name (native).
    translated_text: String, // Translated language name.
}

impl LanguageInfo {
    fn new(
        lang: impl Into<String>,
        text: impl Into<String>,
        tr: impl Into<String>,
    ) -> LanguageInfo {
        LanguageInfo {
            lang: lang.into(),
            text: text.into(),
            translated_text: tr.into(),
        }
    }
}

impl ExtractPrimaryKey for LanguageInfo {
    fn extract_key(&self) -> yew::virtual_dom::Key {
        Key::from(self.lang.clone())
    }
}

#[derive(Clone, PartialEq, Properties)]
#[builder]
pub struct LanguageSelector {
    #[builder(IntoPropValue, into_prop_value)]
    pub default: Option<AttrValue>,

    /// On change callback.
    #[builder_cb(IntoEventCallback, into_event_callback, String)]
    on_change: Option<Callback<String>>,
}

impl LanguageSelector {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

#[doc(hidden)]
pub struct ProxmoxLanguageSelector {
    store: Store<LanguageInfo>,
    selection: Selection,
    lang: String,
}

fn language_list() -> Vec<LanguageInfo> {
    vec![
        LanguageInfo::new("ar", "العربية", tr!("Arabic")),
        LanguageInfo::new("ca", "Català", tr!("Catalan")),
        LanguageInfo::new("da", "Dansk", tr!("Danish")),
        LanguageInfo::new("de", "Deutsch", tr!("German")),
        LanguageInfo::new("en", "English", tr!("English")),
        LanguageInfo::new("es", "Español", tr!("Spanish")),
        LanguageInfo::new("eu", "Euskera (Basque)", tr!("Euskera (Basque)")),
        LanguageInfo::new("fa", "فارسی", tr!("Persian (Farsi)")),
        LanguageInfo::new("fr", "Français", tr!("French")),
        LanguageInfo::new("he", "עברית", tr!("Hebrew")),
        LanguageInfo::new("it", "Italiano", tr!("Italian")),
        LanguageInfo::new("ja", "日本語", tr!("Japanese")),
        LanguageInfo::new("kr", "한국어", tr!("Korean")),
        LanguageInfo::new("nb", "Bokmål", tr!("Norwegian (Bokmal)")),
        LanguageInfo::new("nl", "Nederlands", tr!("Dutch")),
        LanguageInfo::new("nn", "Nynorsk", tr!("Norwegian (Nynorsk)")),
        LanguageInfo::new("pl", "Polski", tr!("Polish")),
        LanguageInfo::new("pt_BR", "Português Brasileiro", tr!("Portuguese (Brazil)")),
        LanguageInfo::new("ru", "Русский", tr!("Russian")),
        LanguageInfo::new("sl", "Slovenščina", tr!("Slovenian")),
        LanguageInfo::new("sv", "Svenska", tr!("Swedish")),
        LanguageInfo::new("tr", "Türkçe", tr!("Turkish")),
        LanguageInfo::new("zh_CN", "中文（简体）", tr!("Chinese (Simplified)")),
        LanguageInfo::new("zh_TW", "中文（繁體）", tr!("Chinese (Traditional)")),
    ]
}

pub enum Msg {
    Select(String),
}

impl Component for ProxmoxLanguageSelector {
    type Message = Msg;
    type Properties = LanguageSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        let store = Store::new();
        store.set_data(language_list());
        let selection = Selection::new();

        let lang = props.default.as_deref().unwrap_or("").to_string();

        selection.select(Key::from(lang.clone()));

        Self { store, selection, lang }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::Select(lang) => {
                self.lang = lang.clone();
                if let Some(on_change) = &props.on_change {
                    on_change.emit(lang);
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let picker = {
            let store = self.store.clone();
            let columns = COLUMNS.with(Rc::clone);
            let selection = self.selection.clone();

            move |on_select: &Callback<Key>| {
                let table = DataTable::new(columns.clone(), store.clone());

                GridPicker::new(table)
                    .selection(selection.clone())
                    .show_filter(false)
                    .on_select(on_select.clone())
                    .into()
            }
        };

        let store = self.store.clone();

        Dropdown::new(picker)
            .value(self.lang.clone())
            .on_change(ctx.link().callback(Msg::Select))
            .render_value(move |id: &AttrValue| {
                let key = Key::from(id.to_string());
                if let Some(info) = store.read().lookup_record(&key) {
                    html! {&info.text}
                } else {
                    html! {id}
                }
            })
            .into()
    }
}

impl Into<VNode> for LanguageSelector {
    fn into(self) -> VNode {
        let comp = VComp::new::<ProxmoxLanguageSelector>(Rc::new(self), None);
        VNode::from(comp)
    }
}

thread_local! {
    static COLUMNS: Rc<Vec<DataTableHeader<LanguageInfo>>> = Rc::new(vec![
        DataTableColumn::new(tr!("Language"))
            .width("200px")
            .show_menu(false)
            .render(|info: &LanguageInfo| {
                html!{&info.text}
            })
            .sorter(|a: &LanguageInfo, b: &LanguageInfo| {
                a.text.cmp(&b.text)
            })
            .sort_order(true)
            .into(),
        DataTableColumn::new(tr!("Translated"))
            .width("200px")
            .show_menu(false)
            .render(|info: &LanguageInfo| {
                html!{&info.translated_text}
            })
            .sorter(|a: &LanguageInfo, b: &LanguageInfo| {
                a.translated_text.cmp(&b.translated_text)
            })
           .into(),
    ]);
}
