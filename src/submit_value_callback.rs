use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::Error;
use derivative::Derivative;
use serde_json::Value;

/// A [SubmitValueCallback] is an async callback ([Future]) that gets the
/// [Value] as parameter, returning the [Result] of the submit
/// opertation.
///
/// We currently use this for the [Wizard](super::Wizard).
#[derive(Derivative)]
#[derivative(Clone, PartialEq)]
pub struct SubmitValueCallback(
    #[derivative(PartialEq(compare_with = "Rc::ptr_eq"))]
    Rc<dyn Fn(Value) -> Pin<Box<dyn Future<Output = Result<(), Error>>>>>,
);

impl SubmitValueCallback {
    pub fn new<F, R>(callback: F) -> Self
    where
        F: 'static + Fn(Value) -> R,
        R: 'static + Future<Output = Result<(), Error>>,
    {
        Self(Rc::new(move |state: Value| {
            let future = callback(state);
            Box::pin(future)
        }))
    }

    pub async fn apply(&self, data: Value) -> Result<(), Error> {
        (self.0)(data).await
    }
}

/// Helper trait to create an optional [SubmitValueCallback] property.
pub trait IntoSubmitValueCallback {
    fn into_submit_value_callback(self) -> Option<SubmitValueCallback>;
}

impl IntoSubmitValueCallback for Option<SubmitValueCallback> {
    fn into_submit_value_callback(self) -> Option<SubmitValueCallback> {
        self
    }
}

impl<F, R> IntoSubmitValueCallback for F
where
    F: 'static + Fn(Value) -> R,
    R: 'static + Future<Output = Result<(), Error>>,
{
    fn into_submit_value_callback(self) -> Option<SubmitValueCallback> {
        Some(SubmitValueCallback::new(self))
    }
}
