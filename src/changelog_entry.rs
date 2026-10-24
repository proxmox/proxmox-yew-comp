use regex::Regex;

use pwt::css::{AlignItems, ColorScheme, FlexWrap, Opacity};
use pwt::prelude::*;
use pwt::widget::{Badge, Container, Row};

use proxmox_deb_changelog::{parse_changes, ChangeBlock, ChangelogEntry};

pub(crate) fn empty_changelog_text() -> String {
    tr!(
        "No changelog entries for these versions. The installed changelog only covers the installed version and older ones."
    )
}

/// Urgencies worth pointing out, the usual ones are low and medium.
const URGENT_URGENCIES: [&str; 3] = ["high", "critical", "emergency"];

/// Renders an entry with a header line for its metadata and its changes as lists.
pub(crate) fn render_entry(entry: &ChangelogEntry) -> Html {
    let mut header = Row::new()
        .class(FlexWrap::Wrap)
        .class(AlignItems::Baseline)
        .gap(2)
        .padding_y(1)
        .class("pwt-border-bottom")
        .with_child(html! {
            <span style="font-weight: bold; font-size: 1.1em;">{&entry.version}</span>
        });
    for distribution in entry.distributions.split_whitespace() {
        header.add_child(Badge::new(distribution.to_string()));
    }
    let urgency = entry.urgency.as_deref().map(str::to_lowercase);
    if let Some(urgency) = urgency.filter(|urgency| URGENT_URGENCIES.contains(&urgency.as_str())) {
        header.add_child(
            Badge::new(tr!("urgency {0}", urgency)).color_scheme(ColorScheme::WarningContainer),
        );
    }

    let maintainer = entry.maintainer.as_ref().map(|maintainer| {
        let (name, email) = match maintainer.split_once('<') {
            Some((name, email)) if !name.trim().is_empty() && email.ends_with('>') => {
                (name.trim(), Some(email.trim_end_matches('>').to_string()))
            }
            _ => (maintainer.as_str(), None),
        };
        html! { <span title={email}>{name}</span> }
    });
    let date = entry.date.as_ref().map(|date| {
        html! { <span title={date.clone()}>{format_date(date)}</span> }
    });
    header.add_child(
        Container::from_tag("span")
            .class(Opacity::ThreeQuarters)
            .style("margin-left", "auto")
            .with_child(html! {
                <>
                    {maintainer.clone().unwrap_or_default()}
                    if maintainer.is_some() && date.is_some() { {" · "} }
                    {date.unwrap_or_default()}
                </>
            }),
    );

    let proxmox = entry
        .maintainer
        .as_deref()
        .is_some_and(|maintainer| maintainer.contains("@proxmox.com"));
    Container::new()
        .margin_bottom(3)
        .with_child(header)
        .with_child(render_blocks(
            &parse_changes(&entry.changes),
            proxmox,
            false,
        ))
        .into()
}

/// Formats a changelog date like `Thu, 22 Oct 2026 23:53:59 +0200` in local time, or keeps it if
/// it cannot be parsed.
fn format_date(date: &str) -> String {
    let time = js_sys::Date::parse(date);
    if time.is_nan() {
        return date.to_string();
    }
    let date = js_sys::Date::new(&time.into());
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
    )
}

/// Renders the blocks of the changes of an entry, items as lists.
fn render_blocks(blocks: &[ChangeBlock], proxmox: bool, nested: bool) -> Html {
    let list_style = if nested {
        "padding-left: 1.6em; list-style: circle outside;"
    } else {
        "padding-left: 1.6em; list-style: disc outside;"
    };
    let mut html = Vec::new();
    let mut items = Vec::new();
    for block in blocks {
        if !matches!(block, ChangeBlock::Item { .. }) && !items.is_empty() {
            html.push(html! { <ul class="pwt-my-1" style={list_style}>{for items.drain(..)}</ul> });
        }
        match block {
            ChangeBlock::Item { text, children } => items.push(html! {
                <li class="pwt-my-1" style="line-height: 1.45;">
                    {render_text(text, proxmox)}
                    {render_blocks(children, proxmox, true)}
                </li>
            }),
            ChangeBlock::Paragraph(text) => html.push(html! {
                <div class="pwt-white-space-pre-wrap pwt-my-1" style="line-height: 1.45;">
                    {render_text(text, proxmox)}
                </div>
            }),
            ChangeBlock::Author(name) => html.push(html! {
                <div class="pwt-opacity-75 pwt-mt-2" style="font-style: italic;">
                    {name}
                </div>
            }),
        }
    }
    if !items.is_empty() {
        html.push(html! { <ul class="pwt-my-1" style={list_style}>{for items}</ul> });
    }
    html.into_iter().collect()
}

/// Renders text with links to more information about the bugs and vulnerabilities it mentions.
fn render_text(text: &str, proxmox: bool) -> Html {
    link_parts(text, proxmox)
        .into_iter()
        .map(|part| match part {
            TextPart::Text(text) => Html::from(text.to_string()),
            TextPart::Link { text, url } => html! {
                <a href={url} target="_blank" rel="noopener noreferrer">{text}</a>
            },
        })
        .collect()
}

