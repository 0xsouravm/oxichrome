//! What a `#[oxichrome::devtools_panel]` page can do with the tab its devtools inspect.
//!
//! These work only inside a devtools panel page, where both browsers provide
//! `chrome.devtools`. Elsewhere they return an error (or `None`).

use serde::de::DeserializeOwned;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;

fn not_here() -> OxichromeError {
    OxichromeError::InvalidArgument("`devtools` works only in a `#[oxichrome::devtools_panel]` page".into())
}

/// The id of the tab these devtools inspect, or `None` outside a devtools panel.
pub fn inspected_tab_id() -> Option<i32> {
    js_bridge::devtools_tab_id().as_f64().map(|id| id as i32)
}

/// Runs `code` in the inspected page, in the page's own world (it sees the page's
/// variables), and reads its last value as `T`.
///
/// The value travels as JSON, so both browsers give the same result: functions and
/// `undefined` inside objects are dropped, and a promise isn't awaited. A page that
/// throws gives an error with the first line of what it threw, like `Error: boom`.
/// No host permission is needed. Browser pages and `about:blank` refuse in Chrome.
pub async fn eval<T: DeserializeOwned>(code: &str) -> Result<T> {
    let promise = js_bridge::devtools_eval(code);
    let raw = match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(raw) => raw,
        Err(err) => return Err(eval_error(&JsError::new(err).to_string())),
    };
    let text = raw.as_string().unwrap_or_else(|| "null".into());
    serde_json::from_str(&text).map_err(|err| OxichromeError::UnexpectedValue(format!("the page's value doesn't read as the type asked for: {err}")))
}

#[derive(Debug, PartialEq)]
enum Failure {
    NotHere,
    Threw(String),
    Refused,
    Other(String),
}

fn classify(message: &str) -> Failure {
    if message == "oxichrome:no-devtools" {
        return Failure::NotHere;
    }
    if let Some(thrown) = message.strip_prefix("oxichrome:threw:") {
        return Failure::Threw(thrown.lines().next().unwrap_or(thrown).trim().to_string());
    }
    if let Some(details) = message.strip_prefix("oxichrome:refused:") {
        if details.contains("Permission denied") {
            return Failure::Refused;
        }
        return Failure::Other(format!("the browser couldn't run the code: {details}"));
    }
    Failure::Other(message.to_string())
}

fn eval_error(message: &str) -> OxichromeError {
    let js = |text: String| OxichromeError::Js(JsError::new(JsValue::from_str(&text)));
    match classify(message) {
        Failure::NotHere => not_here(),
        Failure::Threw(first) => js(format!("the inspected page threw: {first}")),
        Failure::Refused => OxichromeError::InvalidArgument("the browser doesn't let extensions run code in this page (a browser page or about:blank)".into()),
        Failure::Other(text) => js(text),
    }
}

/// Reloads the inspected page. `ignore_cache` fetches everything again.
pub fn reload(ignore_cache: bool) -> Result<()> {
    if js_bridge::devtools_reload(ignore_cache) {
        Ok(())
    } else {
        Err(not_here())
    }
}

/// The devtools color theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

/// The theme the devtools use, or `None` outside a devtools panel.
pub fn theme() -> Option<Theme> {
    match js_bridge::devtools_theme().as_string()?.as_str() {
        "dark" => Some(Theme::Dark),
        _ => Some(Theme::Light),
    }
}

/// A listener added with one of the `on_*` functions here.
pub struct Listener {
    remove: js_sys::Function,
}

impl Listener {
    /// Stops listening.
    pub fn remove(self) {
        let _ = self.remove.call0(&JsValue::NULL);
    }
}

fn listen(kind: &str, callback: JsValue) -> Result<Listener> {
    let remove = js_bridge::devtools_listen(kind, &callback);
    if remove.is_function() {
        Ok(Listener { remove: remove.into() })
    } else {
        Err(not_here())
    }
}

/// Runs `handler` each time the user opens this panel's tab, and once right away if it's
/// showing now (the panel is usually shown before its page has loaded).
pub fn on_shown(handler: impl Fn() + 'static) -> Result<Listener> {
    listen("shown", Closure::<dyn Fn()>::new(handler).into_js_value())
}

/// Runs `handler` each time the user leaves this panel's tab.
pub fn on_hidden(handler: impl Fn() + 'static) -> Result<Listener> {
    listen("hidden", Closure::<dyn Fn()>::new(handler).into_js_value())
}

/// Runs `handler` with the new URL each time the inspected page navigates.
pub fn on_navigated(handler: impl Fn(String) + 'static) -> Result<Listener> {
    let callback = Closure::<dyn Fn(JsValue)>::new(move |url: JsValue| handler(url.as_string().unwrap_or_default())).into_js_value();
    listen("navigated", callback)
}

/// A request the inspected page made, from the network panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub url: String,
    pub method: String,
    /// The response status, `0` when there was no response.
    pub status: u16,
    /// The response's MIME type, like `text/html`.
    pub mime_type: String,
    /// How long the request took, in milliseconds.
    pub time: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRequest {
    url: String,
    method: String,
    #[serde(default)]
    status: f64,
    #[serde(default)]
    mime_type: Option<String>,
    #[serde(default)]
    time: Option<f64>,
}

/// Runs `handler` for each request the inspected page finishes, while the devtools are open.
pub fn on_request_finished(handler: impl Fn(Request) + 'static) -> Result<Listener> {
    let callback = Closure::<dyn Fn(JsValue)>::new(move |raw: JsValue| {
        if let Ok(raw) = crate::json::from_js::<RawRequest>(&raw) {
            handler(Request {
                url: raw.url,
                method: raw.method,
                status: raw.status.max(0.0) as u16,
                mime_type: raw.mime_type.unwrap_or_default(),
                time: raw.time.unwrap_or(0.0).max(0.0),
            });
        }
    })
    .into_js_value();
    listen("request", callback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_errors_read_the_same_in_both_browsers() {
        let chrome = classify("oxichrome:threw:Error: boom\n    at <anonymous>:1:7");
        let firefox = classify("oxichrome:threw:Error: boom\n\t@debugger eval called from moz-extension://x/ - eval code:1:7\n");
        assert_eq!(chrome, Failure::Threw("Error: boom".into()));
        assert_eq!(chrome, firefox);
        assert_eq!(classify("oxichrome:refused:Operation failed: %s Permission denied"), Failure::Refused);
        assert_eq!(classify("oxichrome:no-devtools"), Failure::NotHere);
        assert!(not_here().to_string().contains("devtools_panel"));
    }
}
