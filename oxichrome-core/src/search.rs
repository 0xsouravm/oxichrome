use serde_json::json;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    CurrentTab,
    NewTab,
    NewWindow,
    Tab(i32),
}

fn browser_error(error: JsValue, at: Where) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    if message == "oxichrome:no-permission" {
        return OxichromeError::InvalidArgument(
            "the search API isn't available; add `Search` to `permissions` in #[oxichrome::extension] (if it's optional, request it first)".to_string(),
        );
    }
    if message == "oxichrome:no-current-tab" {
        return OxichromeError::InvalidArgument("there's no open tab to search in; use `Where::NewTab` or `Where::NewWindow`".to_string());
    }
    let lower = message.to_ascii_lowercase();
    if let Where::Tab(id) = at {
        if lower.contains("no tab with id") || lower.contains("invalid tab id") {
            return OxichromeError::InvalidArgument(format!("there's no tab with id {id}"));
        }
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

pub async fn query(text: &str, at: Where) -> Result<()> {
    if text.trim().is_empty() {
        return Err(OxichromeError::InvalidArgument("the search text is empty; give the words to search for".to_string()));
    }
    let mut details = json!({ "text": text });
    match at {
        Where::CurrentTab => details["disposition"] = json!("CURRENT_TAB"),
        Where::NewTab => details["disposition"] = json!("NEW_TAB"),
        Where::NewWindow => details["disposition"] = json!("NEW_WINDOW"),
        Where::Tab(id) if id < 0 => return Err(OxichromeError::InvalidArgument(format!("there's no tab with id {id}"))),
        Where::Tab(id) => details["tabId"] = json!(id),
    }
    let promise = js_bridge::search_query(&details.to_string()).map_err(|error| browser_error(error, at))?;
    JsFuture::from(promise).await.map_err(|error| browser_error(error, at))?;
    Ok(())
}
