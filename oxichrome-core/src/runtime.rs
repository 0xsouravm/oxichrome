use serde::Serialize;
use wasm_bindgen::prelude::*;
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

pub struct NativePort(js_bridge::NativeMessagingPort);

impl NativePort {
    pub fn post_message<T: Serialize>(&self, msg: &T) -> Result<()> {
        let js_msg = serde_wasm_bindgen::to_value(msg)?;
        self.0.post_message(&js_msg);
        Ok(())
    }

    pub fn disconnect(self) {
        self.0.disconnect();
    }

    pub fn on_message(&self, callback: Closure<dyn FnMut(JsValue)>) {
        let on_message = js_sys::Reflect::get(&self.0, &JsValue::from_str("onMessage"))
            .unwrap_or(JsValue::UNDEFINED);
        let add_listener =
            js_sys::Reflect::get(&on_message, &JsValue::from_str("addListener"))
                .unwrap_or(JsValue::UNDEFINED);
        if let Ok(f) = add_listener.dyn_into::<js_sys::Function>() {
            let _ = f.call1(&on_message, &callback.into_js_value());
        }
    }

    pub fn on_disconnect(&self, callback: Closure<dyn FnMut()>) {
        let on_disconnect = js_sys::Reflect::get(&self.0, &JsValue::from_str("onDisconnect"))
            .unwrap_or(JsValue::UNDEFINED);
        let add_listener =
            js_sys::Reflect::get(&on_disconnect, &JsValue::from_str("addListener"))
                .unwrap_or(JsValue::UNDEFINED);
        if let Ok(f) = add_listener.dyn_into::<js_sys::Function>() {
            let _ = f.call1(&on_disconnect, &callback.into_js_value());
        }
    }
}

pub fn connect_native(app_name: &str) -> NativePort {
    NativePort(js_bridge::chrome_runtime_connect_native(app_name))
}

pub async fn send_native_message<T: Serialize>(app_name: &str, msg: &T) -> Result<JsValue> {
    let js_msg = serde_wasm_bindgen::to_value(msg)?;
    let promise = js_bridge::chrome_runtime_send_native_message(app_name, &js_msg);
    let result = JsFuture::from(promise).await?;
    Ok(result)
}
