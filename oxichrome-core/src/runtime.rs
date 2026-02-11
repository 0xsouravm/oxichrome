use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::Result;
use crate::js_bridge;

pub fn get_url(path: &str) -> String {
    js_bridge::chrome_runtime_get_url(path)
}

pub async fn send_message<T: Serialize>(message: &T) -> Result<JsValue> {
    let js_msg = serde_wasm_bindgen::to_value(message)?;
    let promise = js_bridge::chrome_runtime_send_message(&js_msg);
    let result = JsFuture::from(promise).await?;
    Ok(result)
}
