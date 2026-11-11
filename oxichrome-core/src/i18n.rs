use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedLanguages {
    pub is_reliable: bool,
    pub languages: Vec<DetectedLanguage>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DetectedLanguage {
    pub language: String,
    pub percentage: u32,
}

fn browser_error(error: JsValue) -> OxichromeError {
    OxichromeError::Js(JsError::new(error))
}

#[doc(hidden)]
pub fn __message(key: &str, substitutions: &[String]) -> String {
    js_bridge::i18n_message(key, substitutions.to_vec())
}

pub fn ui_language() -> String {
    js_bridge::i18n_ui_language()
}

pub async fn accept_languages() -> Result<Vec<String>> {
    let value = JsFuture::from(js_bridge::i18n_accept_languages().map_err(browser_error)?)
        .await
        .map_err(browser_error)?;
    crate::json::from_js(&value)
}

pub async fn detect_language(text: &str) -> Result<DetectedLanguages> {
    let value = JsFuture::from(js_bridge::i18n_detect_language(text).map_err(browser_error)?)
        .await
        .map_err(browser_error)?;
    crate::json::from_js(&value)
}
