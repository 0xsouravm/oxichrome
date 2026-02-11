use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::Result;
use crate::js_bridge;

pub async fn query<Q: Serialize, T: DeserializeOwned>(query_info: &Q) -> Result<Vec<T>> {
    let js_query = serde_wasm_bindgen::to_value(query_info)?;
    let promise = js_bridge::chrome_tabs_query(&js_query);
    let result = JsFuture::from(promise).await?;
    let tabs: Vec<T> = serde_wasm_bindgen::from_value(result)?;
    Ok(tabs)
}

pub async fn create<P: Serialize, T: DeserializeOwned>(props: &P) -> Result<T> {
    let js_props = serde_wasm_bindgen::to_value(props)?;
    let promise = js_bridge::chrome_tabs_create(&js_props);
    let result = JsFuture::from(promise).await?;
    let tab: T = serde_wasm_bindgen::from_value(result)?;
    Ok(tab)
}

pub async fn send_message<M: Serialize>(tab_id: i32, message: &M) -> Result<JsValue> {
    let js_msg = serde_wasm_bindgen::to_value(message)?;
    let promise = js_bridge::chrome_tabs_send_message(tab_id, &js_msg);
    let result = JsFuture::from(promise).await?;
    Ok(result)
}
