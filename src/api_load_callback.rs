use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use serde::de::DeserializeOwned;

use yew::html::IntoPropValue;
use yew::AttrValue;

use proxmox_client::ApiResponseData;

/// Api Load Callback
///
/// Similar to [pwt::props::LoadCallback], but return [ApiResponseData] to
/// allow access to additional properties like digest.

pub struct ApiLoadCallback<T> {
    callback: Rc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<ApiResponseData<T>, Error>>>>>,
    url: Option<AttrValue>, // only used for change tracking
}

impl<T> ApiLoadCallback<T> {
    pub fn new<F, R>(callback: F) -> Self
    where
        F: 'static + Fn() -> R,
        R: 'static + Future<Output = Result<ApiResponseData<T>, Error>>,
    {
        Self {
            url: None,
            callback: Rc::new(move || {
                let future = callback();
                Box::pin(future)
            }),
        }
    }

    pub fn url(mut self, url: impl IntoPropValue<Option<AttrValue>>) -> Self {
        self.set_url(url);
        self
    }

    pub fn set_url(&mut self, url: impl IntoPropValue<Option<AttrValue>>) {
        self.url = url.into_prop_value();
    }

    /// Mark the callback as static (disable change detection).
    ///
    /// Useful for callback which always returns the same data.
    pub fn static_callback(self) -> Self {
        // Simply set a fixed url
        self.url("__static__")
    }

    pub async fn apply(&self) -> Result<ApiResponseData<T>, Error> {
        (self.callback)().await
    }
}

impl<T> Clone for ApiLoadCallback<T> {
    fn clone(&self) -> Self {
        Self {
            callback: Rc::clone(&self.callback),
            url: self.url.clone(),
        }
    }
}

impl<T> PartialEq for ApiLoadCallback<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.url.is_some() && other.url.is_some() {
            return self.url == other.url;
        }

        Rc::ptr_eq(&self.callback, &other.callback)
    }
}

impl<T: 'static + DeserializeOwned> From<&str> for ApiLoadCallback<T> {
    fn from(url: &str) -> Self {
        url.to_owned().into()
    }
}

impl<T: 'static + DeserializeOwned> From<AttrValue> for ApiLoadCallback<T> {
    fn from(url: AttrValue) -> Self {
        url.to_string().into()
    }
}

impl<T: 'static, F, R> From<F> for ApiLoadCallback<T>
where
    F: 'static + Fn() -> R,
    R: 'static + Future<Output = Result<ApiResponseData<T>, Error>>,
{
    fn from(callback: F) -> Self {
        ApiLoadCallback::new(callback)
    }
}

impl<T: 'static + DeserializeOwned> From<String> for ApiLoadCallback<T> {
    fn from(url: String) -> Self {
        ApiLoadCallback::new({
            let url = url.clone();
            move || {
                let url = url.clone();
                async move {
                    let data = crate::http_get_full(url.to_string(), None).await?;
                    Ok(data)
                }
            }
        })
        .url(url)
    }
}

impl<T: 'static, F, R, P> From<(F, P)> for ApiLoadCallback<T>
where
    P: Into<AttrValue>,
    F: 'static + Fn(AttrValue) -> R,
    R: 'static + Future<Output = Result<ApiResponseData<T>, Error>>,
{
    fn from(tuple: (F, P)) -> Self {
        let (callback, url) = (tuple.0, tuple.1.into());
        let callback = {
            let url = url.clone();
            move || callback(url.clone())
        };
        ApiLoadCallback::new(callback).url(url)
    }
}

/// Helper trait to create an optional [ApiLoadCallback] property.
pub trait IntoApiLoadCallback<T> {
    fn into_api_load_callback(self) -> Option<ApiLoadCallback<T>>;
}

impl<T, I: Into<ApiLoadCallback<T>>> IntoApiLoadCallback<T> for I {
    fn into_api_load_callback(self) -> Option<ApiLoadCallback<T>> {
        Some(self.into())
    }
}

impl<T, I: Into<ApiLoadCallback<T>>> IntoApiLoadCallback<T> for Option<I> {
    fn into_api_load_callback(self) -> Option<ApiLoadCallback<T>> {
        self.map(|callback| callback.into())
    }
}
