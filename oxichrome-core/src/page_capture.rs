//! Saving a tab as one MHTML file, page and resources together. Chrome only, with
//! `PageCapture` in `permissions`.

use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::call::{self, Only};
use crate::error::{OxichromeError, Result};

/// The tab's page as MHTML (`multipart/related`), ready to download or store.
pub async fn save_as_mhtml(tab_id: i32) -> Result<Vec<u8>> {
    call::only(Only::Chrome, "pageCapture", "Firefox can save a page as a PDF with `tabs.saveAsPDF` from a click instead")?;
    let blob = call::call("pageCapture", "saveAsMHTML", &[crate::json::to_js(&serde_json::json!({ "tabId": tab_id }))?], "PageCapture").await?;
    if blob.is_undefined() || blob.is_null() {
        return Err(OxichromeError::UnexpectedValue(format!("Chrome couldn't save tab {tab_id}; it may be a browser page, which can't be captured")));
    }
    let array_buffer = js_sys::Reflect::get(&blob, &JsValue::from_str("arrayBuffer"))?;
    let buffer = js_sys::Function::from(array_buffer).call0(&blob)?;
    let buffer = JsFuture::from(js_sys::Promise::from(buffer)).await?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}
