use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use serde_json::{json, Value};

use yew::virtual_dom::{Key, VComp, VNode};

use pwt::prelude::*;
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{
    CellConfiguration, DataTable, DataTableColumn, DataTableHeader, DataTableMouseEvent,
};
use pwt::widget::form::{Form, FormContext, TextArea};
use pwt::widget::{Button, Container, Dialog, FileButton, MessageBox, Toolbar};

use crate::common_api_types::CertificateInfo;
use crate::utils::render_epoch;
use crate::{
    ConfirmButton, EditWindow, KVGrid, KVGridRow, LoadableComponent, LoadableComponentContext,
    LoadableComponentMaster,
};

async fn upload_custom_certificate(form_ctx: FormContext) -> Result<(), Error> {
    let mut data = form_ctx.get_submit_data();
    data["force"] = true.into();
    data["restart"] = true.into();
    let _certs: Vec<CertificateInfo> =
        crate::http_post("/nodes/localhost/certificates/custom", Some(data)).await?;
    Ok(())
}

#[derive(PartialEq, Properties)]
pub struct CertificateList {}

impl Default for CertificateList {
    fn default() -> Self {
        Self::new()
    }
}

impl CertificateList {
    pub fn new() -> Self {
        Self {}
    }
}

pub enum Msg {
    Redraw,
}

#[derive(PartialEq)]
pub enum ViewState {
    CertificateView(Rc<Value>),
    UploadCustomCertificate,
    PleaseReload,
}

#[doc(hidden)]
pub struct ProxmoxCertificateList {
    selection: Selection,
    store: Store<CertificateInfo>,
    columns: Rc<Vec<DataTableHeader<CertificateInfo>>>,
    rows: Rc<Vec<KVGridRow>>,
}

impl LoadableComponent for ProxmoxCertificateList {
    type Properties = CertificateList;
    type Message = Msg;
    type ViewState = ViewState;

    fn create(ctx: &LoadableComponentContext<Self>) -> Self {
        let selection = Selection::new().on_select(ctx.link().callback(|_| Msg::Redraw));
        let store =
            Store::with_extract_key(|record: &CertificateInfo| Key::from(record.filename.clone()));
        let columns = Rc::new(columns());
        let rows = Rc::new(rows());
        Self {
            selection,
            store,
            columns,
            rows,
        }
    }

    fn load(
        &self,
        _ctx: &LoadableComponentContext<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> {
        let path = "/nodes/localhost/certificates/info".to_string();
        let store = self.store.clone();
        Box::pin(async move {
            let data = crate::http_get(&path, None).await?;
            store.write().set_data(data);
            Ok(())
        })
    }

    fn toolbar(&self, ctx: &LoadableComponentContext<Self>) -> Option<Html> {
        let selected_key = self.selection.selected_key();
        let selected_cert = match &selected_key {
            Some(selected_key) => self.store.read().lookup_record(selected_key).cloned(),
            None => None,
        };

        let toolbar = Toolbar::new()
            .class("pwt-w-100")
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(
                Button::new(tr!("Upload Custom Certificate")).onclick(
                    ctx.link()
                        .change_view_callback(|_| Some(ViewState::UploadCustomCertificate)),
                ),
            )
            .with_child(
                ConfirmButton::new(tr!("Delete Custom Certificate"))
                    .confirm_message(tr!(
                        "Are you sure you want to remove the certificate used for {0}",
                        "proxy.pem"
                    ))
                    .on_activate({
                        let link = ctx.link();
                        move |_| {
                            let link = link.clone();
                            let command_path = "/nodes/localhost/certificates/custom".to_string();
                            let data = Some(json!({"restart": true}));
                            let command_future = crate::http_delete(command_path, data);
                            link.clone().spawn(async move {
                                match command_future.await {
                                    Ok(()) => {
                                        link.change_view(Some(ViewState::PleaseReload));
                                    }
                                    Err(err) => {
                                        link.show_error(tr!("Error"), err, true);
                                    }
                                }
                            });
                        }
                    }),
            )
            .with_child(
                Button::new(tr!("View Certificate"))
                    .disabled(selected_cert.is_none())
                    .onclick({
                        let selected_cert = selected_cert.clone();
                        let link = ctx.link();
                        move |_| {
                            if let Some(selected_cert) = &selected_cert {
                                let cert_data: Value =
                                    serde_json::to_value(selected_cert.clone()).unwrap();
                                link.change_view(Some(ViewState::CertificateView(Rc::new(
                                    cert_data,
                                ))));
                            }
                        }
                    }),
            );

        Some(toolbar.into())
    }

    fn main_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        DataTable::new(self.columns.clone(), self.store.clone())
            .class("pwt-flex-fit")
            .selection(self.selection.clone())
            .on_row_dblclick({
                let store = self.store.clone();
                let link = ctx.link();
                move |event: &mut DataTableMouseEvent| {
                    let key = &event.record_key;
                    if let Some(selected_cert) = store.read().lookup_record(key).cloned() {
                        let cert_data: Value = serde_json::to_value(selected_cert.clone()).unwrap();
                        link.change_view(Some(ViewState::CertificateView(Rc::new(cert_data))));
                    };
                }
            })
            .into()
    }

