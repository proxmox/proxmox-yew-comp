use std::rc::Rc;
use std::sync::Mutex;

use anyhow::{bail, format_err, Error};
use percent_encoding::percent_decode_str;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use proxmox_login::{Authentication, Login, Ticket, TicketResult};
use proxmox_client::HttpApiResponse;

//use crate::percent_encoding::DEFAULT_ENCODE_SET;
use crate::ProxmoxProduct;

pub fn authentication_from_cookie(product: ProxmoxProduct) -> Option<Authentication> {
    if let Some((ticket, csrfprevention_token)) = extract_auth_from_cookie(product) {
        let ticket: Result<Ticket, _> = ticket.parse();
        if let Ok(ticket) = ticket {
            return Some(Authentication {
                api_url: String::new(),
                userid: ticket.userid().to_string(),
                ticket,
                clustername: None,
                csrfprevention_token,
            });
        }
    }

    None
}

fn extract_auth_from_cookie(product: ProxmoxProduct) -> Option<(String, String)> {
    let cookie = crate::get_cookie();
    //log::info!("COOKIE: {}", cookie);

    for part in cookie.split(';') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            // cookie value can be percent encoded
            let value = match percent_decode_str(value).decode_utf8() {
                Ok(value) => value,
                Err(_) => continue,
            };

            if product == ProxmoxProduct::PBS && key == "PBSAuthCookie" {
                let items: Vec<&str> = value.split(':').take(2).collect();
                if items[0] == "PBS" {
                    let csrf_token = crate::load_csrf_token().unwrap_or(String::new());
                    return Some((value.to_string(), csrf_token));
                }
            }
            if product == ProxmoxProduct::PVE && key == "PVEAuthCookie" {
                let items: Vec<&str> = value.split(':').take(2).collect();
                if items[0] == "PVE" {
                    let csrf_token = crate::load_csrf_token().unwrap_or(String::new());
                    return Some((value.to_string(), csrf_token));
                }
            }
            if product == ProxmoxProduct::PMG && key == "PMGAuthCookie" {
                let items: Vec<&str> = value.split(':').take(2).collect();
                if items[0] == "PMG" || items[0] == "PMGQUAR" {
                    let csrf_token = crate::load_csrf_token().unwrap_or(String::new());
                    return Some((value.to_string(), csrf_token));
                }
            }
        }
    }

    None
}

pub struct HttpClientWasm {
    product: ProxmoxProduct,
    auth: Mutex<Option<Authentication>>,
}

impl HttpClientWasm {
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

    pub fn set_auth(&self, auth: Authentication) {
        let cookie = auth.ticket.cookie();
        crate::set_cookie(&cookie);
        crate::store_csrf_token(&auth.csrfprevention_token);
        *self.auth.lock().unwrap() = Some(auth);
    }

    pub fn get_auth(&self) -> Option<Authentication> {
        self.auth.lock().unwrap().clone()
    }

    pub fn clear_auth(&self) {
        *self.auth.lock().unwrap() = None;
    }

