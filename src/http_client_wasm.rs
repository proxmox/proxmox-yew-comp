use std::sync::Mutex;

use anyhow::{bail, format_err, Error};
use percent_encoding::percent_encode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::percent_encoding::DEFAULT_ENCODE_SET;
use crate::ProxmoxProduct;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct LoginInfo {
    pub username: String,
    pub CSRFPreventionToken: String,
    pub ticket: String,
}

impl LoginInfo {
    pub fn from_cookie(product: ProxmoxProduct) -> Option<Self> {
        if let Some((auth_id, ticket, csrf_token)) = Self::extract_auth_from_cookie(product) {
            if !auth_id.is_empty() {
                //log::info!("HAS COOKIE {} {}", ticket, csrf_token);
                return Some(LoginInfo {
                    username: auth_id,
                    CSRFPreventionToken: csrf_token,
                    ticket: ticket,
                });
            }
        }

        None
    }

    fn extract_auth_from_cookie(product: ProxmoxProduct) -> Option<(String, String, String)> {
        let cookie = crate::get_cookie();
        //log::info!("COOKIE: {}", cookie);

        for part in cookie.split(';') {
            let part = part.trim();
            if let Some((key, value)) = part.split_once('=') {
                if product == ProxmoxProduct::PBS && key == "PBSAuthCookie" {
                    let items: Vec<&str> = value.split(':').take(2).collect();
                    if items[0] == "PBS" {
                        let csrf_token = crate::load_csrf_token().unwrap_or(String::new());
                        return Some((items[1].to_string(), value.to_string(), csrf_token));
                    }
                }
                if product == ProxmoxProduct::PVE && key == "PVEAuthCookie" {
                    let items: Vec<&str> = value.split(':').take(2).collect();
                    if items[0] == "PVE" {
                        let csrf_token = crate::load_csrf_token().unwrap_or(String::new());
                        return Some((items[1].to_string(), value.to_string(), csrf_token));
                    }
                }
                if product == ProxmoxProduct::PMG && key == "PMGAuthCookie" {
                    let items: Vec<&str> = value.split(':').take(2).collect();
                    if items[0] == "PMG" || items[0] == "PMGQUAR" {
                        let csrf_token = crate::load_csrf_token().unwrap_or(String::new());
                        return Some((items[1].to_string(), value.to_string(), csrf_token));
                    }
                }
            }
        }

        None
    }
}

pub struct HttpClient {
    product: ProxmoxProduct,
    auth: Mutex<Option<LoginInfo>>,
}

impl HttpClient {
    pub fn new(product: ProxmoxProduct) -> Self {
        Self {
            product,
            auth: Mutex::new(None),
       }
    }

    pub fn product(&self) -> ProxmoxProduct {
        self.product
    }

    pub fn set_product(&mut self, product: ProxmoxProduct) {
        self.product = product;
    }

    pub fn set_auth(&self, info: LoginInfo) {
        *self.auth.lock().unwrap() = Some(info);
    }

    pub fn clear_auth(&self) {
        *self.auth.lock().unwrap() = None;
    }

    pub async fn get(&self, path: &str, data: Option<Value>) -> Result<Value, Error> {
        let req = Self::request_builder("GET", path, data)?;
        self.request(req).await
    }

    pub async fn delete(&self, path: &str, data: Option<Value>) -> Result<Value, Error> {
        let req = Self::request_builder("DELETE", path, data)?;
        self.request(req).await
    }

    pub async fn post(&self, path: &str, data: Option<Value>) -> Result<Value, Error> {
        let req = Self::request_builder("POST", path, data)?;
        self.request(req).await
    }

    pub async fn put(&self, path: &str, data: Option<Value>) -> Result<Value, Error> {
        let req = Self::request_builder("PUT", path, data)?;
        self.request(req).await
    }

