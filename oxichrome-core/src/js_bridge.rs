//! Raw `#[wasm_bindgen]` extern declarations for Chrome Extension APIs.

use wasm_bindgen::prelude::*;

// chrome.runtime

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["chrome", "runtime"], js_name = getURL)]
    pub fn chrome_runtime_get_url(path: &str) -> String;

    #[wasm_bindgen(js_namespace = ["chrome", "runtime"], js_name = sendMessage)]
    pub fn chrome_runtime_send_message(message: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "runtime", "onInstalled"], js_name = addListener)]
    pub fn chrome_runtime_on_installed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_namespace = ["chrome", "runtime", "onMessage"], js_name = addListener)]
    pub fn chrome_runtime_on_message_add_listener(
        callback: &Closure<dyn FnMut(JsValue, JsValue, JsValue)>,
    );
}

// chrome.storage.local

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["chrome", "storage", "local"], js_name = get)]
    pub fn chrome_storage_local_get(keys: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "storage", "local"], js_name = set)]
    pub fn chrome_storage_local_set(items: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "storage", "local"], js_name = remove)]
    pub fn chrome_storage_local_remove(keys: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "storage", "onChanged"], js_name = addListener)]
    pub fn chrome_storage_on_changed_add_listener(
        callback: &Closure<dyn FnMut(JsValue, JsValue)>,
    );
}

// chrome.tabs

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["chrome", "tabs"], js_name = query)]
    pub fn chrome_tabs_query(query_info: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "tabs"], js_name = create)]
    pub fn chrome_tabs_create(create_properties: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "tabs"], js_name = sendMessage)]
    pub fn chrome_tabs_send_message(tab_id: i32, message: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "tabs", "onUpdated"], js_name = addListener)]
    pub fn chrome_tabs_on_updated_add_listener(
        callback: &Closure<dyn FnMut(JsValue, JsValue, JsValue)>,
    );

    #[wasm_bindgen(js_namespace = ["chrome", "tabs", "onActivated"], js_name = addListener)]
    pub fn chrome_tabs_on_activated_add_listener(
        callback: &Closure<dyn FnMut(JsValue)>,
    );
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::__log_impl(&format!($($arg)*));
    };
}

#[doc(hidden)]
pub fn __log_impl(msg: &str) {
    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(msg));
}