/// A part of the text of a change, which can link to more information.
#[derive(Clone, Debug, PartialEq, Eq)]
enum TextPart<'a> {
    Text(&'a str),
    Link { text: &'a str, url: String },
}

thread_local! {
    static REFERENCE: Regex = Regex::new(
        r"(?-u:\b)(?:CVE-[0-9]{4}-[0-9]{4,}(?-u:\b)|(?i:closes:)\s*(?i:bug)?#[0-9]+(?:\s*,\s*(?i:bug)?#[0-9]+)*|(?i:fix(?:es)?)\s+#([0-9]+))",
    ).unwrap();
    static BUG: Regex = Regex::new(r"#([0-9]+)").unwrap();
}

/// Splits text into parts that link CVE IDs to the Debian security tracker, Debian bugs closed by
/// an upload to the Debian bug tracker and, for Proxmox packages, fixed bugs to the Proxmox
/// Bugzilla.
fn link_parts(text: &str, proxmox: bool) -> Vec<TextPart<'_>> {
    let mut links = Vec::new();
    REFERENCE.with(|reference| {
        for found in reference.captures_iter(text) {
            let whole = found.get(0).unwrap();
            let (start, end) = (whole.start(), whole.end());
            if let Some(id) = found.get(1) {
                if proxmox {
                    links.push((
                        start,
                        end,
                        format!(
                            "https://bugzilla.proxmox.com/show_bug.cgi?id={}",
                            id.as_str()
                        ),
                    ));
                }
            } else if whole.as_str().starts_with("CVE-") {
                links.push((
                    start,
                    end,
                    format!(
                        "https://security-tracker.debian.org/tracker/{}",
                        whole.as_str()
                    ),
                ));
            } else {
                BUG.with(|bug| {
                    for found in bug.captures_iter(whole.as_str()) {
                        let matched = found.get(0).unwrap();
                        links.push((
                            start + matched.start(),
                            start + matched.end(),
                            format!("https://bugs.debian.org/{}", &found[1]),
                        ));
                    }
                });
            }
        }
    });
    let mut parts = Vec::new();
    let mut last = 0;
    for (start, end, url) in links {
        if start > last {
            parts.push(TextPart::Text(&text[last..start]));
        }
        parts.push(TextPart::Link {
            text: &text[start..end],
            url,
        });
        last = end;
    }
    if last < text.len() {
        parts.push(TextPart::Text(&text[last..]));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links(text: &str, proxmox: bool) -> Vec<(&str, String)> {
        link_parts(text, proxmox)
            .into_iter()
            .filter_map(|part| match part {
                TextPart::Link { text, url } => Some((text, url)),
                TextPart::Text(_) => None,
            })
            .collect()
    }

    #[test]
    fn link_references() {
        assert_eq!(
            links(
                "fix CVE-2026-12345 and CVE-2026-1234, not XCVE-2026-12345",
                false
            ),
            [
                (
                    "CVE-2026-12345",
                    "https://security-tracker.debian.org/tracker/CVE-2026-12345".to_string()
                ),
                (
                    "CVE-2026-1234",
                    "https://security-tracker.debian.org/tracker/CVE-2026-1234".to_string()
                ),
            ]
        );
        assert_eq!(
            links("(Closes: #1, bug#2,#3) closes: #4", false),
            [
                ("#1", "https://bugs.debian.org/1".to_string()),
                ("#2", "https://bugs.debian.org/2".to_string()),
                ("#3", "https://bugs.debian.org/3".to_string()),
                ("#4", "https://bugs.debian.org/4".to_string()),
            ]
        );
        assert_eq!(links("fix #7721: zoom", false), []);
        assert_eq!(
            links("fix #7721: zoom, Fixes  #12", true),
            [
                (
                    "fix #7721",
                    "https://bugzilla.proxmox.com/show_bug.cgi?id=7721".to_string()
                ),
                (
                    "Fixes  #12",
                    "https://bugzilla.proxmox.com/show_bug.cgi?id=12".to_string()
                ),
            ]
        );
        let text = "a CVE-2026-12345 b (Closes: #1) c";
        let joined: String = link_parts(text, false)
            .into_iter()
            .map(|part| match part {
                TextPart::Text(text) | TextPart::Link { text, .. } => text,
            })
            .collect();
        assert_eq!(joined, text);
    }

    #[test]
    fn link_boundaries_and_unicode() {
        let text = "éCVE-2026-1234 CVE-2026-12345a CVE-20261-1234 Closes:bug#5 fixes\t#9";
        assert_eq!(
            links(text, true),
            [
                (
                    "CVE-2026-1234",
                    "https://security-tracker.debian.org/tracker/CVE-2026-1234".to_string()
                ),
                ("#5", "https://bugs.debian.org/5".to_string()),
                (
                    "fixes\t#9",
                    "https://bugzilla.proxmox.com/show_bug.cgi?id=9".to_string()
                ),
            ]
        );
        for proxmox in [false, true] {
            let joined: String = link_parts(text, proxmox)
                .into_iter()
                .map(|part| match part {
                    TextPart::Text(text) | TextPart::Link { text, .. } => text,
                })
                .collect();
            assert_eq!(joined, text);
        }
    }
}
