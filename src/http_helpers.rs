use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Error};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use proxmox_login::{Authentication, TicketResult};

use crate::{HttpClientWasm, ProxmoxProduct};

lazy_static::lazy_static! {
    pub static ref CLIENT: Mutex<Arc<HttpClientWasm>> = {
        Mutex::new(Arc::new(HttpClientWasm::new(ProxmoxProduct::PBS)))
    };
}

pub fn http_setup(product: ProxmoxProduct) {
    let client = HttpClientWasm::new(product);
    *CLIENT.lock().unwrap() = Arc::new(client);
}

pub fn http_set_auth(info: Authentication) {
    let client = &*CLIENT.lock().unwrap();
    client.set_auth(info);
}

pub fn http_clear_auth() {
    let client = &*CLIENT.lock().unwrap();
    client.clear_auth();
    crate::clear_auth_cookie(client.product().auth_cookie_name());
}

pub async fn http_login(
    username: impl Into<String>,
    password: impl Into<String>,
    realm: impl Into<String>,
) -> Result<TicketResult, Error> {
    let username = username.into();
    let password = password.into();
    let realm = realm.into();

    let product = CLIENT.lock().unwrap().product();
    let client = HttpClientWasm::new(product);
    let ticket_result = client.login(format!("{username}@{realm}"), password).await?;

    match ticket_result {
        TicketResult::Full(auth) => {
            client.set_auth(auth.clone());
            *CLIENT.lock().unwrap() = Arc::new(client);
            Ok(TicketResult::Full(auth))
        }
        challenge => Ok(challenge),
    }
}

pub async fn http_login_tfa(
    challenge: Rc<proxmox_login::SecondFactorChallenge>,
    request: proxmox_login::Request,
) -> Result<Authentication, Error> {
    let product = CLIENT.lock().unwrap().product();
    let client = HttpClientWasm::new(product);
    let auth = client.login_tfa(challenge, request).await?;
    client.set_auth(auth.clone());
    *CLIENT.lock().unwrap() = Arc::new(client);
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
    let client = Arc::clone(&*CLIENT.lock().unwrap());
    let resp = client.get(&format!("/api2/extjs{}", path), data).await?;
    extract_result_full(resp)
}

pub async fn http_get<T: DeserializeOwned>(path: &str, data: Option<Value>) -> Result<T, Error> {
    let client = Arc::clone(&*CLIENT.lock().unwrap());
    let resp = client.get(&format!("/api2/extjs{}", path), data).await?;
    extract_result(resp)
}

pub async fn http_delete(path: &str, data: Option<Value>) -> Result<Value, Error> {
    let client = Arc::clone(&*CLIENT.lock().unwrap());
    let resp = client.delete(&format!("/api2/extjs{}", path), data).await?;
    extract_result(resp)
}

pub async fn http_post(path: &str, data: Option<Value>) -> Result<Value, Error> {
    let client = Arc::clone(&*CLIENT.lock().unwrap());
    let resp = client.post(&format!("/api2/extjs{}", path), data).await?;
    extract_result(resp)
}

pub async fn http_put(path: &str, data: Option<Value>) -> Result<Value, Error> {
    let client = Arc::clone(&*CLIENT.lock().unwrap());
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
