use js_sys::Promise;
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::js_bridge;

async fn read<T: DeserializeOwned>(promise: Promise, key: &str) -> Result<Option<T>> {
    let result = JsFuture::from(promise).await?;

    let val =
        js_sys::Reflect::get(&result, &JsValue::from_str(key)).map_err(OxichromeError::from)?;

    if val.is_undefined() || val.is_null() {
        return Ok(None);
    }

    Ok(Some(crate::json::from_js(&val)?))
}

fn items<T: Serialize>(key: &str, value: &T) -> Result<JsValue> {
    let obj = js_sys::Object::new();
    let js_val = crate::json::to_js(value)?;
    js_sys::Reflect::set(&obj, &JsValue::from_str(key), &js_val).map_err(OxichromeError::from)?;
    Ok(obj.into())
}

async fn wait(promise: Promise) -> Result<()> {
    JsFuture::from(promise).await?;
    Ok(())
}

pub async fn get<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
    read(
        js_bridge::chrome_storage_local_get(&JsValue::from_str(key))?,
        key,
    )
    .await
}

pub async fn set<T: Serialize>(key: &str, value: &T) -> Result<()> {
    wait(js_bridge::chrome_storage_local_set(&items(key, value)?)?).await
}

pub async fn remove(key: &str) -> Result<()> {
    wait(js_bridge::chrome_storage_local_remove(&JsValue::from_str(
        key,
    ))?)
    .await
}

pub mod sync {
    use super::*;

    pub async fn get<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
        read(
            js_bridge::chrome_storage_sync_get(&JsValue::from_str(key))?,
            key,
        )
        .await
    }

    pub async fn set<T: Serialize>(key: &str, value: &T) -> Result<()> {
        wait(js_bridge::chrome_storage_sync_set(&items(key, value)?)?).await
    }

    pub async fn remove(key: &str) -> Result<()> {
        wait(js_bridge::chrome_storage_sync_remove(&JsValue::from_str(
            key,
        ))?)
        .await
    }
}

pub mod session {
    use super::*;

    pub async fn get<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
        read(
            js_bridge::chrome_storage_session_get(&JsValue::from_str(key))?,
            key,
        )
        .await
    }

    pub async fn set<T: Serialize>(key: &str, value: &T) -> Result<()> {
        wait(js_bridge::chrome_storage_session_set(&items(key, value)?)?).await
    }

    pub async fn remove(key: &str) -> Result<()> {
        wait(js_bridge::chrome_storage_session_remove(
            &JsValue::from_str(key),
        )?)
        .await
    }
}
