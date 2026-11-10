//! Raw `#[wasm_bindgen]` extern declarations for Chrome Extension APIs.

use wasm_bindgen::prelude::*;

// chrome.runtime

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["chrome", "runtime"], js_name = getURL)]
    pub fn chrome_runtime_get_url(path: &str) -> String;

    #[wasm_bindgen(js_namespace = ["chrome", "runtime"], js_name = sendMessage)]
    pub fn chrome_runtime_send_message(message: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "runtime"], js_name = sendMessage)]
    pub fn chrome_runtime_send_message_checked(
        message: &JsValue,
    ) -> Result<js_sys::Promise, JsValue>;

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
    #[wasm_bindgen(catch, js_namespace = ["chrome", "storage", "local"], js_name = get)]
    pub fn chrome_storage_local_get(keys: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "storage", "local"], js_name = set)]
    pub fn chrome_storage_local_set(items: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "storage", "local"], js_name = remove)]
    pub fn chrome_storage_local_remove(keys: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "storage", "sync"], js_name = get)]
    pub fn chrome_storage_sync_get(keys: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "storage", "sync"], js_name = set)]
    pub fn chrome_storage_sync_set(items: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "storage", "sync"], js_name = remove)]
    pub fn chrome_storage_sync_remove(keys: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "storage", "session"], js_name = get)]
    pub fn chrome_storage_session_get(keys: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "storage", "session"], js_name = set)]
    pub fn chrome_storage_session_set(items: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "storage", "session"], js_name = remove)]
    pub fn chrome_storage_session_remove(keys: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_namespace = ["chrome", "storage", "onChanged"], js_name = addListener)]
    pub fn chrome_storage_on_changed_add_listener(callback: &Closure<dyn FnMut(JsValue, JsValue)>);
}

// chrome.tabs

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["chrome", "tabs"], js_name = query)]
    pub fn chrome_tabs_query(query_info: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "tabs"], js_name = create)]
    pub fn chrome_tabs_create(create_properties: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "tabs"], js_name = get)]
    pub fn chrome_tabs_get(tab_id: i32) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "tabs"], js_name = update)]
    pub fn chrome_tabs_update(
        tab_id: i32,
        update_properties: &JsValue,
    ) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "tabs"], js_name = remove)]
    pub fn chrome_tabs_remove(tab_id: i32) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "tabs"], js_name = sendMessage)]
    pub fn chrome_tabs_send_message(
        tab_id: i32,
        message: &JsValue,
    ) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_namespace = ["chrome", "tabs", "onUpdated"], js_name = addListener)]
    pub fn chrome_tabs_on_updated_add_listener(
        callback: &Closure<dyn FnMut(JsValue, JsValue, JsValue)>,
    );

    #[wasm_bindgen(js_namespace = ["chrome", "tabs", "onRemoved"], js_name = addListener)]
    pub fn chrome_tabs_on_removed_add_listener(callback: &Closure<dyn FnMut(JsValue, JsValue)>);

    #[wasm_bindgen(js_namespace = ["chrome", "tabs", "onActivated"], js_name = addListener)]
    pub fn chrome_tabs_on_activated_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = setBadgeText)]
    pub fn chrome_action_set_badge_text(details: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = getBadgeText)]
    pub fn chrome_action_get_badge_text(details: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = setBadgeBackgroundColor)]
    pub fn chrome_action_set_badge_background_color(
        details: &JsValue,
    ) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = setBadgeTextColor)]
    pub fn chrome_action_set_badge_text_color(
        details: &JsValue,
    ) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = setTitle)]
    pub fn chrome_action_set_title(details: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = getTitle)]
    pub fn chrome_action_get_title(details: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = setIcon)]
    pub fn chrome_action_set_icon(details: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = setPopup)]
    pub fn chrome_action_set_popup(details: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = getPopup)]
    pub fn chrome_action_get_popup(details: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = enable)]
    pub fn chrome_action_enable(tab_id: Option<i32>) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "action"], js_name = disable)]
    pub fn chrome_action_disable(tab_id: Option<i32>) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_namespace = ["chrome", "action", "onClicked"], js_name = addListener)]
    pub fn chrome_action_on_clicked_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["chrome", "alarms"], js_name = create)]
    pub fn chrome_alarms_create(name: &str, alarm_info: &JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "alarms"], js_name = get)]
    pub fn chrome_alarms_get(name: &str) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "alarms"], js_name = getAll)]
    pub fn chrome_alarms_get_all() -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "alarms"], js_name = clear)]
    pub fn chrome_alarms_clear(name: &str) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "alarms"], js_name = clearAll)]
    pub fn chrome_alarms_clear_all() -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_namespace = ["chrome", "alarms", "onAlarm"], js_name = addListener)]
    pub fn chrome_alarms_on_alarm_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["chrome", "notifications"], js_name = create)]
    pub fn chrome_notifications_create(
        id: &str,
        options: &JsValue,
    ) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "notifications"], js_name = update)]
    pub fn chrome_notifications_update(
        id: &str,
        options: &JsValue,
    ) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "notifications"], js_name = clear)]
    pub fn chrome_notifications_clear(id: &str) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["chrome", "notifications"], js_name = getAll)]
    pub fn chrome_notifications_get_all() -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_namespace = ["chrome", "notifications", "onClicked"], js_name = addListener)]
    pub fn chrome_notifications_on_clicked_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_namespace = ["chrome", "notifications", "onClosed"], js_name = addListener)]
    pub fn chrome_notifications_on_closed_add_listener(
        callback: &Closure<dyn FnMut(JsValue, JsValue)>,
    );

    #[wasm_bindgen(js_namespace = ["chrome", "notifications", "onButtonClicked"], js_name = addListener)]
    pub fn chrome_notifications_on_button_clicked_add_listener(
        callback: &Closure<dyn FnMut(JsValue, JsValue)>,
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

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["chrome", "contextMenus"], js_name = update)]
    pub fn chrome_context_menus_update(
        id: &str,
        properties: &JsValue,
    ) -> Result<js_sys::Promise, JsValue>;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["chrome", "commands"], js_name = getAll)]
    pub fn chrome_commands_get_all() -> Result<js_sys::Promise, JsValue>;
}

#[wasm_bindgen(inline_js = "export function open_shortcut_settings() { const api = globalThis.browser?.commands; if (typeof api?.openShortcutSettings === 'function') { return api.openShortcutSettings(); } return chrome.tabs.create({ url: 'chrome://extensions/shortcuts' }).then(() => undefined); }")]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn open_shortcut_settings() -> Result<js_sys::Promise, JsValue>;
}
