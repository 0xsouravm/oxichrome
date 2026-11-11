use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;
use std::result::Result as StdResult;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Panel {
    pub window_id: i32,
    pub tab_id: Option<i32>,
}

impl FromEventArg for Panel {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        crate::json::from_js(&value)
    }
}

impl EventArg<events::SidePanelOnOpened, 1> for Panel {}
impl EventArg<events::SidePanelOnClosed, 1> for Panel {}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    let lower = message.to_ascii_lowercase();
    if lower.contains("user gesture") || lower.contains("user input handler") {
        return OxichromeError::InvalidArgument(format!(
            "{message} (use the handler `side_panel::open_on_click` or `close_on_click` returns, so the browser sees the click)"
        ));
    }
    if lower.contains("no active side panel") {
        return OxichromeError::InvalidArgument(format!("{message} (the side panel isn't open in this window)"));
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

fn ready() -> Result<()> {
    match js_bridge::side_panel_ready().as_str() {
        "" => Ok(()),
        "missing" => Err(OxichromeError::InvalidArgument(
            "this extension has no side panel; add a `#[oxichrome::side_panel]` component".to_string(),
        )),
        _ => Err(OxichromeError::InvalidArgument(
            "the side panel can only be opened or closed from extension pages (the popup, the options page, the side panel itself, or a tab showing an extension page)".to_string(),
        )),
    }
}

fn on_click<E, F, Fut>(what: &'static str, call: fn() -> StdResult<js_sys::Promise, JsValue>, then: F) -> impl FnMut(E) + 'static
where
    E: AsRef<web_sys::Event>,
    F: FnMut(Result<()>) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    let then = Rc::new(RefCell::new(then));
    move |event: E| {
        let started = if !event.as_ref().is_trusted() {
            Err(OxichromeError::InvalidArgument(format!(
                "only a real click or key press can {what} the side panel; this event was created by code"
            )))
        } else {
            ready().and_then(|()| call().map_err(browser_error))
        };
        let then = then.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = match started {
                Ok(promise) => JsFuture::from(promise).await.map(|_| ()).map_err(browser_error),
                Err(err) => Err(err),
            };
            let future = (then.borrow_mut())(result);
            future.await;
        });
    }
}

pub fn open_on_click<E, F, Fut>(then: F) -> impl FnMut(E) + 'static
where
    E: AsRef<web_sys::Event>,
    F: FnMut(Result<()>) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    on_click("open", js_bridge::side_panel_open, then)
}

pub fn close_on_click<E, F, Fut>(then: F) -> impl FnMut(E) + 'static
where
    E: AsRef<web_sys::Event>,
    F: FnMut(Result<()>) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    on_click("close", js_bridge::side_panel_close, then)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_chrome_events() {
        let panel: Panel = serde_json::from_str(r#"{"path":"/sidepanel.html","windowId":12}"#).unwrap();
        assert_eq!((panel.window_id, panel.tab_id), (12, None));
        let panel: Panel = serde_json::from_str(r#"{"path":"/sidepanel.html","windowId":12,"tabId":4}"#).unwrap();
        assert_eq!(panel.tab_id, Some(4));
    }
}