    pub async fn login(
        &self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<LoginInfo, Error> {
        let username = username.into();
        let password = password.into();

        if let Some(auth) = self.auth.lock().unwrap().clone() {
            return Ok(auth);
        }

        let data = json!({ "username": username, "password": password });
        let req = Self::request_builder("POST", "/api2/json/access/ticket", Some(data))?;
        let mut resp = self.api_request(req).await?;

        let data = resp["data"].take();
        let info: LoginInfo = serde_json::from_value(data)?;

        let enc_ticket = percent_encode(info.ticket.as_bytes(), DEFAULT_ENCODE_SET);
        crate::set_auth_cookie(self.product.auth_cookie_name(), &enc_ticket.to_string());
        crate::store_csrf_token(&info.CSRFPreventionToken);

        *self.auth.lock().unwrap() = Some(info.clone());

        Ok(info)
    }

    fn request_builder(
        method: &str,
        url: &str,
        data: Option<Value>,
    ) -> Result<web_sys::Request, Error> {
        let mut init = web_sys::RequestInit::new();
        init.method(method);

        let js_headers = web_sys::Headers::new().map_err(|err| format_err!("{:?}", err))?;

        js_headers
            .append("cache-control", "no-cache")
            .map_err(|err| format_err!("{:?}", err))?;

        let url_with_data = if let Some(data) = data {
            if method == "POST" {
                let body = data.to_string();
                js_headers
                    .append("content-type", "application/json")
                    .map_err(|err| format_err!("{:?}", err))?;
                init.body(Some(&wasm_bindgen::JsValue::from_str(&body)));
                url.to_string()
            } else {
                js_headers
                    .append("content-type", "application/x-www-form-urlencoded")
                    .map_err(|err| format_err!("{:?}", err))?;
                let query = json_object_to_query(data)?;
                format!("{}?{}", url, query)
            }
        } else {
            js_headers
                .append("content-type", "application/x-www-form-urlencoded")
                .map_err(|err| format_err!("{:?}", err))?;
            url.to_string()
        };

        init.headers(&js_headers);

        let js_req = web_sys::Request::new_with_str_and_init(&url_with_data, &init)
            .map_err(|err| format_err!("{:?}", err))?;

        Ok(js_req)
    }

    async fn request(&self, js_req: web_sys::Request) -> Result<Value, Error> {
        let auth = self.auth.lock().unwrap().clone();

        if auth.is_none() {
            bail!("client is not authenticated - please login first");
        }

        if let Some(auth) = &auth {
            let headers = js_req.headers();
            headers
                .append("CSRFPreventionToken", &auth.CSRFPreventionToken)
                .map_err(|err| format_err!("{:?}", err))?;

            if auth.username.contains('!')
            /* is_token */
            {
                let enc_api_token = format!(
                    "PBSAPIToken {}:{}",
                    auth.username,
                    percent_encode(auth.ticket.as_bytes(), DEFAULT_ENCODE_SET),
                );
                headers
                    .append("Authorization", &enc_api_token)
                    .map_err(|err| format_err!("{:?}", err))?;
            } else {
                let enc_ticket = percent_encode(auth.ticket.as_bytes(), DEFAULT_ENCODE_SET);
                crate::set_auth_cookie(self.product.auth_cookie_name(), &enc_ticket.to_string());
            }
        }

        self.api_request(js_req).await
    }

    async fn api_request(&self, js_req: web_sys::Request) -> Result<Value, Error> {
        let window = web_sys::window().ok_or_else(|| format_err!("unable to get window object"))?;

        let promise = window.fetch_with_request(&js_req);
        let js_fut = wasm_bindgen_futures::JsFuture::from(promise);
        let js_resp = js_fut.await.map_err(|err| format_err!("{:?}", err))?;

        let resp: web_sys::Response = js_resp.into();

        let promise = resp.text().map_err(|err| format_err!("{:?}", err))?;

        let js_fut = wasm_bindgen_futures::JsFuture::from(promise);
        let body = js_fut.await.map_err(|err| format_err!("{:?}", err))?;

        //web_sys::console::log_1(&body);

        let text = body
            .as_string()
            .ok_or_else(|| format_err!("Got non-utf8-string response"))?;

        if resp.ok() {
            if text.is_empty() {
                return Ok(Value::Null);
            }

            let json =
                serde_json::from_str(&text).map_err(|err| format_err!("invalid json: {}", err))?;

            Ok(json)
        } else {
            bail!("HTTP status {}: {}", resp.status(), resp.status_text());
        }
    }
}

pub fn json_object_to_query(data: Value) -> Result<String, Error> {
    let mut query = url::form_urlencoded::Serializer::new(String::new());

    let object = data.as_object().ok_or_else(|| {
        format_err!("json_object_to_query: got wrong data type (expected object).")
    })?;

    for (key, value) in object {
        match value {
            Value::Bool(b) => {
                query.append_pair(key, &b.to_string());
            }
            Value::Number(n) => {
                query.append_pair(key, &n.to_string());
            }
            Value::String(s) => {
                query.append_pair(key, &s);
            }
            Value::Array(arr) => {
                for element in arr {
                    match element {
                        Value::Bool(b) => {
                            query.append_pair(key, &b.to_string());
                        }
                        Value::Number(n) => {
                            query.append_pair(key, &n.to_string());
                        }
                        Value::String(s) => {
                            query.append_pair(key, &s);
                        }
                        _ => bail!(
                            "json_object_to_query: unable to handle complex array data types."
                        ),
                    }
                }
            }
            _ => bail!("json_object_to_query: unable to handle complex data types."),
        }
    }

    Ok(query.finish())
}
