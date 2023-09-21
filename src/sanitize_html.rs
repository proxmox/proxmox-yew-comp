use anyhow::{bail, Error};
use wasm_bindgen::JsCast;

use pwt::convert_js_error;

fn is_http_like(url: &str) -> bool {
    let url = url.to_lowercase();
    url.starts_with("http:") || url.starts_with("https:")
}

fn sanitize_url(text: &str, base_url: &str) -> Result<String, Error> {
    let text = text.trim();
    if is_http_like(text) {
        return Ok(text.to_string());
    }

    let url = web_sys::Url::new_with_base(text, base_url).map_err(convert_js_error)?;
    let protocol = url.protocol();
    if is_http_like(&protocol) {
        return Ok(url.href());
    } else {
        bail!("got unexpected url protocol: {protocol}");
    }
}

fn sanitize_html_element(node: &web_sys::Node, base_url: &str) -> Result<(), Error> {
    let node_type = node.node_type();

    match node_type {
        3 => Ok(()), /* Text Node */
        1 => {
            /*  Element Node */
            let node: &web_sys::Element = node.unchecked_ref();

            let tag_name = node.tag_name();

            match tag_name.as_str() {
                "SCRIPT" | "STYLE" | "FORM" | "SELECT" | "OPTION" | "OPTGROUP" | "MAP" | "AREA"
                | "CANVAS" | "TEXTAREA" | "APPLET" | "FONT" | "IFRAME" | "AUDIO" | "VIDEO "
                | "OBJECT " | "EMBED" | "SVG" => {
                    // could do node.remove() instead, but it's nicer UX if we keep the (encoded!) html
                    node.set_outer_html(&format!(
                        "<span>{}</span>",
                        html_escape::encode_text(&node.outer_html())
                    ));
                }
                _ => {}
            }

            let attributes = node.attributes();
            for i in (0..attributes.length()).rev() {
                if let Some(attr) = attributes.get_with_index(i) {
                    // TODO: we may want to also disallow class and id attrs
                    let name = attr.name().to_lowercase();
                    match name.as_str() {
                        "href" | "src" => {
                            let value = attr.value();
                            if let Ok(url) = sanitize_url(&value, base_url) {
                                attr.set_value(&url);
                            } else {
                                attributes
                                    .remove_named_item(&name)
                                    .map_err(convert_js_error)?;
                            }
                        }
                        "class" | "id" | "name" | "alt" | "align" | "valign" | "disabled"
                        | "checked" | "start" | "type" => { /* allow */ }
                        _ => {
                            attributes
                                .remove_named_item(&name)
                                .map_err(convert_js_error)?;
                        }
                    }
                }
            }

            let children = node.child_nodes();
            for i in (0..children.length()).rev() {
                if let Some(node) = children.get(i) {
                    sanitize_html_element(&node, base_url)?;
                }
            }

            return Ok(());
        }
        n => {
            bail!("got unexpected node type {n}");
        }
    }
}

/// Sanitize Html
///
/// Transforms HTML to a DOM tree and recursively descends and HTML-encodes every branch with a
/// "bad" node.type and drops "bad" attributes from the remaining nodes.
/// "bad" means anything which can do XSS or break the layout of the outer page.
pub fn sanitize_html(text: &str) -> Result<String, Error> {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let origin = location.origin().unwrap_or(String::new());

    let dom_parser = web_sys::DomParser::new().map_err(convert_js_error)?;
    let doc = dom_parser
        .parse_from_string(text, web_sys::SupportedType::TextHtml)
        .map_err(convert_js_error)?;

    if let Some(body) = doc.body() {
        sanitize_html_element(&body, &origin)?;
        let html = body.inner_html();
        Ok(html)
    } else {
        Ok(String::new())
    }
}