    fn dialog_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        view_state: &Self::ViewState,
    ) -> Option<Html> {
        match view_state {
            ViewState::CertificateView(info) => Some(self.create_certificate_view(ctx, info)),
            ViewState::PleaseReload => Some(self.create_please_reload_view(ctx)),
            ViewState::UploadCustomCertificate => Some(self.create_upload_custom_certificate(ctx)),
        }
    }
}

impl From<CertificateList> for VNode {
    fn from(val: CertificateList) -> Self {
        let comp =
            VComp::new::<LoadableComponentMaster<ProxmoxCertificateList>>(Rc::new(val), None);
        VNode::from(comp)
    }
}

async fn update_field_from_file(
    form_ctx: FormContext,
    field_name: &'static str,
    file_list: Option<web_sys::FileList>,
) {
    if let Some(file_list) = file_list {
        if let Some(file) = file_list.get(0) {
            let text_future = wasm_bindgen_futures::JsFuture::from(file.text());
            let form_ctx = form_ctx.clone();
            match text_future.await {
                Ok(text) => {
                    let text = text.as_string().unwrap();
                    form_ctx
                        .write()
                        .set_field_value(field_name, Value::String(text));
                }
                Err(err) => {
                    log::error!("File::text(): {err:?}");
                }
            }
        }
    }
}

impl ProxmoxCertificateList {
    fn create_upload_custom_certificate(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let link = ctx.link();
        EditWindow::new(tr!("Upload Custom Certificate"))
            .width(600)
            .on_close(ctx.link().change_view_callback(|_| None))
            .submit_text(tr!("Upload"))
            .renderer(move |form_ctx: &FormContext| {
                Form::new()
                    .padding(2)
                    .class("pwt-gap-2 pwt-flex-direction-column")
                    .form_context(form_ctx.clone())
                    .with_child(html! {<span>{tr!("Private Key (Optional)")}</span>})
                    .with_child(
                        TextArea::new()
                            .attribute("rows", "4")
                            .name("key")
                            .placeholder(tr!("No change")),
                    )
                    .with_child(
                        FileButton::new(tr!("From File"))
                            .class("pwt-align-self-flex-start pwt-scheme-primary")
                            .on_change({
                                let form_ctx = form_ctx.clone();
                                let link = link.clone();
                                move |file_list: Option<web_sys::FileList>| {
                                    link.spawn(update_field_from_file(
                                        form_ctx.clone(),
                                        "key",
                                        file_list,
                                    ));
                                }
                            }),
                    )
                    .with_child(
                        Container::from_tag("span")
                            .padding_top(4)
                            .with_child(tr!("Certificate Chain")),
                    )
                    .with_child(
                        TextArea::new()
                            .required(true)
                            .attribute("rows", "4")
                            .name("certificates"),
                    )
                    .with_child(
                        FileButton::new(tr!("From File"))
                            .class("pwt-align-self-flex-start pwt-scheme-primary")
                            .on_change({
                                let form_ctx = form_ctx.clone();
                                let link = link.clone();
                                move |file_list: Option<web_sys::FileList>| {
                                    link.spawn(update_field_from_file(
                                        form_ctx.clone(),
                                        "certificates",
                                        file_list,
                                    ));
                                }
                            }),
                    )
                    .into()
            })
            .on_submit({
                let link = ctx.link();
                move |form_ctx: FormContext| {
                    let link = link.clone();
                    async move {
                        upload_custom_certificate(form_ctx).await?;
                        link.change_view(Some(ViewState::PleaseReload));
                        Ok(())
                    }
                }
            })
            .into()
    }

