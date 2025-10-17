use wasm_bindgen::JsCast;
use yew::NodeRef;

use pwt::convert_js_error;

#[deprecated(
    note = "This relies on the deprecated `execCommand` method. Please use `utils::copy_text_to_clipboard` instead."
)]
/// Copies the content of the passed `NodeRef` to the user's clipboard. The `NodeRef` should be
/// caste-able to `web_sys::HtmlInputElement`.
pub fn copy_to_clipboard(node_ref: &NodeRef) {
    if let Some(el) = node_ref.cast::<web_sys::HtmlInputElement>() {
        let window = gloo_utils::window();
        let document = gloo_utils::document();

        let selection = window.get_selection().unwrap().unwrap();
        let _ = selection.remove_all_ranges();

        let range = document.create_range().unwrap();
        let _ = range.select_node_contents(&el);

        let _ = selection.add_range(&range);

        let document = document.dyn_into::<web_sys::HtmlDocument>().unwrap();
        let _ = document.exec_command("copy");
    }
}

/// Copies `text` to a user's clipboard via the `Clipboard` API.
pub fn copy_text_to_clipboard(text: &str) {
    let text = text.to_owned();

    wasm_bindgen_futures::spawn_local(async move {
        let future: wasm_bindgen_futures::JsFuture = gloo_utils::window()
            .navigator()
            .clipboard()
            .write_text(&text)
            .into();

        let res = future.await.map_err(convert_js_error);

        if let Err(e) = res {
            log::error!("could not copy to clipboard: {e:#}");
        }
    });
}
