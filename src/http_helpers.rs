use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread_local;

use anyhow::{bail, format_err, Error};
use js_sys::Uint8Array;
use pwt::{convert_js_error, AsyncAbortGuard, WebSysAbortGuard};
use slab::Slab;
use web_sys::{File, Headers, Request, RequestInit, Response};

use proxmox_client::ApiResponseData;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use proxmox_client::HttpApiClient;
use proxmox_login::{ticket::Validity, Authentication, TicketResult};
use yew::Callback;

use crate::acl_context::LocalAclTree;
use crate::{json_object_to_query, ExistingProduct, HttpClientWasm, ProjectInfo};

static LAST_NOTIFY_EPOCH: AtomicU32 = AtomicU32::new(0);
static CLIENT_NOTIFY_EPOCH: AtomicU32 = AtomicU32::new(0);

thread_local! {
    pub static CLIENT: RefCell<Rc<HttpClientWasm>> = {
        start_ticket_refresh_loop();
        CLIENT_NOTIFY_EPOCH.fetch_add(1, Ordering::SeqCst);
        RefCell::new(Rc::new(
            HttpClientWasm::new(&ExistingProduct::PBS, notify_auth_listeners)
        ))
    };
}

fn update_global_client(client: HttpClientWasm) {
    CLIENT_NOTIFY_EPOCH.fetch_add(1, Ordering::SeqCst);
    CLIENT.with(move |c| *c.borrow_mut() = Rc::new(client));
}

thread_local! {
    static AUTH_OBSERVER: RefCell<Slab<Callback<bool>>> = const { RefCell::new(Slab::new()) };
}

fn notify_auth_listeners(logout: bool) {
    let last_epoch = LAST_NOTIFY_EPOCH.load(Ordering::SeqCst);
    let client_epoch = CLIENT_NOTIFY_EPOCH.load(Ordering::SeqCst);

    if last_epoch == client_epoch {
        log::info!("SUPPRESS AUTH NOTIFICATION");
        return;
    }

    log::info!("NOTIFY AUTH LISTENERS");
    LAST_NOTIFY_EPOCH.store(client_epoch, Ordering::SeqCst);

    // Note: short borrow, just clone callbacks
    let list: Vec<Callback<bool>> =
        AUTH_OBSERVER.with(|slab| slab.borrow().iter().map(|(_key, cb)| cb.clone()).collect());
    for callback in list {
        callback.emit(logout);
    }
}

pub struct AuthObserver {
    key: usize,
}

impl Drop for AuthObserver {
    fn drop(&mut self) {
        AUTH_OBSERVER.with(|slab| {
            let mut slab = slab.borrow_mut();
            slab.remove(self.key);
        });
    }
}

pub fn register_auth_observer(callback: impl Into<Callback<bool>>) -> AuthObserver {
    let callback = callback.into();
    AUTH_OBSERVER.with(|slab| {
        let mut slab = slab.borrow_mut();
        let key = slab.insert(callback);
        AuthObserver { key }
    })
}

thread_local! {
    static TICKET_REFRESH_LOOP_GUARD: RefCell<Option<AsyncAbortGuard>> = const { RefCell::new(None) };
}

pub fn start_ticket_refresh_loop() {
    let abort_guard = AsyncAbortGuard::spawn(ticket_refresh_loop());

    // Make sure there is a single loop running.
    TICKET_REFRESH_LOOP_GUARD.with_borrow_mut(|v| *v = Some(abort_guard));
}

pub fn stop_ticket_refresh_loop() {
    TICKET_REFRESH_LOOP_GUARD.with_borrow_mut(|v| *v = None);
}

