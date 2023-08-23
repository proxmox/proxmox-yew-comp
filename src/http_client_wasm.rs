use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Mutex;

use anyhow::{bail, format_err, Error};
use percent_encoding::percent_decode_str;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use proxmox_client::HttpClient;
use proxmox_login::{Authentication, Login, Ticket, TicketResult};

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
    ) -> Result<http::Request<Vec<u8>>, Error> {
        let request = http::Request::builder()
            .method("POST")
            .uri(url)
            .header("cache-control", "no-cache")
            .header("content-type", content_type)
            .body(data.as_bytes().to_vec())?;

        Ok(request)
    }

    fn request_builder<P: Serialize>(
        method: &str,
        url: &str,
        data: Option<P>,
    ) -> Result<http::Request<Vec<u8>>, Error> {
        let request = http::Request::builder()
            .method(method)
            .header("cache-control", "no-cache");

        let request = if method == "POST" {
            let body = if let Some(data) = data {
                serde_json::to_vec(&data)
                    .map_err(|err| format_err!("serialize failure: {}", err))?
            } else {
                Vec::new()
            };

            request
                .header("content-type", "application/json")
                .uri(url)
                .body(body)?
        } else {
            let url = if let Some(data) = data {
                let data = serde_json::to_value(data)
                    .map_err(|err| format_err!("serialize failure: {}", err))?;
                let query = json_object_to_query(data)?;
                format!("{}?{}", url, query)
            } else {
                url.to_string()
            };
            request
                .header("content-type", "application/x-www-form-urlencoded")
                .uri(url)
                .body(Vec::new())?
        };

        Ok(request)
    }

    async fn api_request<T: DeserializeOwned>(
        &self,
        request: http::Request<Vec<u8>>,
    ) -> Result<T, Error> {
        let response = self.request(request).await?;
        let (parts, body) = response.into_parts();

        if !parts.status.is_success() {
            bail!("HTTP status {}", parts.status);
        }

        serde_json::from_slice(&body).map_err(|err| format_err!("invalid json: {}", err))
    }

    async fn api_request_text(&self, request: http::Request<Vec<u8>>) -> Result<String, Error> {
        let response = self.request(request).await?;
        let (parts, body) = response.into_parts();

        if !parts.status.is_success() {
            bail!("HTTP status {}", parts.status);
        }

        let text = String::from_utf8(body)?;

        return Ok(text);
    }
}

async fn web_sys_response_to_http_response(
    resp: web_sys::Response,
) -> Result<http::Response<Vec<u8>>, Error> {
    let promise = resp
        .array_buffer()
        .map_err(|err| format_err!("{:?}", err))?;

    let js_fut = wasm_bindgen_futures::JsFuture::from(promise);
    let body = js_fut.await.map_err(|err| format_err!("{:?}", err))?;
    let body = js_sys::Uint8Array::new(&body).to_vec();

    let mut response = http::response::Builder::new().status(resp.status());

    if let Some(js_iter) =
        js_sys::try_iter(&resp.headers()).map_err(|err| format_err!("{:?}", err))?
    {
        for item in js_iter {
            if let Ok(item) = item {
                let item: js_sys::Array = item.into();
                if let Some(key) = item.get(0).as_string() {
                    if let Some(value) = item.get(1).as_string() {
                        //log::info!("HEADER {}: {}", key, value);
                        response = response.header(&key, &value);
                    }
                }
            }
        }
    }

    Ok(response.body(body)?)
}

async fn api_request_raw(js_req: web_sys::Request) -> Result<http::Response<Vec<u8>>, Error> {
    let window = web_sys::window().ok_or_else(|| format_err!("unable to get window object"))?;

    let promise = window.fetch_with_request(&js_req);
    let js_fut = wasm_bindgen_futures::JsFuture::from(promise);
    let js_resp = js_fut.await.map_err(|err| format_err!("{:?}", err))?;

    let resp: web_sys::Response = js_resp.into();

    web_sys_response_to_http_response(resp).await
}

impl proxmox_client::HttpClient for HttpClientWasm {
    type Error = anyhow::Error;
    type ResponseFuture =
        Pin<Box<dyn Future<Output = Result<http::response::Response<Vec<u8>>, anyhow::Error>>>>;

    fn request(&self, mut request: http::Request<Vec<u8>>) -> Self::ResponseFuture {
        let auth = self.get_auth();

        if let Some(auth) = &auth {
            if let Ok(csrfprevention_token) =
                http::HeaderValue::from_str(&auth.csrfprevention_token)
            {
                request
                    .headers_mut()
                    .insert("CSRFPreventionToken", csrfprevention_token);
            }

            let cookie = auth.ticket.cookie();
            crate::set_cookie(&cookie);
        }

        let (parts, body) = request.into_parts();

        let mut init = web_sys::RequestInit::new();
        init.method(parts.method.as_str());

        // Howto handle erors here? unwrap() is wrong!

        let js_headers = web_sys::Headers::new().unwrap();

        js_headers.append("cache-control", "no-cache").unwrap();

        for (key, value) in parts.headers.iter() {
            if let Ok(value) = value.to_str() {
                js_headers.append(key.as_str(), value).unwrap();
            }
        }

        if !body.is_empty() {
            let js_body = js_sys::Uint8Array::new_with_length(body.len() as u32);
            js_body.copy_from(&body);
            init.body(Some(&js_body));
        }

        init.headers(&js_headers);

        let js_req =
            web_sys::Request::new_with_str_and_init(&parts.uri.to_string(), &init).unwrap();

        Box::pin(async move { api_request_raw(js_req).await })
    }
}

// dummy - no really used, but required by proxmox-client api code
impl proxmox_client::Environment for HttpClientWasm {
    type Error = anyhow::Error;
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
