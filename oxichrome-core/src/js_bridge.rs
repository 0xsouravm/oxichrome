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

#[wasm_bindgen(inline_js = r#"
function api() {
    if (!globalThis.chrome?.scripting) {
        throw new Error('[oxichrome] chrome.scripting is missing: add `Scripting` to `permissions` in #[oxichrome::extension], or, when it is in `optional_permissions`, ask for it with `permissions::request_on_click` first');
    }
    return chrome.scripting;
}

async function inject(url, name, arg) {
    const fresh = '__oxichrome_fresh_wasm';
    const pending = (globalThis.__oxichrome_script_panics ||= new Set());
    let settle;
    const panicked = new Promise((resolve) => { settle = resolve; });
    pending.add(settle);
    const restart = () => { globalThis[fresh] = (globalThis[fresh] || 0) + 1; };
    try {
        const module = await import(globalThis[fresh] ? `${url}?${globalThis[fresh]}` : url);
        await module.default();
        const outcome = await Promise.race([
            module[name](arg).then((ok) => ({ ok })),
            panicked.then((message) => ({ panic: message })),
        ]);
        if (outcome.panic !== undefined) {
            restart();
            return { err: outcome.panic };
        }
        return outcome;
    } catch (error) {
        if (error instanceof WebAssembly.RuntimeError) {
            restart();
            return { err: `it panicked (${error.message})` };
        }
        return { err: String((error && error.message) || error) };
    } finally {
        pending.delete(settle);
    }
}

export async function run_script(target, module, name, arg) {
    const results = await api().executeScript({
        target,
        func: inject,
        args: [chrome.runtime.getURL(module), name, arg],
    });
    return (results || []).filter((frame) => frame).map((frame) => ({
        frameId: frame.frameId,
        ok: frame.result && typeof frame.result.ok === 'string' ? frame.result.ok : undefined,
        err: frame.result && frame.result.err !== undefined
            ? frame.result.err
            : frame.error !== undefined
                ? String((frame.error && frame.error.message) || frame.error)
                : (frame.result ? undefined : 'the script did not run in this frame'),
    }));
}

export function report_script_panic(message) {
    for (const settle of globalThis.__oxichrome_script_panics || []) {
        settle(message);
    }
}

export async function insert_css(target, css) {
    await api().removeCSS({ target, css });
    await api().insertCSS({ target, css });
}

export async function remove_css(target, css) {
    await api().removeCSS({ target, css });
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn run_script(target: &JsValue, module: &str, name: &str, arg: &str) -> Result<js_sys::Promise, JsValue>;

    pub fn report_script_panic(message: &str);

    #[wasm_bindgen(catch)]
    pub fn insert_css(target: &JsValue, css: &str) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn remove_css(target: &JsValue, css: &str) -> Result<js_sys::Promise, JsValue>;
}

#[wasm_bindgen(inline_js = r#"
function api() {
    return globalThis.chrome.permissions;
}

export function permissions_request(set) {
    return api().request(set);
}

export function permissions_contains(set) {
    return api().contains(set);
}

export function permissions_remove(set) {
    return api().remove(set);
}

export function permissions_get_all() {
    return api().getAll();
}

export function permissions_declared() {
    const manifest = globalThis.chrome.runtime.getManifest();
    return {
        permissions: manifest.permissions || [],
        optional: manifest.optional_permissions || [],
        hosts: manifest.host_permissions || [],
        optional_hosts: manifest.optional_host_permissions || [],
    };
}

export function on_permissions_added(callback) {
    api().onAdded.addListener(callback);
}