async fn ticket_refresh_loop() {
    loop {
        let sleep_time_ms = 5000;
        let future: wasm_bindgen_futures::JsFuture = crate::async_sleep(sleep_time_ms).into();
        future.await.unwrap();

        let auth = CLIENT.with(|c| c.borrow().get_auth());

        if let Some(data) = &auth {
            match data.ticket.validity() {
                Validity::Expired => {
                    log::info!("ticket_refresh_loop: Ticket is expired.");
                    http_clear_auth()
                }
                Validity::Refresh => {
                    let client = CLIENT.with(|c| Rc::clone(&*c.borrow()));

                    // if the ticket is not signed, there is no point in sending it, assume we
                    // are using a HttpOnly cookie that is properly handled by the
                    // browser/cookie anyway
                    let result = if data.ticket.is_info_only() {
                        client.refresh(&data.userid).await
                    } else {
                        client.login(&data.userid, &data.ticket.to_string()).await
                    };

                    match result {
                        // TODO: eventually deprecate support for `TicketResult::Full` and
                        // throw an error. this package should only ever be used in a browser
                        // context where authentication info should be set via HttpOnly cookies.
                        Ok(TicketResult::Full(auth)) | Ok(TicketResult::HttpOnly(auth)) => {
                            log::info!("ticket_refresh_loop: Got ticket update.");
                            client.set_auth(auth.clone());
                            LocalAclTree::load().await;
                        }
                        _ => { /* do nothing */ }
                    }
                }
                Validity::Valid => { /* do nothing  */ }
            }
        };
    }
}

pub fn http_setup(project: &'static dyn ProjectInfo) {
    let client = HttpClientWasm::new(project, notify_auth_listeners);
    update_global_client(client);
}

pub fn http_set_auth(info: Authentication) {
    CLIENT.with(move |c| c.borrow_mut().set_auth(info));
}

pub fn http_get_auth() -> Option<Authentication> {
    CLIENT.with(move |c| c.borrow().get_auth())
}

thread_local! {
    static LOGOUT_GUARD: RefCell<Option<AsyncAbortGuard>> = const { RefCell::new(None) };
}

pub fn http_clear_auth() {
    let abort_guard = AsyncAbortGuard::spawn(async move {
        let client = CLIENT.with(|c| Rc::clone(&*c.borrow()));
        let _ = client.logout().await;
        client.clear_auth();
        crate::clear_auth_cookie(client.product().auth_cookie_name());
    });

    LOGOUT_GUARD.with_borrow_mut(|v| *v = Some(abort_guard));
}

pub async fn http_login(
    username: impl Into<String>,
    password: impl Into<String>,
    realm: impl Into<String>,
) -> Result<TicketResult, proxmox_client::Error> {
    let username = username.into();
    let password = password.into();
    let realm = realm.into();

    let product = CLIENT.with(|c| c.borrow().product());
    let client = HttpClientWasm::new(product, notify_auth_listeners);
    let ticket_result = client
        .login(format!("{username}@{realm}"), password)
        .await?;

    match ticket_result {
        // TODO: eventually deprecate support for `TicketResult::Full` and
        // throw an error. this package should only ever be used in a browser
        // context where authentication info should be set via HttpOnly cookies.
        TicketResult::Full(auth) => {
            client.set_auth(auth.clone());
            update_global_client(client);
            LocalAclTree::load().await;
            Ok(TicketResult::Full(auth))
        }
        TicketResult::HttpOnly(auth) => {
            client.set_auth(auth.clone());
            update_global_client(client);
            LocalAclTree::load().await;
            Ok(TicketResult::HttpOnly(auth))
        }
        challenge => Ok(challenge),
    }
}

pub async fn http_login_tfa(
    challenge: Rc<proxmox_login::SecondFactorChallenge>,
    request: proxmox_login::Request,
) -> Result<Authentication, proxmox_client::Error> {
    let product = CLIENT.with(|c| c.borrow().product());
    let client = HttpClientWasm::new(product, notify_auth_listeners);
    let auth = client.login_tfa(challenge, request).await?;
    client.set_auth(auth.clone());
    update_global_client(client);
    LocalAclTree::load().await;
    Ok(auth)
}

