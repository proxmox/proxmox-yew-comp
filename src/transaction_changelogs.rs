use std::collections::HashSet;
use std::rc::Rc;

use yew::virtual_dom::{VComp, VNode};

use pwt::css::{AlignItems, Opacity, WhiteSpace};
use pwt::prelude::*;
use pwt::widget::{Button, Column, Container, Fa, Toolbar};

use proxmox_apt_history::{PackageChange, TransactionChangelog};

use crate::changelog_entry::{empty_changelog_text, render_entry};

/// Shows the changelog entries for the package changes of a transaction, in the given groups by
/// source package. The entries of a group only get rendered once it is opened.
#[derive(Properties, PartialEq, Clone)]
pub(crate) struct TransactionChangelogs {
    pub groups: Rc<Vec<TransactionChangelog>>,
    #[prop_or_default]
    pub empty_text: Option<AttrValue>,
}

impl TransactionChangelogs {
    pub fn new(groups: Rc<Vec<TransactionChangelog>>) -> Self {
        yew::props!(Self { groups })
    }

    pub fn empty_text(mut self, text: impl Into<AttrValue>) -> Self {
        self.empty_text = Some(text.into());
        self
    }
}

pub(crate) enum Msg {
    SetAllOpen(bool),
    SetOpen(usize, bool),
}

pub(crate) struct ProxmoxTransactionChangelogs {
    open: Vec<bool>,
    on_toggle: Vec<Callback<bool>>,
}

impl ProxmoxTransactionChangelogs {
    fn reset(&mut self, ctx: &Context<Self>) {
        let count = ctx.props().groups.len();
        self.open = vec![false; count];
        self.on_toggle = (0..count)
            .map(|index| ctx.link().callback(move |open| Msg::SetOpen(index, open)))
            .collect();
    }
}

impl Component for ProxmoxTransactionChangelogs {
    type Message = Msg;
    type Properties = TransactionChangelogs;

    fn create(ctx: &Context<Self>) -> Self {
        let mut this = Self {
            open: Vec::new(),
            on_toggle: Vec::new(),
        };
        this.reset(ctx);
        this
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SetAllOpen(open) => {
                let changed = self.open.iter().any(|value| *value != open);
                self.open.fill(open);
                changed
            }
            Msg::SetOpen(index, open) => std::mem::replace(&mut self.open[index], open) != open,
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old: &Self::Properties) -> bool {
        if !Rc::ptr_eq(&ctx.props().groups, &old.groups) {
            self.reset(ctx);
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let groups = &ctx.props().groups;
        let toolbar = Toolbar::new()
            .class("pwt-border-bottom")
            .with_child(
                Button::new(tr!("Expand All"))
                    .icon_class("fa fa-plus-square-o")
                    .onclick(ctx.link().callback(|_| Msg::SetAllOpen(true))),
            )
            .with_child(
                Button::new(tr!("Collapse All"))
                    .icon_class("fa fa-minus-square-o")
                    .onclick(ctx.link().callback(|_| Msg::SetAllOpen(false))),
            );
        let list: Html = if groups.is_empty() {
            tr!("No package changes with a changelog.").into()
        } else {
            (0..groups.len())
                .map(|index| {
                    html! {
                        <ProxmoxChangelogGroup
                            key={index}
                            groups={groups.clone()}
                            empty_text={ctx.props().empty_text.clone()}
                            {index}
                            open={self.open[index]}
                            on_toggle={self.on_toggle[index].clone()}
                        />
                    }
                })
                .collect()
        };
        Column::new()
            .class("pwt-flex-fit")
            .with_child(toolbar)
            .with_child(
                Container::new()
                    .padding_x(2)
                    .class("pwt-flex-fit")
                    .with_child(list),
            )
            .into()
    }
}

impl From<TransactionChangelogs> for VNode {
    fn from(props: TransactionChangelogs) -> Self {
        let comp = VComp::new::<ProxmoxTransactionChangelogs>(Rc::new(props), None);
        VNode::from(comp)
    }
}

#[derive(Properties, Clone)]
struct ChangelogGroup {
    groups: Rc<Vec<TransactionChangelog>>,
    empty_text: Option<AttrValue>,
    index: usize,
    open: bool,
    on_toggle: Callback<bool>,
}

/// Compares the groups by pointer, comparing their content for each group on every change would be
/// expensive for big transactions.
impl PartialEq for ChangelogGroup {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.groups, &other.groups)
            && self.index == other.index
            && self.open == other.open
            && self.on_toggle == other.on_toggle
            && self.empty_text == other.empty_text
    }
}

