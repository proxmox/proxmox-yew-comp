use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{bail, Error};
use wasm_bindgen::JsCast;

use pwt::convert_js_error;

/// Tags we explicitly allow.  Anything not on this list (incl. SVG/MathML, custom elements,
/// `<plaintext>`/`<noscript>`/`<template>`/`<base>`/`<meta>`/`<link>`/`<frame*>` and so on) gets
/// replaced with a `<span>` containing the encoded HTML by the walker.
///
/// Compared in lower-case (HTML elements report tag_name in upper-case, foreign elements in
/// lower-case; we normalize to lower-case before matching).  Covers what pulldown-cmark produces
/// plus common raw-HTML patterns admins use in notes.
#[rustfmt::skip]
const ALLOWED_TAGS: &[&str] = &[
    // structural; `html` and `body` are kept since DomParser always wraps the input in them and
    // we walk doc.body itself.
    "html", "body",
    "a", "abbr", "address", "article", "aside", "b", "bdi", "bdo", "blockquote", "br", "caption",
    "cite", "code", "col", "colgroup", "dd", "del", "details", "dfn", "div", "dl", "dt", "em",
    "figcaption", "figure", "footer", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr", "i",
    "img", "input", "ins", "kbd", "li", "main", "mark", "nav", "ol", "p", "pre", "q", "rp", "rt",
    "ruby", "s", "samp", "section", "small", "span", "strong", "sub", "summary", "sup", "table",
    "tbody", "td", "tfoot", "th", "thead", "time", "tr", "u", "ul", "var", "wbr",
];

/// Attributes we keep on allowed elements.  Anything else is dropped.  `id`/`name` are handled
/// specially (namespaced) and so are NOT in this list.
#[rustfmt::skip]
const ALLOWED_ATTRS: &[&str] = &[
    "class", "href", "src", "alt", "align", "valign", "disabled", "checked", "start", "type",
    "target", "colspan", "rowspan", "title", "width", "height", "dir",
];

/// URL schemes we explicitly deny on href/src.  We keep an otherwise-permissive stance for `<a>`
/// (so admins can use shortcuts like rdp:, ssh:, vnc:, mailto:, tel:) but block anything that has
/// historically been used for XSS or local-resource access.
#[rustfmt::skip]
const DENIED_SCHEMES: &[&str] = &[
    "javascript:", "vbscript:", "livescript:", "mocha:", "data:", "jar:",
];

/// Counter to namespace `id`/`name` per [`sanitize_html`] call so multiple notes on the same page
/// can't clobber each other. Avoid pwt's get_unique_element_id() to render ids as `pmx-md-N-...`.
static INSTANCE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn is_http_like(url: &str) -> bool {
    let url = url.trim_start().to_lowercase();
    url.starts_with("http:") || url.starts_with("https:")
}

/// Restrict `<img src="data:...">` to raster image MIMEs only.  Even though browsers don't run
/// scripts in img-loaded SVG today, this also rules out content-sniffing surprises.
fn is_img_data_mime(value: &str) -> bool {
    let lower = value.trim_start().to_lowercase();
    [
        "data:image/png;",
        "data:image/gif;",
        "data:image/jpeg;",
        "data:image/jpg;",
        "data:image/webp;",
        "data:image/x-icon;",
        "data:image/vnd.microsoft.icon;",
        "data:image/bmp;",
    ]
    .iter()
    .any(|p| lower.starts_with(p))
}

/// Validate an `href`/`src` value.  Returns the resolved URL string if safe, or `None` if the
/// attribute should be dropped.
fn validate_url(
    tag_name: &str,
    attr_name: &str,
    value: &str,
    base_url: &str,
    prefix: &str,
) -> Option<String> {
    // same-document fragment-only href: namespace it so it points at our rewritten id/name.
    let trimmed = value.trim_start();
    if let Some(rest) = trimmed.strip_prefix('#') {
        return Some(format!("#{prefix}{rest}"));
    }

    let url = web_sys::Url::new_with_base(value, base_url).ok()?;
    let protocol = url.protocol().to_lowercase();

    if DENIED_SCHEMES.contains(&protocol.as_str()) {
        // <img src="data:image/...,..."> is the one carve-out.
        if tag_name == "img" && attr_name == "src" && is_img_data_mime(value) {
            return Some(url.href());
        }
        return None;
    }

    if tag_name == "img" || tag_name == "input" {
        // resource-loading tags must use http(s); no exotic protocol handlers.
        if is_http_like(&protocol) {
            return Some(url.href());
        }
        return None;
    }
    if tag_name == "a" {
        // <a> keeps the broad behaviour so admins can use shortcuts like rdp:, ssh:, vnc:,
        // mailto:, tel:, etc.; only the explicit denylist above is rejected.
        return Some(url.href());
    }
    // any other tag with href/src (rare; survived the allowlist) -> require http(s).
    if is_http_like(&protocol) {
        Some(url.href())
    } else {
        None
    }
}

