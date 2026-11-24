//! Recording a tab's picture and sound. Chrome only, with `TabCapture` in `permissions`.
//!
//! Chrome lets an extension capture a tab only after the user invokes it on that tab (a
//! click on the toolbar button, a menu item or a shortcut). Get a stream id for the tab in
//! that handler with [`stream_id`], then open it with [`open`] in an extension page or an
//! offscreen document (the background service worker can't hold a media stream). Firefox
//! has no tab capture; there, pages can ask the user with `getDisplayMedia`.

use serde::Deserialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::js_bridge;
pub use crate::runtime::Listener;

fn chrome_only(what: &str) -> Result<()> {
    if js_bridge::is_firefox() {
        return Err(OxichromeError::InvalidArgument(format!(
            "Firefox has no tab capture, so `tab_capture::{what}` is Chrome only; let a page ask the user with `getDisplayMedia` there"
        )));
    }
    Ok(())
}

pub(crate) fn capture_error(error: JsValue, permission: &str) -> OxichromeError {
    let text = crate::error::JsError::new(error.clone()).to_string();
    if text == "oxichrome:no-permission" {
        return OxichromeError::InvalidArgument(format!("this capture API isn't available; add `{permission}` to `permissions` in #[oxichrome::extension]"));
    }
    if text.contains("has not been invoked") {
        return OxichromeError::InvalidArgument(format!(
            "{text} Get the stream id from a toolbar button click, menu item or shortcut on that tab."
        ));
    }
    error.into()
}

/// A stream id for `tab` (the active tab when `None`), to [`open`] within a few seconds.
/// `consumer` is the tab that will open it, when that isn't an extension page.
pub async fn stream_id(tab: Option<i32>, consumer: Option<i32>) -> Result<String> {
    chrome_only("stream_id")?;
    let promise = js_bridge::tab_capture_stream_id(tab, consumer).map_err(|error| capture_error(error, "TabCapture"))?;
    let id = JsFuture::from(promise).await.map_err(|error| capture_error(error, "TabCapture"))?;
    id.as_string().ok_or_else(|| OxichromeError::UnexpectedValue("Chrome gave no stream id".into()))
}

/// What to take from a captured tab or screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Media {
    pub video: bool,
    pub audio: bool,
}

impl Media {
    pub const VIDEO: Media = Media { video: true, audio: false };
    pub const AUDIO: Media = Media { video: false, audio: true };
    pub const BOTH: Media = Media { video: true, audio: true };
}

pub(crate) async fn open_stream(source: &str, stream_id: &str, media: Media) -> Result<web_sys::MediaStream> {
    if !media.video && !media.audio {
        return Err(OxichromeError::InvalidArgument("ask for video, audio or both".into()));
    }
    let promise = js_bridge::capture_open(source, stream_id, media.audio, media.video)?;
    Ok(JsFuture::from(promise).await?.unchecked_into())
}

/// Opens the tab's stream from [`stream_id`]. Stop its tracks to end the capture.
pub async fn open(stream_id: &str, media: Media) -> Result<web_sys::MediaStream> {
    chrome_only("open")?;
    open_stream("tab", stream_id, media).await
}

/// Where a capture is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Status {
    Pending,
    Active,
    Stopped,
    Error,
}

/// A tab being captured.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    pub tab_id: i32,
    pub status: Status,
    pub fullscreen: bool,
}

/// The tabs this extension is capturing.
pub async fn captured_tabs() -> Result<Vec<Capture>> {
    chrome_only("captured_tabs")?;
    let promise = js_bridge::tab_capture_captured().map_err(|error| capture_error(error, "TabCapture"))?;
    crate::json::from_js(&JsFuture::from(promise).await?)
}

/// Runs whenever a capture starts, stops or goes fullscreen.
pub fn on_status_changed(handler: impl Fn(Capture) + 'static) -> Result<Listener> {
    chrome_only("on_status_changed")?;
    let callback = wasm_bindgen::closure::Closure::<dyn Fn(JsValue)>::new(move |info: JsValue| {
        if let Ok(capture) = crate::json::from_js::<Capture>(&info) {
            handler(capture);
        }
    })
    .into_js_value();
    Listener::add("tabCapture", "onStatusChanged", &callback)
        .ok_or_else(|| OxichromeError::InvalidArgument("`chrome.tabCapture` isn't available; add `TabCapture` to `permissions` in #[oxichrome::extension]".into()))
}
