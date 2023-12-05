use std::rc::Rc;
use std::cell::RefCell;
use std::thread_local;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{bail, Error};
use slab::Slab;

use proxmox_client::ApiResponseData;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use proxmox_login::{Authentication, TicketResult, ticket::Validity};
use proxmox_client::HttpApiClient;
use yew::Callback;

use crate::{HttpClientWasm, ProxmoxProduct, json_object_to_query};

static LAST_NOTIFY_EPOCH: AtomicU32 = AtomicU32::new(0);
static CLIENT_NOTIFY_EPOCH: AtomicU32 = AtomicU32::new(0);


thread_local! {
    static CLIENT: RefCell<Rc<HttpClientWasm>> = {
        start_ticket_refresh_loop();
        CLIENT_NOTIFY_EPOCH.fetch_add(1, Ordering::SeqCst);
        RefCell::new(Rc::new(
            HttpClientWasm::new(ProxmoxProduct::PBS, notify_auth_listeners)
        ))
    };
}

fn update_global_client(client: HttpClientWasm) {
    CLIENT_NOTIFY_EPOCH.fetch_add(1, Ordering::SeqCst);
    CLIENT.with(move |c| *c.borrow_mut() = Rc::new(client));
}

thread_local! {
    static AUTH_OBSERVER: RefCell<Slab<Callback<()>>> = {
        start_ticket_refresh_loop();
        RefCell::new(Slab::new())
    };
}



fn notify_auth_listeners(_: ()) {
    let last_epoch = LAST_NOTIFY_EPOCH.load(Ordering::SeqCst);
    let client_epoch = CLIENT_NOTIFY_EPOCH.load(Ordering::SeqCst);

    if last_epoch == client_epoch {
        log::info!("SUPPRESS AUTH NOTIFICATION");
        return;
    }

    log::info!("NOTIFY AUTH LISTENERS");
    LAST_NOTIFY_EPOCH.store(client_epoch, Ordering::SeqCst);

    // Note: short borrow, just clone callbacks
    let list: Vec<Callback<()>> = AUTH_OBSERVER.with(|slab| {
        slab.borrow().iter().map(|(_key, cb)| cb.clone()).collect()
    });
    for callback in list {
        callback.emit(());
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

pub fn register_auth_observer(callback: impl Into<Callback<()>>) -> AuthObserver {
    let callback = callback.into();
    AUTH_OBSERVER.with(|slab| {
        let mut slab = slab.borrow_mut();
        let key = slab.insert(callback);
        AuthObserver { key }
    })
}

fn start_ticket_refresh_loop() {
    wasm_bindgen_futures::spawn_local(async move {

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
                        if let Ok(TicketResult::Full(auth)) = client.login(&data.userid, &data.ticket.to_string()).await {
                            log::info!("ticket_refresh_loop: Got ticket update.");
                            client.set_auth(auth.clone());
                        }
                    }
                    Validity::Valid => {
                        /* do nothing  */
                    }
                }
            };

        }
    });
}


pub fn http_setup(product: ProxmoxProduct) {
    let client = HttpClientWasm::new(product, notify_auth_listeners);
    update_global_client(client);
}

pub fn http_set_auth(info: Authentication) {
    CLIENT.with(move |c| c.borrow_mut().set_auth(info));
}

pub fn http_get_auth() -> Option<Authentication> {
    CLIENT.with(move |c| c.borrow().get_auth())
}

pub fn http_clear_auth() {
    CLIENT.with(move |c| {
        c.borrow_mut().clear_auth();
        crate::clear_auth_cookie(c.borrow().product().auth_cookie_name());
    });
}

pub async fn http_login(
    username: impl Into<String>,
    password: impl Into<String>,
    realm: impl Into<String>,
) -> Result<TicketResult, Error> {
    let username = username.into();
    let password = password.into();
    let realm = realm.into();

    let product = CLIENT.with(|c| c.borrow().product());
    let client = HttpClientWasm::new(product, notify_auth_listeners);
    let ticket_result = client.login(format!("{username}@{realm}"), password).await?;

    match ticket_result {
        TicketResult::Full(auth) => {
            client.set_auth(auth.clone());
            update_global_client(client);
            Ok(TicketResult::Full(auth))
        }
        challenge => Ok(challenge),
    }
}

pub async fn http_login_tfa(
    challenge: Rc<proxmox_login::SecondFactorChallenge>,
    request: proxmox_login::Request,
) -> Result<Authentication, Error> {
    let product = CLIENT.with(|c| c.borrow().product());
    let client = HttpClientWasm::new(product, notify_auth_listeners);
    let auth = client.login_tfa(challenge, request).await?;
    client.set_auth(auth.clone());
    update_global_client(client);
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

pub async fn http_get<T: DeserializeOwned>(path: impl Into<String>, data: Option<Value>) -> Result<T, Error> {
    let resp = http_get_full(path, data).await?;
    Ok(resp.data)
}

pub async fn http_delete(path: impl Into<String>, data: Option<Value>) -> Result<(), Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));

    let path_and_query = path_and_param_to_api_url(&path.into(), data)?;

    let resp: proxmox_client::HttpApiResponse = client.delete(&path_and_query).await?;
    resp.nodata()?; // we do not expect and data here
    Ok(())
}

pub async fn http_post<T: DeserializeOwned>(path: impl Into<String>, data: Option<Value>) -> Result<T, Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));

    let path_and_query = path_and_param_to_api_url(&path.into(), None::<()>)?;

    let resp: proxmox_client::HttpApiResponse  = if let Some(data) = &data {
        client.post(&path_and_query, &data).await?
    } else {
        client.post_without_body(&path_and_query).await?
    };
    let resp: ApiResponseData<T> = resp.expect_json()?;
    Ok(resp.data)
}

pub async fn http_put<T: DeserializeOwned>(path: impl Into<String>, data: Option<Value>) -> Result<T, Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));

    let path_and_query = path_and_param_to_api_url(&path.into(), None::<()>)?;

    let resp: proxmox_client::HttpApiResponse  = if let Some(data) = &data {
        client.put(&path_and_query, &data).await?
    } else {
        client.put_without_body(&path_and_query).await?
    };
    let resp: ApiResponseData<T> = resp.expect_json()?;
    Ok(resp.data)
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
    log::info!("url {}", url);

    let mut stat: Value;
    let mut sleep_time_ms = 100;
    loop {
        stat = http_get(&url, None).await?;

        if stat["status"] != Value::from("running") {
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