fn path_and_param_to_api_url<P: Serialize>(path: &str, data: Option<P>) -> Result<String, Error> {
    let path_and_query = if let Some(data) = data {
        let data = serde_json::to_value(data)?;
        let query = json_object_to_query(data)?;
        format!("/api2/extjs{}?{}", path, query)
    } else {
        format!("/api2/extjs{}", path)
    };
    Ok(path_and_query)
}

pub async fn http_get_full<T: DeserializeOwned>(
    path: impl Into<String>,
    data: Option<Value>,
) -> Result<ApiResponseData<T>, Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));

    let path_and_query = path_and_param_to_api_url(&path.into(), data)?;

    let resp: proxmox_client::HttpApiResponse = client.get(&path_and_query).await?;
    let resp: ApiResponseData<T> = resp.expect_json()?;
    Ok(resp)
}

pub async fn http_get<T: DeserializeOwned>(
    path: impl Into<String>,
    data: Option<Value>,
) -> Result<T, Error> {
    let resp = http_get_full(path, data).await?;
    Ok(resp.data)
}

/// Delete and return data
pub async fn http_delete_get<T: DeserializeOwned>(
    path: impl Into<String>,
    data: Option<Value>,
) -> Result<T, Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));

    let path_and_query = path_and_param_to_api_url(&path.into(), data)?;

    let resp: proxmox_client::HttpApiResponse = client.delete(&path_and_query).await?;
    let resp: ApiResponseData<T> = resp.expect_json()?;
    Ok(resp.data)
}

/// Delete (no return data expected)
pub async fn http_delete(path: impl Into<String>, data: Option<Value>) -> Result<(), Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));

    let path_and_query = path_and_param_to_api_url(&path.into(), None::<()>)?;

    let resp: proxmox_client::HttpApiResponse = client
        .request(http::Method::DELETE, &path_and_query, data)
        .await?;
    resp.nodata()?; // we do not expect and data here
    Ok(())
}

pub async fn http_post<T: DeserializeOwned>(
    path: impl Into<String>,
    data: Option<Value>,
) -> Result<T, Error> {
    Ok(http_post_full(path, data).await?.data)
}

/// POST and return the full [`ApiResponseData`] so callers can inspect response attributes.
///
/// This can, for example, be use to check the post-mutation `digest` (used for
/// optimistic-concurrency chains where a follow-up write needs to pin the digest the server settled
/// on after the previous write).
pub async fn http_post_full<T: DeserializeOwned>(
    path: impl Into<String>,
    data: Option<Value>,
) -> Result<ApiResponseData<T>, Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));

    let path_and_query = path_and_param_to_api_url(&path.into(), None::<()>)?;

    let resp: proxmox_client::HttpApiResponse = if let Some(data) = &data {
        client.post(&path_and_query, &data).await?
    } else {
        client.post_without_body(&path_and_query).await?
    };
    Ok(resp.expect_json()?)
}

pub async fn http_put<T: DeserializeOwned>(
    path: impl Into<String>,
    data: Option<Value>,
) -> Result<T, Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));

    let path_and_query = path_and_param_to_api_url(&path.into(), None::<()>)?;

    let resp: proxmox_client::HttpApiResponse = if let Some(data) = &data {
        client.put(&path_and_query, &data).await?
    } else {
        client.put_without_body(&path_and_query).await?
    };
    let resp: ApiResponseData<T> = resp.expect_json()?;
    Ok(resp.data)
}

