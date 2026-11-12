use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;

pub struct Offscreen<A, R> {
    name: &'static str,
    export: &'static str,
    types: PhantomData<fn(A) -> R>,
}

impl<A, R> Clone for Offscreen<A, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, R> Copy for Offscreen<A, R> {}

impl<A, R> std::fmt::Debug for Offscreen<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Offscreen").field(&self.name).finish()
    }
}

fn message(error: &JsValue) -> String {
    js_sys::Reflect::get(error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

fn browser_error(name: &str, error: JsValue) -> OxichromeError {
    let text = message(&error);
    match text.as_str() {
        "oxichrome:content-script" => OxichromeError::InvalidArgument(format!(
            "`{name}` can't run from a content script; call it from the background, the popup or another extension page"
        )),
        "oxichrome:no-offscreen" => OxichromeError::InvalidArgument(format!(
            "`{name}` has nothing to work on: this extension has no #[oxichrome::offscreen] functions, so it has no offscreen document"
        )),
        "oxichrome:inside-offscreen" => OxichromeError::InvalidArgument(format!(
            "`{name}` can't be called inside an offscreen function; open and close the offscreen document from the background or an extension page"
        )),
        _ => OxichromeError::Js(JsError::new(JsValue::from_str(&text))),
    }
}

impl<A: Serialize, R: DeserializeOwned> Offscreen<A, R> {
    #[doc(hidden)]
    pub const fn __new(name: &'static str, export: &'static str) -> Self {
        Offscreen { name, export, types: PhantomData }
    }

    pub async fn run(&self, arg: A) -> Result<R> {
        let text = serde_json::to_string(&arg).map_err(|err| OxichromeError::Serde(err.to_string()))?;
        crate::json::check_text(&text)?;
        let promise = js_bridge::offscreen_run(self.name, self.export, &text).map_err(|err| browser_error(self.name, err))?;
        let outcome = JsFuture::from(promise).await.map_err(|err| browser_error(self.name, err))?;
        let get = |key: &str| js_sys::Reflect::get(&outcome, &JsValue::from_str(key)).ok().filter(|value| !value.is_undefined());
        if let Some(err) = get("err") {
            return Err(OxichromeError::Js(JsError::new(JsValue::from_str(&format!("`{}` {}", self.name, err.as_string().unwrap_or_default())))));
        }
        let json = get("ok").and_then(|ok| ok.as_string()).unwrap_or_else(|| "null".to_string());
        serde_json::from_str(&json).map_err(|err| OxichromeError::Serde(err.to_string()))
    }
}

pub async fn close() -> Result<()> {
    let promise = js_bridge::offscreen_close().map_err(|err| browser_error("close", err))?;
    JsFuture::from(promise).await.map_err(|err| browser_error("close", err))?;
    Ok(())
}

pub async fn is_open() -> Result<bool> {
    let promise = js_bridge::offscreen_is_open().map_err(|err| browser_error("is_open", err))?;
    let value = JsFuture::from(promise).await.map_err(|err| browser_error("is_open", err))?;
    Ok(value.as_bool().unwrap_or(false))
}
