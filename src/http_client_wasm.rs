use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Mutex;

use anyhow::{bail, format_err, Error};
use percent_encoding::percent_decode_str;
use serde::Serialize;
use serde_json::Value;

use pwt::convert_js_error;

use proxmox_client::{HttpApiClient, HttpApiResponse, HttpApiResponseStream};
use proxmox_login::{Authentication, Login, Ticket, TicketResult};
use yew::{html::IntoEventCallback, Callback};

//use crate::percent_encoding::DEFAULT_ENCODE_SET;
use crate::ProjectInfo;

pub fn authentication_from_cookie(project: &dyn ProjectInfo) -> Option<Authentication> {
    if let Some((ticket, csrfprevention_token)) = extract_auth_from_cookie(project) {
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

fn extract_auth_from_cookie(project: &dyn ProjectInfo) -> Option<(String, String)> {
    let cookie = crate::get_cookie();
    //log::info!("COOKIE: {}", cookie);

    let name = project.auth_cookie_name();
    let prefixes = project.auth_cookie_prefixes();

    for part in cookie.split(';') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            // cookie value can be percent encoded
            let value = match percent_decode_str(value).decode_utf8() {
                Ok(value) => value,
                Err(_) => continue,
            };

            if key == name {
                let items: Vec<&str> = value.split(':').take(2).collect();
                if prefixes.contains(&items[0]) {
                    let csrf_token = crate::load_csrf_token().unwrap_or(String::new());
                    return Some((value.to_string(), csrf_token));
                }
            }
        }
    }

    None
}

pub struct HttpClientWasm {
    project: &'static dyn ProjectInfo,
    auth: Mutex<Option<Authentication>>,
    on_auth_failure: Option<Callback<()>>,
}

impl HttpClientWasm {
    pub fn new(
        project: &'static dyn ProjectInfo,
        on_auth_failure: impl IntoEventCallback<()>,
    ) -> Self {
        Self {
            project,
            auth: Mutex::new(None),
            on_auth_failure: on_auth_failure.into_event_callback(),
        }
    }

    pub fn product(&self) -> &'static dyn ProjectInfo {
        self.project
    }

    pub fn set_product(&mut self, project: &'static dyn ProjectInfo) {
        self.project = project;
    }

    pub fn set_auth(&self, auth: Authentication) {
        let cookie = format!("{}; SameSite=Lax; Secure;", auth.ticket.cookie());
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
        let resp = self.fetch_request_text(request).await?;

        Ok(login.response(&resp)?)
    }

    pub async fn login_tfa(
        &self,
        challenge: Rc<proxmox_login::SecondFactorChallenge>,
        request: proxmox_login::Request,
    ) -> Result<Authentication, Error> {
        let request =
            Self::post_request_builder(&request.url, &request.content_type, &request.body)?;
        let resp = self.fetch_request_text(request).await?;
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

        let js_headers = web_sys::Headers::new().map_err(|err| convert_js_error(err))?;

        js_headers
            .append("cache-control", "no-cache")
            .map_err(|err| convert_js_error(err))?;

        js_headers
            .append("content-type", content_type)
            .map_err(|err| convert_js_error(err))?;

        init.body(Some(&wasm_bindgen::JsValue::from_str(&data)));
        init.headers(&js_headers);

        web_sys::Request::new_with_str_and_init(&url, &init).map_err(|err| convert_js_error(err))
    }

    fn request_builder<P: Serialize>(
        method: &str,
        url: &str,
        data: Option<P>,
    ) -> Result<web_sys::Request, Error> {
        let mut init = web_sys::RequestInit::new();
        init.method(method);

        let js_headers = web_sys::Headers::new().map_err(|err| convert_js_error(err))?;

        js_headers
            .append("cache-control", "no-cache")
            .map_err(|err| convert_js_error(err))?;

        if method == "POST" || method == "PUT" {
            if let Some(data) = data {
                js_headers
                    .append("content-type", "application/json")
                    .map_err(|err| convert_js_error(err))?;
                let body: Vec<u8> = serde_json::to_vec(&data)
                    .map_err(|err| format_err!("serialize failure: {}", err))?;
                let js_body = js_sys::Uint8Array::new_with_length(body.len() as u32);
                js_body.copy_from(&body);
                init.body(Some(&js_body));
            }
            init.headers(&js_headers);
            web_sys::Request::new_with_str_and_init(url, &init).map_err(|err| convert_js_error(err))
        } else {
            if let Some(data) = data {
                js_headers
                    .append("content-type", "application/x-www-form-urlencoded")
                    .map_err(|err| convert_js_error(err))?;
                let data = serde_json::to_value(data)
                    .map_err(|err| format_err!("serialize failure: {}", err))?;
                let query = json_object_to_query(data)?;
                let url = format!("{}?{}", url, query);
                init.headers(&js_headers);
                web_sys::Request::new_with_str_and_init(&url, &init)
                    .map_err(|err| convert_js_error(err))
            } else {
                init.headers(&js_headers);
                web_sys::Request::new_with_str_and_init(url, &init)
                    .map_err(|err| convert_js_error(err))
            }
        }
    }

    async fn fetch_request_text(&self, request: web_sys::Request) -> Result<String, Error> {
        let response =
            web_sys_response_to_http_api_response(self.fetch_request(request).await?).await?;

        if !(response.status >= 200 && response.status < 300) {
            bail!("HTTP status {}", response.status);
        }

        let text = String::from_utf8(response.body)?;

        return Ok(text);
    }

    async fn fetch_request(&self, request: web_sys::Request) -> Result<web_sys::Response, Error> {
        let auth = self.get_auth();
        let headers = request.headers();

        if let Some(auth) = &auth {
            headers
                .append("CSRFPreventionToken", &auth.csrfprevention_token)
                .map_err(|err| convert_js_error(err))?;

            let cookie = format!("{}; SameSite=Lax; Secure;", auth.ticket.cookie());
            crate::set_cookie(&cookie);
        }

        let window = web_sys::window().ok_or_else(|| format_err!("unable to get window object"))?;
        let promise = window.fetch_with_request(&request);

        let js_resp = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|err| convert_js_error(err))?;

        let resp: web_sys::Response = js_resp.into();

        if resp.status() == 401 {
            log::info!("Got UNAUTHORIZED status - clearing AUTH cookie");
            self.clear_auth();
            let auth_cookie_name = self.project.auth_cookie_name();
            crate::clear_auth_cookie(auth_cookie_name);
            if let Some(on_auth_failure) = &self.on_auth_failure {
                on_auth_failure.emit(());
            }
        }

        Ok(resp)
    }
}