/// POST raw `bytes` to `url` with the CSRF token attached, for uploads too large for the JSON body
/// lane: proxmox-rest-server caps a parsed request body at 512 KiB, whereas a raw-body `AsyncHttp`
/// endpoint reads the stream directly. Hits the `/api2/json` endpoint so HTTP status codes come back
/// verbatim; a JSON reply's ExtJS `{ data: ... }` envelope is parsed into `T`, otherwise `Ok(None)`.
/// On a 401 the auth cookie is cleared, matching the JSON helpers.
pub async fn http_post_bytes<T: DeserializeOwned>(
    url: &str,
    bytes: Uint8Array,
    content_type: Option<&str>,
) -> Result<Option<T>, Error> {
    let window = web_sys::window().ok_or_else(|| format_err!("unable to get window object"))?;
    let headers = Headers::new().map_err(convert_js_error)?;
    headers
        .append("cache-control", "no-cache")
        .map_err(convert_js_error)?;
    headers
        .append(
            "content-type",
            content_type.unwrap_or("application/octet-stream"),
        )
        .map_err(convert_js_error)?;

    // A write without a CSRF token would be rejected server-side anyway; attach it when present.
    if let Some(auth) = http_get_auth() {
        headers
            .append("CSRFPreventionToken", &auth.csrfprevention_token)
            .map_err(convert_js_error)?;
    }

    let url = format!("/api2/json{url}");
    let abort = WebSysAbortGuard::new()?;

    let request_init = RequestInit::new();
    request_init.set_method("POST");
    request_init.set_headers(&headers);
    request_init.set_body(&bytes);
    request_init.set_signal(Some(&abort.signal()));

    let request = Request::new_with_str_and_init(&url, &request_init).map_err(convert_js_error)?;

    let resp: Response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(convert_js_error)?
        .into();

    // Reading the whole body dismisses the abort guard.
    let body = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(convert_js_error)?)
        .await
        .map_err(convert_js_error)?
        .as_string()
        .unwrap_or_default();

    if resp.status() == 401 {
        log::info!("got UNAUTHORIZED while uploading - clearing the auth cookie");
        http_clear_auth();
        bail!("could not post to '{url}' - UNAUTHORIZED");
    }
    if resp.status() != 200 {
        bail!(
            "could not post, '{}', response status {}",
            body,
            resp.status()
        );
    }

    // the server sends a parameterized media type (application/json;charset=UTF-8), so match on
    // the bare type only, like HttpClientWasm does when it normalizes a response
    let content_type = resp
        .headers()
        .get("Content-Type")
        .map_err(convert_js_error)?;
    let is_json = content_type
        .as_deref()
        .and_then(|ct| ct.split(';').next())
        .map(str::trim)
        == Some("application/json");

    if is_json {
        // The body is the usual ExtJS-shaped { data: ... } envelope.
        let data = serde_json::from_str::<Value>(&body)
            .and_then(|value| serde_json::from_value::<T>(value["data"].to_owned()))?;
        return Ok(Some(data));
    }

    Ok(None)
}

/// Read a picked `File` into a byte buffer, awaiting the result. A `File` is a `Blob`, so its
/// `array_buffer()` promise yields the contents directly - use this when an upload chain needs the
/// bytes inline rather than through a `FileReader` callback. Pair it with [`http_post_bytes`].
pub async fn read_file_bytes(file: &File) -> Result<Uint8Array, Error> {
    let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
        .await
        .map_err(convert_js_error)?;
    Ok(Uint8Array::new(&buffer))
}

/// Helper to wait for a task result
///
/// You can directly pass the result of an API call that returns a UPID.
pub async fn http_task_result(task: Result<Value, Error>) -> Result<Value, Error> {
    use crate::percent_encoding::percent_encode_component;

    let upid = match task {
        Ok(value) => match value.as_str() {
            None => bail!("http_task_result: missing UPID"),
            Some(upid) => upid.to_string(),
        },
        err => return err,
    };

    let url = format!(
        "/nodes/localhost/tasks/{}/status",
        percent_encode_component(&upid)
    );

    let mut stat: Value;
    let mut sleep_time_ms = 100;
    loop {
        stat = http_get(&url, None).await?;

        if stat["status"] != *"running" {
            break;
        }

        let future: wasm_bindgen_futures::JsFuture = crate::async_sleep(sleep_time_ms).into();
        future.await.unwrap();

        if sleep_time_ms < 1600 {
            sleep_time_ms *= 2;
        }
    }

    let status = stat["exitstatus"].as_str().unwrap_or("unknown");

    if status == "OK" {
        return Ok(Value::Null);
    }

    bail!("{}", status);
}
