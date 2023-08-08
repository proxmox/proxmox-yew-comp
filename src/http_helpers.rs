use std::rc::Rc;
use std::cell::RefCell;
use std::thread_local;

use anyhow::{bail, Error};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

use proxmox_login::{Authentication, TicketResult, ticket::Validity};

use crate::{HttpClientWasm, ProxmoxProduct};

thread_local! {
    static CLIENT: RefCell<Rc<HttpClientWasm>> = {
        start_ticket_refresh_loop();
        RefCell::new(Rc::new(HttpClientWasm::new(ProxmoxProduct::PBS)))
    };
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
    let client = HttpClientWasm::new(product);
    CLIENT.with(move |c| *c.borrow_mut() = Rc::new(client));
}

pub fn http_set_auth(info: Authentication) {
    CLIENT.with(move |c| c.borrow_mut().set_auth(info));
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
    let client = HttpClientWasm::new(product);
    let ticket_result = client.login(format!("{username}@{realm}"), password).await?;

    match ticket_result {
        TicketResult::Full(auth) => {
            client.set_auth(auth.clone());
            CLIENT.with(|c| *c.borrow_mut() = Rc::new(client));
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
    let client = HttpClientWasm::new(product);
    let auth = client.login_tfa(challenge, request).await?;
    client.set_auth(auth.clone());
    CLIENT.with(|c| *c.borrow_mut() = Rc::new(client));
    Ok(auth)
}


#[derive(Deserialize)]
pub struct Metadata {
    pub success: bool,
    pub message: Option<String>,
    pub total: Option<u64>,
}

fn extract_result_full<T: DeserializeOwned>(mut resp: Value) -> Result<(T, Metadata), Error> {
    if let Some(success_integer) = resp["success"].as_u64() {
        if success_integer == 0 {
            resp["success"] = false.into();
        }
        if success_integer == 1 {
            resp["success"] = true.into();
        }
    }

    let meta = Metadata::deserialize(&resp)?;

    if !meta.success {
        let message = meta
            .message
            .unwrap_or_else(|| String::from("unknown error"));
        return Err(Error::msg(message));
    }

    let data = serde_json::from_value(resp["data"].take())?;
    Ok((data, meta))
}

fn extract_result<T: DeserializeOwned>(resp: Value) -> Result<T, Error> {
    let (data, _meta) = extract_result_full(resp)?;
    Ok(data)
}

pub async fn http_get_full<T: DeserializeOwned>(
    path: &str,
    data: Option<Value>,
) -> Result<(T, Metadata), Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));
    let resp = client.get(&format!("/api2/extjs{}", path), data).await?;
    extract_result_full(resp)
}

pub async fn http_get<T: DeserializeOwned>(path: &str, data: Option<Value>) -> Result<T, Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));
    let resp = client.get(&format!("/api2/extjs{}", path), data).await?;
    extract_result(resp)
}

pub async fn http_delete(path: &str, data: Option<Value>) -> Result<(), Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));
    let resp = client.delete(&format!("/api2/extjs{}", path), data).await?;
    extract_result(resp)
}

pub async fn http_post<T: DeserializeOwned>(path: &str, data: Option<Value>) -> Result<T, Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));
    let resp = client.post(&format!("/api2/extjs{}", path), data).await?;
    extract_result(resp)
}

pub async fn http_put<T: DeserializeOwned>(path: &str, data: Option<Value>) -> Result<T, Error> {
    let client = CLIENT.with(|c| Rc::clone(&c.borrow()));
    let resp = client.put(&format!("/api2/extjs{}", path), data).await?;
    extract_result(resp)
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