async fn web_sys_response_to_http_api_response(
    response: web_sys::Response,
) -> Result<HttpApiResponse, Error> {
    let promise = response
        .array_buffer()
        .map_err(|err| convert_js_error(err))?;

    let js_fut = wasm_bindgen_futures::JsFuture::from(promise);
    let body = js_fut.await.map_err(|err| convert_js_error(err))?;
    let body = js_sys::Uint8Array::new(&body).to_vec();

    let mut content_type = response.headers().get("content-type").unwrap_or(None);
    if let Some(ct) = &content_type {
        if ct.starts_with("application/json;") {
            // strip rest of information (i.e. charset=UTF8;),
            // Note: proxmox_client crate expects "application/json"
            content_type = Some(String::from("application/json"));
        }
    }
    Ok(HttpApiResponse {
        status: response.status(),
        content_type,
        body,
    })
}

async fn web_sys_response_to_http_api_stream_response(
    response: web_sys::Response,
) -> Result<HttpApiResponseStream<web_sys::ReadableStream>, Error> {
    let body = response.body();

    let mut content_type = response.headers().get("content-type").unwrap_or(None);
    if let Some(ct) = &content_type {
        if ct.starts_with("application/json;") {
            // strip rest of information (i.e. charset=UTF8;),
            // Note: proxmox_client crate expects "application/json"
            content_type = Some(String::from("application/json"));
        }
    }
    Ok(HttpApiResponseStream {
        status: response.status(),
        content_type,
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

impl HttpApiClient for HttpClientWasm {
    type ResponseFuture<'a> = Pin<Box<dyn Future<Output=Result<HttpApiResponse, proxmox_client::Error>> + 'a>>
        where Self: 'a;

    type Body = web_sys::ReadableStream;

    type ResponseStreamFuture<'a> = Pin<Box<dyn Future<Output=Result<HttpApiResponseStream<Self::Body>, proxmox_client::Error>> + 'a>>
        where Self: 'a;

    fn request<'a, T>(
        &'a self,
        method: http::Method,
        path_and_query: &'a str,
        params: Option<T>,
    ) -> Self::ResponseFuture<'a>
    where
        T: Serialize + 'a,
    {
        Box::pin(async move {
            let request = Self::request_builder(method.as_str(), path_and_query, params)
                .map_err(proxmox_client::Error::Anyhow)?;
            let response = self
                .fetch_request(request)
                .await
                .map_err(proxmox_client::Error::Anyhow)?;
            web_sys_response_to_http_api_response(response)
                .await
                .map_err(proxmox_client::Error::Anyhow)
        })
    }

    fn streaming_request<'a, T>(
        &'a self,
        method: http::Method,
        path_and_query: &'a str,
        params: Option<T>,
    ) -> Self::ResponseStreamFuture<'a>
    where
        T: Serialize + 'a,
    {
        Box::pin(async move {
            let request = Self::request_builder(method.as_str(), path_and_query, params)
                .map_err(proxmox_client::Error::Anyhow)?;
            let response = self
                .fetch_request(request)
                .await
                .map_err(proxmox_client::Error::Anyhow)?;
            web_sys_response_to_http_api_stream_response(response)
                .await
                .map_err(proxmox_client::Error::Anyhow)
        })
    }
}