    pub async fn get<P: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        data: Option<P>,
    ) -> Result<T, Error> {
        let req = Self::request_builder("GET", path, data)?;
        self.api_request(req).await
    }

    pub async fn delete<P: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        data: Option<P>,
    ) -> Result<T, Error> {
        let req = Self::request_builder("DELETE", path, data)?;
        self.api_request(req).await
    }

    pub async fn post<P: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        data: Option<P>,
    ) -> Result<T, Error> {
        let req = Self::request_builder("POST", path, data)?;
        self.api_request(req).await
    }

    pub async fn put<P: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        data: Option<P>,
    ) -> Result<T, Error> {
        let req = Self::request_builder("PUT", path, data)?;
        self.api_request(req).await
    }

    pub async fn login(
        &self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<TicketResult, Error> {
        let username = username.into();
        let password = password.into();

        let login = Login::new("", username, password);
        let request = login.request();
        let request =
            Self::post_request_builder(&request.url, &request.content_type, &request.body)?;
        let resp = self.api_request_text(request).await?;

        Ok(login.response(&resp)?)
    }

    pub async fn login_tfa(
        &self,
        challenge: Rc<proxmox_login::SecondFactorChallenge>,
        request: proxmox_login::Request,
    ) -> Result<Authentication, Error> {
        let request =
            Self::post_request_builder(&request.url, &request.content_type, &request.body)?;
        let resp = self.api_request_text(request).await?;
        Ok(challenge.response(&resp)?)
    }

    // This is useful to create web_sys::Request from proxmox-login::Request
    fn post_request_builder(
        url: &str,
        content_type: &'static str,
        data: &str,
    ) -> Result<web_sys::Request, Error> {
        let mut init = web_sys::RequestInit::new();
        init.method("POST");

        let js_headers = web_sys::Headers::new().map_err(|err| format_err!("{:?}", err))?;

        js_headers
            .append("cache-control", "no-cache")
            .map_err(|err| format_err!("{:?}", err))?;

        js_headers
            .append("content-type", content_type)
            .map_err(|err| format_err!("{:?}", err))?;

        init.body(Some(&wasm_bindgen::JsValue::from_str(&data)));
        init.headers(&js_headers);

        web_sys::Request::new_with_str_and_init(&url, &init).map_err(|err| format_err!("{:?}", err))
    }

    fn request_builder<P: Serialize>(
        method: &str,
        url: &str,
        data: Option<P>,
    ) -> Result<web_sys::Request, Error> {
        let mut init = web_sys::RequestInit::new();
        init.method(method);

        let js_headers = web_sys::Headers::new().map_err(|err| format_err!("{:?}", err))?;

        js_headers
            .append("cache-control", "no-cache")
            .map_err(|err| format_err!("{:?}", err))?;

        if method == "POST" || method == "PUT" {
            if let Some(data) = data {
                js_headers
                    .append("content-type", "application/json")
                    .map_err(|err| format_err!("{:?}", err))?;
                let body: Vec<u8> = serde_json::to_vec(&data)
                    .map_err(|err| format_err!("serialize failure: {}", err))?;
                let js_body = js_sys::Uint8Array::new_with_length(body.len() as u32);
                js_body.copy_from(&body);
                init.body(Some(&js_body));
            }
            web_sys::Request::new_with_str_and_init(url, &init)
                .map_err(|err| format_err!("{:?}", err))
        } else {
            if let Some(data) = data {
                js_headers
                    .append("content-type", "application/x-www-form-urlencoded")
                    .map_err(|err| format_err!("{:?}", err))?;
                let data = serde_json::to_value(data)
                    .map_err(|err| format_err!("serialize failure: {}", err))?;
                let query = json_object_to_query(data)?;
                let url = format!("{}?{}", url, query);
                web_sys::Request::new_with_str_and_init(&url, &init)
                    .map_err(|err| format_err!("{:?}", err))
            } else {
                web_sys::Request::new_with_str_and_init(url, &init)
                    .map_err(|err| format_err!("{:?}", err))
            }
        }
    }

    async fn api_request<T: DeserializeOwned>(
        &self,
        request: web_sys::Request,
    ) -> Result<T, Error> {
        let response = self.request(request).await?;

        if !(response.status >= 200 && response.status < 300) {
            bail!("HTTP status {}", response.status);
        }

        serde_json::from_slice(&response.body).map_err(|err| format_err!("invalid json: {}", err))
    }

    async fn api_request_text(&self, request: web_sys::Request) -> Result<String, Error> {
        let response = self.request(request).await?;

        if !(response.status >= 200 && response.status < 300) {
            bail!("HTTP status {}", response.status);
        }

        let text = String::from_utf8(response.body)?;

        return Ok(text);
    }

    async fn request(
        &self,
        request: web_sys::Request,
    ) -> Result<HttpApiResponse, Error> {
        let auth = self.get_auth();
        let headers = request.headers();

        if let Some(auth) = &auth {
            headers
                .append("CSRFPreventionToken", &auth.csrfprevention_token)
                .map_err(|err| format_err!("{:?}", err))?;

            let cookie = auth.ticket.cookie();
            crate::set_cookie(&cookie);
        }

        let window = web_sys::window().ok_or_else(|| format_err!("unable to get window object"))?;
        let promise = window.fetch_with_request(&request);
        let js_fut = wasm_bindgen_futures::JsFuture::from(promise);
        let js_resp = js_fut.await.map_err(|err| format_err!("{:?}", err))?;
        let resp: web_sys::Response = js_resp.into();

        web_sys_response_to_http_api_response(resp).await
    }
}

async fn web_sys_response_to_http_api_response(
    response: web_sys::Response,
) -> Result<HttpApiResponse, Error> {
    let promise = response
        .array_buffer()
        .map_err(|err| format_err!("{:?}", err))?;

    let js_fut = wasm_bindgen_futures::JsFuture::from(promise);
    let body = js_fut.await.map_err(|err| format_err!("{:?}", err))?;
    let body = js_sys::Uint8Array::new(&body).to_vec();

    Ok(HttpApiResponse {
        status: response.status(),
        content_type: response.headers().get("content-type").unwrap_or(None),
        body,
    })
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
