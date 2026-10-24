use std::rc::Rc;

use anyhow::{format_err, Error};
use serde_json::json;

use yew::html::{IntoEventCallback, IntoPropValue};
use yew::virtual_dom::{Key, VComp, VNode};

use pwt::css::AlignItems;
use pwt::prelude::*;
use pwt::state::Loader;
use pwt::widget::form::Combobox;
use pwt::widget::{error_message, Button, Column, Container, Dialog, Row, Toolbar};

use proxmox_deb_changelog::{ChangelogEntry, VersionRange};

use crate::changelog_entry::{empty_changelog_text, render_entry};

use pwt_macros::builder;

async fn get_installed_changelog(
    base_url: AttrValue,
    package: AttrValue,
) -> Result<Vec<ChangelogEntry>, Error> {
    let url = format!("{base_url}/installed-changelog");
    crate::http_get(url, Some(json!({ "name": package.as_str() })))
        .await
        .map_err(|err| match err.downcast_ref::<proxmox_client::Error>() {
            Some(proxmox_client::Error::Api(status, _))
                if *status == http::StatusCode::NOT_FOUND =>
            {
                format_err!(tr!(
                    "No changelog available. The package is not installed or was installed without its documentation."
                ))
            }
            _ => crate::apt_history::api_error_message(err),
        })
}

/// Shows the Debian changelog that an installed package ships, limited to the versions later than
/// `since_version` up to including `to_version`, like for an upgrade from the former to the latter.
#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct PackageChangelog {
    #[prop_or_default]
    pub key: Option<Key>,

    #[prop_or("/nodes/localhost/apt".into())]
    #[builder(IntoPropValue, into_prop_value)]
    /// The base url of the APT API of the node
    pub base_url: AttrValue,

    /// The name of the installed package.
    pub package: AttrValue,

    /// Only show the entries of versions later than this one.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub since_version: Option<AttrValue>,

    /// Only show the entries of versions up to including this one.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub to_version: Option<AttrValue>,

    /// Close callback
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,
}

impl PackageChangelog {
    pub fn new(package: impl IntoPropValue<AttrValue>) -> Self {
        yew::props!(Self {
            package: package.into_prop_value(),
        })
    }

    pwt::impl_yew_std_props_builder!();
}

pub enum Msg {
    Loaded,
    Since(String),
    To(String),
    ShowAll,
}

pub struct ProxmoxPackageChangelog {
    /// All entries, loaded once and filtered here, which is cheap compared to loading them again.
    loader: Loader<Vec<ChangelogEntry>>,
    versions: Rc<Vec<AttrValue>>,
    since: String,
    to: String,
    /// Whether to show all entries, not only the first part of a big changelog.
    show_all: bool,
}

impl ProxmoxPackageChangelog {
    fn version_selector(
        &self,
        label: String,
        value: &str,
        placeholder: String,
        on_change: Callback<String>,
    ) -> Html {
        Row::new()
            .gap(1)
            .class(AlignItems::Center)
            .with_child(label.clone())
            .with_child(
                Combobox::new()
                    .aria_label(label)
                    .editable(true)
                    .items(self.versions.clone())
                    .value(AttrValue::from(value.to_owned()))
                    .placeholder(placeholder)
                    .on_change(on_change),
            )
            .into()
    }

    fn entries_view(&self, ctx: &Context<Self>, entries: &[ChangelogEntry]) -> Html {
        let range = match VersionRange::new(version_limit(&self.since), version_limit(&self.to)) {
            Ok(range) => range,
            Err(err) => return error_message(&err.to_string()).into(),
        };
        let shown: Vec<&ChangelogEntry> = range.select(entries).collect();
        if shown.is_empty() {
            return empty_changelog_text().into();
        }

        let count = if self.show_all {
            shown.len()
        } else {
            first_part(&shown)
        };
        let mut html: Vec<Html> = shown[..count]
            .iter()
            .map(|entry| render_entry(entry))
            .collect();
        if !range.is_listed(entries) {
            let to = self.to.trim();
            let note = tr!(
                "The changelog does not list version {0}, so only its newest entry is shown.",
                to
            );
            html.insert(
                0,
                Container::new().padding_bottom(2).with_child(note).into(),
            );
        }
        if count < shown.len() {
            let n = shown.len();
            html.push(
                Button::new(tr!("Show all {n} entry" | "Show all {n} entries" % n))
                    .onclick(ctx.link().callback(|_| Msg::ShowAll))
                    .into(),
            );
        }
        html.into_iter().collect()
    }
}

/// The size of the changes shown at first, as rendering a few MiB of changes takes a while.
const FIRST_PART_SIZE: usize = 256 * 1024;

/// The number of entries in the first part, at least one.
fn first_part(entries: &[&ChangelogEntry]) -> usize {
    let mut count = 0;
    let mut size = 0;
    while count < entries.len() && (count == 0 || size < FIRST_PART_SIZE) {
        size += entries[count].changes.len();
        count += 1;
    }
    count
}

/// An empty version selection does not limit the versions.
fn version_limit(version: &str) -> Option<&str> {
    Some(version.trim()).filter(|version| !version.is_empty())
}

impl Component for ProxmoxPackageChangelog {
    type Message = Msg;
    type Properties = PackageChangelog;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();
        let base_url = props.base_url.clone();
        let package = props.package.clone();
        let loader = Loader::new()
            .loader(move || get_installed_changelog(base_url.clone(), package.clone()))
            .on_change(ctx.link().callback(|_| Msg::Loaded));
        loader.load();
        Self {
            loader,
            versions: Rc::new(Vec::new()),
            since: props
                .since_version
                .as_deref()
                .unwrap_or_default()
                .to_string(),
            to: props.to_version.as_deref().unwrap_or_default().to_string(),
            show_all: false,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded => {
                if let Some(Ok(entries)) = &self.loader.read().data {
                    self.versions = Rc::new(
                        entries
                            .iter()
                            .map(|entry| AttrValue::from(entry.version.clone()))
                            .collect(),
                    );
                }
            }
            Msg::Since(since) => self.since = since,
            Msg::To(to) => self.to = to,
            Msg::ShowAll => self.show_all = true,
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let toolbar = Toolbar::new()
            .class("pwt-border-bottom")
            .with_child(self.version_selector(
                tr!("From"),
                &self.since,
                tr!("Oldest"),
                ctx.link().callback(Msg::Since),
            ))
            .with_child(self.version_selector(
                tr!("To"),
                &self.to,
                tr!("Newest"),
                ctx.link().callback(Msg::To),
            ));

        let entries = Container::new()
            .padding(2)
            .class("pwt-flex-fit")
            .with_child(match &self.loader.read().data {
                // without the error prefix of the loader, as a missing changelog is no fault
                Some(Err(err)) => error_message(&err.to_string()).into(),
                _ => self
                    .loader
                    .render(|entries| self.entries_view(ctx, &entries)),
            });

        Dialog::new(tr!("Changelog") + ": " + &props.package)
            .resizable(true)
            .width(800)
            .height(600)
            .on_close(props.on_close.clone())
            .with_child(
                Column::new()
                    .class("pwt-flex-fit")
                    .with_child(toolbar)
                    .with_child(entries),
            )
            .into()
    }
}

impl From<PackageChangelog> for VNode {
    fn from(props: PackageChangelog) -> Self {
        let key = props.key.clone();
        let comp = VComp::new::<ProxmoxPackageChangelog>(Rc::new(props), key);
        VNode::from(comp)
    }
}