/// Replace `node` with a `<span>` element containing its outer HTML as a single text node.
/// We construct the span explicitly with `set_text_content` rather than going through
/// `set_outer_html` so the encoded HTML is never re-parsed.
fn replace_with_encoded(node: &web_sys::Node) -> Result<(), Error> {
    let owner_doc = match node.owner_document() {
        Some(d) => d,
        None => return Ok(()),
    };
    let parent = match node.parent_node() {
        Some(p) => p,
        None => return Ok(()),
    };
    let elem: &web_sys::Element = node.unchecked_ref();
    let span = owner_doc.create_element("span").map_err(convert_js_error)?;
    span.set_text_content(Some(&elem.outer_html()));
    parent
        .replace_child(&span, node)
        .map_err(convert_js_error)?;
    Ok(())
}

fn sanitize_html_element(node: &web_sys::Node, base_url: &str, prefix: &str) -> Result<(), Error> {
    let node_type = node.node_type();

    match node_type {
        3 => Ok(()), // Text node - leave alone
        // Leave comments alone; some users use them in notes to embed metadata that - while not
        // secret - is hidden from viewers in the rendered HTML (but not in the source!).
        8 => Ok(()),
        1 => {
            let elem: &web_sys::Element = node.unchecked_ref();
            let tag_name = elem.tag_name().to_lowercase();

            if !ALLOWED_TAGS.contains(&tag_name.as_str()) {
                return replace_with_encoded(node);
            }

            // snapshot attributes -- we mutate the live NamedNodeMap below.
            let attributes = elem.attributes();
            let mut attrs: Vec<web_sys::Attr> = Vec::with_capacity(attributes.length() as usize);
            for i in 0..attributes.length() {
                if let Some(attr) = attributes.get_with_index(i) {
                    attrs.push(attr);
                }
            }

            for attr in &attrs {
                let name = attr.name().to_lowercase();
                let value = attr.value();

                if name == "id" || name == "name" {
                    // namespace these to prevent DOM clobbering of surrounding framework code.
                    if value.trim().is_empty() {
                        elem.remove_attribute(&attr.name())
                            .map_err(convert_js_error)?;
                    } else {
                        elem.set_attribute(&name, &format!("{prefix}{value}"))
                            .map_err(convert_js_error)?;
                    }
                    continue;
                }
                if !ALLOWED_ATTRS.contains(&name.as_str()) {
                    elem.remove_attribute(&attr.name())
                        .map_err(convert_js_error)?;
                    continue;
                }
                if name == "href" || name == "src" {
                    match validate_url(&tag_name, &name, &value, base_url, prefix) {
                        Some(resolved) => {
                            elem.set_attribute(&attr.name(), &resolved)
                                .map_err(convert_js_error)?;
                        }
                        None => {
                            elem.remove_attribute(&attr.name())
                                .map_err(convert_js_error)?;
                        }
                    }
                    continue;
                }
                if name == "target" {
                    if tag_name != "a" {
                        elem.remove_attribute(&attr.name())
                            .map_err(convert_js_error)?;
                        continue;
                    }
                    // restrict target: only `_blank` is useful in a notes context; `_top`/
                    // `_parent` could break out of the surrounding admin UI.
                    if value.trim().eq_ignore_ascii_case("_blank") {
                        elem.set_attribute(&attr.name(), "_blank")
                            .map_err(convert_js_error)?;
                        // force rel=noopener noreferrer; modern browsers default to noopener for
                        // target=_blank but older ones don't, and we want to suppress Referer.
                        elem.set_attribute("rel", "noopener noreferrer")
                            .map_err(convert_js_error)?;
                    } else {
                        elem.remove_attribute(&attr.name())
                            .map_err(convert_js_error)?;
                    }
                    continue;
                }
            }

            // snapshot children -- recursion may replace nodes.
            let children = node.child_nodes();
            let mut child_vec: Vec<web_sys::Node> = Vec::with_capacity(children.length() as usize);
            for i in 0..children.length() {
                if let Some(c) = children.get(i) {
                    child_vec.push(c);
                }
            }
            for child in child_vec.iter().rev() {
                sanitize_html_element(child, base_url, prefix)?;
            }

            Ok(())
        }
        n => bail!("got unexpected node type {n}"),
    }
}

/// Sanitize HTML.
///
/// Transforms HTML to a DOM tree and recursively descends.  Elements not on the allowlist are
/// replaced with a `<span>` containing their literal HTML as text; on allowed elements,
/// attributes not on the allowlist are dropped.  `href`/`src` values are validated against a URL
/// scheme policy; `id`/`name` (and same-document fragment hrefs) are rewritten with a per-call
/// prefix to prevent DOM clobbering.
pub fn sanitize_html(text: &str) -> Result<String, Error> {
    let location = gloo_utils::window().location();
    let origin = location.origin().unwrap_or_default();
    let wrapped = format!("<div>{}</div>", text);

    let dom_parser = web_sys::DomParser::new().map_err(convert_js_error)?;
    let doc = dom_parser
        .parse_from_string(&wrapped, web_sys::SupportedType::TextHtml)
        .map_err(convert_js_error)?;

    let n = INSTANCE_COUNTER
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1);
    let prefix = format!("pmx-md-{}-", n);

    if let Some(body) = doc.body() {
        sanitize_html_element(&body, &origin, &prefix)?;
        Ok(body.inner_html())
    } else {
        bail!("DomParser produced no body element");
    }
}