#[function_component]
fn ProxmoxChangelogGroup(props: &ChangelogGroup) -> Html {
    let group = &props.groups[props.index];
    let name = group
        .source
        .as_deref()
        .unwrap_or(&group.packages[0].package);
    let transitions = transitions(&group.packages);
    let others = other_packages(&group.packages, name);
    // the errors shrink with an ellipsis, so that the versions stay visible
    let meta = Container::from_tag("span")
        .class(Opacity::ThreeQuarters)
        .style("margin-left", "auto");
    let meta = if let Some(error) = &group.error {
        meta.class("pwt-text-truncate")
            .attribute("title", error.clone())
            .with_child(error)
    } else if group.source.is_none() {
        meta.class(WhiteSpace::Nowrap)
            .with_child(tr!("No changelog installed"))
    } else {
        let n = group.entries.len();
        meta.class(WhiteSpace::Nowrap)
            .with_child(tr!("{n} entry" | "{n} entries" % n))
    };
    let ontoggle = props.on_toggle.reform(|event: Event| {
        event
            .target_dyn_into::<web_sys::Element>()
            .is_some_and(|el| el.has_attribute("open"))
    });
    let caret = Fa::new("caret-right")
        .fixed_width()
        .class(Opacity::ThreeQuarters)
        .style("flex-shrink", "0")
        .style("transform", props.open.then_some("rotate(90deg)"));
    let summary = Container::from_tag("summary")
        .class("pwt-d-flex pwt-gap-2")
        .padding_y(2)
        .class(AlignItems::Baseline)
        .style("cursor", "pointer")
        .style("list-style", "none")
        .with_child(caret)
        // explicit, as the CSS reset of pwt makes strong as heavy as normal text
        .with_child(html! {
            <strong class={classes!(WhiteSpace::Nowrap)} style="font-weight: bold">{name}</strong>
        })
        .with_child(html! {
            <span class={classes!("pwt-font-monospace", WhiteSpace::Nowrap)}>{transitions}</span>
        })
        .with_child(html! {
            <span class="pwt-text-truncate pwt-opacity-75" title={others.clone()}>{others}</span>
        })
        .with_child(meta);
    let entries = props.open.then(|| {
        Container::new()
            .padding_top(1)
            .padding_bottom(2)
            .style("padding-left", "1.6em")
            .with_child(entries_view(group, props.empty_text.as_deref()))
            .into_html()
    });
    html! {
        <details class="pwt-border-bottom" open={props.open} {ontoggle}>
            {summary}
            {entries}
        </details>
    }
}

fn transitions(packages: &[PackageChange]) -> String {
    let mut seen = HashSet::new();
    packages
        .iter()
        .filter_map(|change| {
            let new = change.new_version.as_deref()?;
            let transition = match change.old_version.as_deref() {
                Some(old) if old != new => format!("{old} → {new}"),
                _ => new.to_owned(),
            };
            seen.insert(transition.clone()).then_some(transition)
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

fn other_packages(packages: &[PackageChange], name: &str) -> String {
    let mut seen = HashSet::new();
    packages
        .iter()
        .map(|change| change.package.as_str())
        .filter(|package| *package != name && seen.insert(*package))
        .collect::<Vec<_>>()
        .join(", ")
}

fn entries_view(group: &TransactionChangelog, empty_text: Option<&str>) -> Html {
    // the summary line only has room for the start of an error
    if let Some(error) = &group.error {
        return html! { {error} };
    }
    if group.source.is_none() {
        return html! {};
    }
    if group.entries.is_empty() {
        return empty_text
            .map(str::to_owned)
            .unwrap_or_else(empty_changelog_text)
            .into();
    }
    let note = group.unlisted_version.then(|| {
        Container::new()
            .padding_bottom(2)
            .with_child(tr!(
                "The changelog does not list the package version, so only its newest entry is shown."
            ))
            .into()
    });
    note.into_iter()
        .chain(group.entries.iter().map(render_entry))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn change(package: &str, old: Option<&str>, new: &str) -> PackageChange {
        serde_json::from_value(json!({
            "package": package, "action": "downgrade", "old-version": old, "new-version": new,
        }))
        .unwrap()
    }

    #[test]
    fn headers_show_unique_actual_transitions() {
        let packages = vec![
            change("libfoo", Some("3"), "2"),
            change("libfoo", Some("3"), "2"),
            change("foo-bin", Some("2"), "2"),
            change("foo-doc", None, "1"),
        ];
        assert_eq!(transitions(&packages), "3 → 2 / 2 / 1");
        assert_eq!(other_packages(&packages, "foo"), "libfoo, foo-bin, foo-doc");
        assert_eq!(other_packages(&packages, "libfoo"), "foo-bin, foo-doc");
    }

    #[test]
    fn unchanged_group_props_compare_equal() {
        let props = ChangelogGroup {
            groups: Rc::new(Vec::new()),
            empty_text: None,
            index: 0,
            open: false,
            on_toggle: Callback::from(|_| ()),
        };
        let unchanged = props.clone();
        let mut toggled = props.clone();
        toggled.open = true;
        assert!(props == unchanged);
        assert!(props != toggled);
        toggled.open = false;
        assert!(props == toggled);
    }
}