    fn create_please_reload_view(&self, ctx: &LoadableComponentContext<Self>) -> Html {
        let msg = tr!(
            "API server will be restarted to use new certificates, please reload web-interface!"
        );

        MessageBox::new(tr!("Please Reload"), msg)
            .on_close(ctx.link().change_view_callback(|_| None))
            .into()
    }

    fn create_certificate_view(
        &self,
        ctx: &LoadableComponentContext<Self>,
        info: &Rc<Value>,
    ) -> Html {
        let grid = KVGrid::new()
            .class("pwt-flex-fit")
            .borderless(true)
            .striped(false)
            .rows(self.rows.clone())
            .data(Rc::clone(info))
            .cell_configuration(
                CellConfiguration::new()
                    .class("pwt-datatable-cell pwt-user-select-text")
                    .padding(2),
            );

        Dialog::new(tr!("Certificate"))
            .with_child(grid)
            .on_close(ctx.link().change_view_callback(|_| None))
            .into()
    }
}

fn rows() -> Vec<KVGridRow> {
    let render_date = |_name: &str, value: &Value, _record: &Value| match value.as_i64() {
        Some(value) => html! {render_epoch(value)},
        None => html! {value.to_string()},
    };

    vec![
        KVGridRow::new("filename", tr!("File")),
        KVGridRow::new("fingerprint", tr!("Fingerprint")),
        KVGridRow::new("issuer", tr!("Issuer")),
        KVGridRow::new("subject", tr!("Subject")),
        KVGridRow::new("public-key-type", tr!("Public Key Alogrithm")),
        KVGridRow::new("public-key-bits", tr!("Public Key Size")),
        KVGridRow::new("notbefore", tr!("Valid Since")).renderer(render_date),
        KVGridRow::new("notafter", tr!("Expires")).renderer(render_date),
        KVGridRow::new("san", tr!("Subject Alternative Names")).renderer(
            |_name, value, _record| {
                let list: Result<Vec<String>, _> = serde_json::from_value(value.clone());
                match list {
                    Ok(value) => html! {<pre>{&value.join("\n")}</pre>},
                    _ => html! {value.to_string()},
                }
            },
        ),
        KVGridRow::new("pem", tr!("Certificate")).renderer(|_name, value, _record| {
            match value.as_str() {
                Some(value) => html! {<pre class="pwt-font-monospace">{&value}</pre>},
                _ => html! {value.to_string()},
            }
        }),
    ]
}

fn columns() -> Vec<DataTableHeader<CertificateInfo>> {
    vec![
        DataTableColumn::new(tr!("File"))
            .width("150px")
            .render(|item: &CertificateInfo| html! { &item.filename })
            .into(),
        DataTableColumn::new(tr!("Issuer"))
            .flex(1)
            .render(|item: &CertificateInfo| html! {&item.issuer})
            .into(),
        DataTableColumn::new(tr!("Subject"))
            .flex(1)
            .render(|item: &CertificateInfo| html! {&item.subject})
            .into(),
        DataTableColumn::new(tr!("Public Key Alogrithm"))
            .hidden(true)
            .render(|item: &CertificateInfo| html! {&item.public_key_type})
            .into(),
        DataTableColumn::new(tr!("Public Key Size"))
            .hidden(true)
            .render(|item: &CertificateInfo| match item.public_key_bits {
                Some(bits) => html! {bits},
                None => html! {"-"},
            })
            .into(),
        DataTableColumn::new(tr!("Valid Since"))
            .width("150px")
            .render(|item: &CertificateInfo| match item.notbefore {
                Some(notbefore) => html! {render_epoch(notbefore)},
                None => html! {"-"},
            })
            .into(),
        DataTableColumn::new(tr!("Expires"))
            .width("150px")
            .render(|item: &CertificateInfo| match item.notafter {
                Some(notafter) => html! {render_epoch(notafter)},
                None => html! {"-"},
            })
            .into(),
        DataTableColumn::new(tr!("Subject Alternative Names"))
            .flex(1)
            .render(|item: &CertificateInfo| {
                html! {<pre>{&item.san.join("\n")}</pre>}
            })
            .into(),
        DataTableColumn::new(tr!("Fingerprint"))
            .hidden(true)
            .render(|item: &CertificateInfo| match &item.fingerprint {
                Some(fingerprint) => html! {fingerprint},
                None => html! {"-"},
            })
            .into(),
    ]
}