export function on_permissions_removed(callback) {
    api().onRemoved.addListener(callback);
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn permissions_request(set: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn permissions_contains(set: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn permissions_remove(set: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn permissions_get_all() -> Result<js_sys::Promise, JsValue>;

    pub fn permissions_declared() -> JsValue;

    #[wasm_bindgen(js_name = on_permissions_added)]
    pub fn chrome_permissions_on_added_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_permissions_removed)]
    pub fn chrome_permissions_on_removed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen(inline_js = r#"
function api() {
    return globalThis.browser?.i18n ?? globalThis.chrome.i18n;
}

export function i18n_message(key, substitutions) {
    const text = api().getMessage(key, substitutions);
    return typeof text === 'string' ? text : '';
}

export function i18n_ui_language() {
    return api().getUILanguage();
}

export function i18n_accept_languages() {
    return Promise.resolve(api().getAcceptLanguages());
}

export function i18n_detect_language(text) {
    return Promise.resolve(api().detectLanguage(text));
}
"#)]
extern "C" {
    pub fn i18n_message(key: &str, substitutions: Vec<String>) -> String;

    pub fn i18n_ui_language() -> String;

    #[wasm_bindgen(catch)]
    pub fn i18n_accept_languages() -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn i18n_detect_language(text: &str) -> Result<js_sys::Promise, JsValue>;
}

#[wasm_bindgen(inline_js = r#"
function api() {
    return globalThis.browser?.windows ?? globalThis.chrome.windows;
}

function isFirefox() {
    return typeof globalThis.browser?.runtime?.getBrowserInfo === 'function';
}

export function windows_get(id, populate) {
    return api().get(id, { populate });
}

export function windows_current(populate) {
    return api().getCurrent({ populate });
}

export function windows_last_focused(populate) {
    return api().getLastFocused({ populate });
}

export function windows_all(populate) {
    return api().getAll({ populate });
}

export function windows_create(data) {
    if (!isFirefox()) {
        delete data.titlePreface;
    }
    return api().create(data).then((window) => window ?? null);
}

export function windows_update(id, info) {
    if (!isFirefox()) {
        delete info.titlePreface;
    }
    return api().update(id, info);
}

export function windows_remove(id) {
    return api().remove(id);
}

export function on_windows_created(callback) {
    api().onCreated.addListener(callback);
}

export function on_windows_removed(callback) {
    api().onRemoved.addListener(callback);
}

export function on_windows_focus_changed(callback) {
    api().onFocusChanged.addListener((id) => callback(id === -1 ? null : id));
}

export function on_windows_bounds_changed(callback) {
    api().onBoundsChanged?.addListener(callback);
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn windows_get(id: i32, populate: bool) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn windows_current(populate: bool) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn windows_last_focused(populate: bool) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn windows_all(populate: bool) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn windows_create(data: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn windows_update(id: i32, info: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn windows_remove(id: i32) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_name = on_windows_created)]
    pub fn chrome_windows_on_created_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_windows_removed)]
    pub fn chrome_windows_on_removed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_windows_focus_changed)]
    pub fn chrome_windows_on_focus_changed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_windows_bounds_changed)]
    pub fn chrome_windows_on_bounds_changed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen(inline_js = r#"
function chromePanel() {
    return globalThis.chrome?.sidePanel;
}

function firefoxSidebar() {
    return chromePanel() ? undefined : globalThis.browser?.sidebarAction;
}

export function side_panel_ready() {
    const manifest = globalThis.chrome?.runtime?.getManifest?.();
    if (!manifest) {
        return 'context';
    }
    if (!manifest.side_panel && !manifest.sidebar_action) {
        return 'missing';
    }
    if (!chromePanel() && !firefoxSidebar()) {
        return 'context';
    }
    return '';
}

export function side_panel_open() {
    const sidebar = firefoxSidebar();
    if (sidebar) {
        return sidebar.open();
    }
    return chromePanel().open({ windowId: chrome.windows.WINDOW_ID_CURRENT });
}

export function side_panel_close() {
    const sidebar = firefoxSidebar();
    if (sidebar) {
        return sidebar.close();
    }
    return chrome.windows.getCurrent().then((window) => chromePanel().close({ windowId: window.id }));
}

export function on_side_panel_opened(callback) {
    chromePanel()?.onOpened?.addListener(callback);
}

export function on_side_panel_closed(callback) {
    chromePanel()?.onClosed?.addListener(callback);
}
"#)]
extern "C" {
    pub fn side_panel_ready() -> String;

    #[wasm_bindgen(catch)]
    pub fn side_panel_open() -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn side_panel_close() -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_name = on_side_panel_opened)]
    pub fn chrome_side_panel_on_opened_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_side_panel_closed)]
    pub fn chrome_side_panel_on_closed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}
