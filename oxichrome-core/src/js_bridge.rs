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

#[wasm_bindgen(inline_js = r#"
function api() {
    return globalThis.chrome.omnibox;
}

export function on_omnibox_started(callback) {
    api()?.onInputStarted.addListener(callback);
}

export function on_omnibox_changed(callback) {
    api()?.onInputChanged.addListener(callback);
}

export function on_omnibox_entered(callback) {
    api()?.onInputEntered.addListener(callback);
}

export function on_omnibox_cancelled(callback) {
    api()?.onInputCancelled.addListener(callback);
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = on_omnibox_started)]
    pub fn chrome_omnibox_on_input_started_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_omnibox_changed)]
    pub fn chrome_omnibox_on_input_changed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_omnibox_entered)]
    pub fn chrome_omnibox_on_input_entered_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_omnibox_cancelled)]
    pub fn chrome_omnibox_on_input_cancelled_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen(inline_js = r#"
export function idle_call(method, seconds) {
    const idle = globalThis.chrome?.idle;
    if (!idle) {
        throw new Error('oxichrome:no-permission');
    }
    const result = Promise.resolve(idle[method](seconds));
    if (method !== 'setDetectionInterval') {
        return result;
    }
    return result.then(() => globalThis.chrome?.storage?.session?.set({ __oxichrome_idle_interval: seconds }).catch(() => {}));
}

export function on_idle_state_changed(callback) {
    globalThis.chrome?.idle?.onStateChanged.addListener(callback);
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn idle_call(method: &str, seconds: i32) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_name = on_idle_state_changed)]
    pub fn chrome_idle_on_state_changed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen(inline_js = r#"
export function web_navigation_call(method, arg) {
    const webNavigation = globalThis.chrome?.webNavigation;
    if (!webNavigation) {
        throw new Error('oxichrome:no-permission');
    }
    return Promise.resolve(webNavigation[method](arg));
}

export function on_web_navigation_before_navigate(callback) {
    globalThis.chrome?.webNavigation?.onBeforeNavigate.addListener(callback);
}

export function on_web_navigation_committed(callback) {
    globalThis.chrome?.webNavigation?.onCommitted.addListener(callback);
}

export function on_web_navigation_dom_content_loaded(callback) {
    globalThis.chrome?.webNavigation?.onDOMContentLoaded.addListener(callback);
}

export function on_web_navigation_completed(callback) {
    globalThis.chrome?.webNavigation?.onCompleted.addListener(callback);
}

export function on_web_navigation_error_occurred(callback) {
    globalThis.chrome?.webNavigation?.onErrorOccurred.addListener(callback);
}

export function on_web_navigation_history_state_updated(callback) {
    globalThis.chrome?.webNavigation?.onHistoryStateUpdated.addListener(callback);
}

export function on_web_navigation_reference_fragment_updated(callback) {
    globalThis.chrome?.webNavigation?.onReferenceFragmentUpdated.addListener(callback);
}

export function on_web_navigation_created_navigation_target(callback) {
    globalThis.chrome?.webNavigation?.onCreatedNavigationTarget.addListener(callback);
}

export function on_web_navigation_tab_replaced(callback) {
    globalThis.chrome?.webNavigation?.onTabReplaced.addListener(callback);
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn web_navigation_call(method: &str, arg: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_name = on_web_navigation_before_navigate)]
    pub fn chrome_web_navigation_on_before_navigate_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_web_navigation_committed)]
    pub fn chrome_web_navigation_on_committed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_web_navigation_dom_content_loaded)]
    pub fn chrome_web_navigation_on_dom_content_loaded_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_web_navigation_completed)]
    pub fn chrome_web_navigation_on_completed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_web_navigation_error_occurred)]
    pub fn chrome_web_navigation_on_error_occurred_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_web_navigation_history_state_updated)]
    pub fn chrome_web_navigation_on_history_state_updated_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_web_navigation_reference_fragment_updated)]
    pub fn chrome_web_navigation_on_reference_fragment_updated_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_web_navigation_created_navigation_target)]
    pub fn chrome_web_navigation_on_created_navigation_target_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_web_navigation_tab_replaced)]
    pub fn chrome_web_navigation_on_tab_replaced_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen(inline_js = r#"
export function cookies_call(method, arg) {
    const cookies = globalThis.chrome?.cookies;
    if (!cookies) {
        throw new Error('oxichrome:no-permission');
    }
    return Promise.resolve(arg === undefined ? cookies[method]() : cookies[method](arg));
}

export function cookies_reachable(url) {
    const parsed = new URL(url);
    const contains = globalThis.chrome?.permissions?.contains;
    if (typeof contains !== 'function') {
        return Promise.resolve(true);
    }
    return Promise.resolve(chrome.permissions.contains({ origins: [`${parsed.protocol}//${parsed.hostname}/*`] }));
}

export function on_cookies_changed(callback) {
    globalThis.chrome?.cookies?.onChanged.addListener(callback);
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn cookies_call(method: &str, arg: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn cookies_reachable(url: &str) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_name = on_cookies_changed)]
    pub fn chrome_cookies_on_changed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen(inline_js = r#"
function api() {
    const downloads = globalThis.chrome?.downloads;
    if (!downloads) {
        throw new Error('oxichrome:no-permission');
    }
    return downloads;
}

function firefox() {
    return typeof globalThis.browser?.runtime?.getBrowserInfo === 'function';
}

function base64(bytes) {
    if (typeof bytes.toBase64 === 'function') {
        return bytes.toBase64();
    }
    let text = '';
    for (let i = 0; i < bytes.length; i += 0x8000) {
        text += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    }
    return btoa(text);
}

const blobs = new Map();
let watching = false;

function release(id) {
    const url = blobs.get(id);
    if (url) {
        blobs.delete(id);
        URL.revokeObjectURL(url);
    }
}

function settle(id) {
    api().search({ id }).then(([item]) => {
        if (!item || item.state === 'complete' || (item.state === 'interrupted' && !item.paused)) {
            release(id);
        }
    }, () => release(id));
}

function watch() {
    if (watching) {
        return;
    }
    watching = true;
    api().onChanged.addListener((delta) => {
        if (blobs.has(delta.id) && delta.state && delta.state.current !== 'in_progress') {
            settle(delta.id);
        }
    });
    api().onErased.addListener(release);
}

function saveHere(save) {
    const url = URL.createObjectURL(new Blob([save.bytes], { type: save.mime }));
    watch();
    return api()
        .download({ url, filename: save.filename, conflictAction: save.conflictAction, saveAs: save.saveAs })
        .then((id) => {
            blobs.set(id, url);
            settle(id);
            return id;
        }, (error) => {
            URL.revokeObjectURL(url);
            throw error;
        });
}

function isBackground() {
    try {
        return globalThis.browser?.extension?.getBackgroundPage?.() === globalThis;
    } catch (error) {
        return false;
    }
}

if (firefox() && isBackground()) {
    browser.runtime.onMessage.addListener((message) => {
        const save = message && typeof message === 'object' ? message.__oxichrome_download_save : undefined;
        if (!save || typeof save !== 'object') {
            return undefined;
        }
        return saveHere(save).then((id) => ({ id }), (error) => ({ error: String(error?.message ?? error) }));
    });
}

export function downloads_call(method, args) {
    return Promise.resolve(api()[method](...args));
}

export function downloads_save(filename, bytes, mime, conflictAction, saveAs) {
    const save = { filename, bytes, mime, conflictAction: conflictAction ?? undefined, saveAs };
    if (!firefox()) {
        return api().download({ url: 'data:' + mime + ';base64,' + base64(bytes), filename, conflictAction: save.conflictAction, saveAs });
    }
    api();
    if (isBackground() || !chrome.runtime.getManifest().background) {
        return saveHere(save);
    }
    return browser.runtime.sendMessage({ __oxichrome_download_save: save }).then((reply) => {
        if (reply && typeof reply.id === 'number') {
            return reply.id;
        }
        throw new Error(reply?.error ?? 'the background page did not answer');
    }, (error) => {
        if (String(error?.message ?? error).includes('Receiving end does not exist')) {
            return saveHere(save);
        }
        throw error;
    });
}

export function downloads_open(id) {
    const downloads = api();
    if (typeof downloads.open !== 'function') {
        throw new Error('oxichrome:no-open-permission');
    }
    return Promise.resolve(downloads.open(id));
}

export function on_downloads_created(callback) {
    globalThis.chrome?.downloads?.onCreated.addListener(callback);
}

export function on_downloads_changed(callback) {
    globalThis.chrome?.downloads?.onChanged.addListener(callback);
}

export function on_downloads_erased(callback) {
    globalThis.chrome?.downloads?.onErased.addListener(callback);
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn downloads_call(method: &str, args: &js_sys::Array) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn downloads_save(filename: &str, bytes: Vec<u8>, mime: &str, conflict_action: Option<&str>, save_as: bool) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn downloads_open(id: i32) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_name = on_downloads_created)]
    pub fn chrome_downloads_on_created_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_downloads_changed)]
    pub fn chrome_downloads_on_changed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_downloads_erased)]
    pub fn chrome_downloads_on_erased_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen(inline_js = r#"
function dnr() {
    const api = globalThis.chrome?.declarativeNetRequest;
    if (!api) {
        throw new Error('oxichrome:no-permission');
    }
    return api;
}

export function dnr_call(method, arg) {
    const api = dnr();
    if (typeof api[method] !== 'function') {
        throw new Error('oxichrome:chrome-only');
    }
    return Promise.resolve(arg === undefined ? api[method]() : api[method](arg));
}

export function dnr_manifest() {
    return globalThis.chrome.runtime.getManifest();
}

export function dnr_has_host_access() {
    return globalThis.chrome.permissions.getAll().then((all) => (all.origins || []).length > 0);
}

export function dnr_read_file(path) {
    return fetch(globalThis.chrome.runtime.getURL(path)).then((response) => {
        if (!response.ok) {
            throw new Error(`can't read ${path}`);
        }
        return response.text();
    });
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn dnr_call(method: &str, arg: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn dnr_manifest() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn dnr_has_host_access() -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn dnr_read_file(path: &str) -> Result<js_sys::Promise, JsValue>;
}

#[wasm_bindgen(inline_js = r#"
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function extensionPage() {
    const protocol = globalThis.location?.protocol;
    if (protocol !== 'chrome-extension:' && protocol !== 'moz-extension:') {
        throw new Error('oxichrome:content-script');
    }
    return protocol === 'moz-extension:';
}

let config = null;
let creating = null;

function readConfig() {
    if (!config) {
        config = fetch(globalThis.chrome.runtime.getURL('offscreen.json')).then(
            (response) => {
                if (!response.ok) {
                    throw new Error('oxichrome:no-offscreen');
                }
                return response.json();
            },
            () => {
                throw new Error('oxichrome:no-offscreen');
            },
        );
        config.catch(() => { config = null; });
    }
    return config;
}

async function openChrome() {
    const offscreen = globalThis.chrome.offscreen;
    if (!offscreen) {
        throw new Error('oxichrome:no-offscreen');
    }
    if (await offscreen.hasDocument()) {
        return;
    }
    if (!creating) {
        creating = (async () => {
            const { reasons, justification } = await readConfig();
            try {
                await offscreen.createDocument({ url: 'offscreen.html', reasons, justification });
            } catch (error) {
                if (!String(error && error.message).includes('single offscreen document')) {
                    throw error;
                }
            }
        })().finally(() => { creating = null; });
    }
    await creating;
}

async function host(op) {
    await readConfig();
    if (globalThis.__oxichrome_offscreen_host) {
        return globalThis.__oxichrome_offscreen_host(op);
    }
    const reply = await globalThis.chrome.runtime.sendMessage({ __oxichrome_offscreen_host: op });
    if (!reply || reply.err !== undefined) {
        throw new Error(reply ? reply.err : 'oxichrome:no-offscreen');
    }
    return reply.ok;
}

async function open(firefox) {
    if (firefox) {
        await host('open');
    } else {
        await openChrome();
    }
}

function inside() {
    if (globalThis.__oxichrome_offscreen_local) {
        throw new Error('oxichrome:inside-offscreen');
    }
}

async function request(message) {
    try {
        return { reply: await globalThis.chrome.runtime.sendMessage(message) };
    } catch (error) {
        return { error: String(error && error.message) };
    }
}

export async function offscreen_run(name, exportName, arg) {
    const firefox = extensionPage();
    if (globalThis.__oxichrome_offscreen_local) {
        return globalThis.__oxichrome_offscreen_local(exportName, arg);
    }
    const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    await open(firefox);
    for (let attempt = 0; ; attempt++) {
        const started = await request({ __oxichrome_offscreen: exportName, phase: 'start', id, arg });
        if (started.reply && started.reply.started === id) {
            break;
        }
        if (started.reply && started.reply.err !== undefined) {
            return started.reply;
        }
        if (started.error !== undefined && !started.error.includes('Receiving end does not exist')) {
            if (/message (channel|port) closed/.test(started.error)) {
                return { err: "didn't finish: the offscreen document closed while it was running" };
            }
            throw new Error(started.error);
        }
        if (attempt >= 100) {
            return { err: "didn't start: the offscreen document never answered" };
        }
        await sleep(50);
        await open(firefox);
    }
    const done = await request({ __oxichrome_offscreen: exportName, phase: 'result', id });
    if (done.reply && (done.reply.ok !== undefined || done.reply.err !== undefined)) {
        return done.reply;
    }
    return { err: "didn't finish: the offscreen document closed while it was running" };
}

export async function offscreen_close() {
    inside();
    if (extensionPage()) {
        await host('close');
        return;
    }
    const offscreen = globalThis.chrome.offscreen;
    if (!offscreen) {
        throw new Error('oxichrome:no-offscreen');
    }
    if (creating) {
        await creating.catch(() => {});
    }
    if (await offscreen.hasDocument()) {
        try {
            await offscreen.closeDocument();
        } catch (error) {
            if (!String(error && error.message).includes('No current offscreen document')) {
                throw error;
            }
        }
    }
}

export async function offscreen_is_open() {
    inside();
    if (extensionPage()) {
        return host('is_open');
    }
    const offscreen = globalThis.chrome.offscreen;
    if (!offscreen) {
        throw new Error('oxichrome:no-offscreen');
    }
    return offscreen.hasDocument();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn offscreen_run(name: &str, export: &str, arg: &str) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn offscreen_close() -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn offscreen_is_open() -> Result<js_sys::Promise, JsValue>;
}

#[wasm_bindgen(inline_js = r#"
const FIREFOX_ROOT = 'root________';
const FIREFOX_TAGS = 'tags________';
let queue = Promise.resolve();

function api() {
    const bookmarks = globalThis.chrome?.bookmarks;
    if (!bookmarks) {
        throw new Error('oxichrome:no-permission');
    }
    return bookmarks;
}

function firefox() {
    return typeof globalThis.browser?.runtime?.getBrowserInfo === 'function';
}

function rootId() {
    return firefox() ? FIREFOX_ROOT : '0';
}

function fail(code, detail) {
    throw new Error(`oxichrome:${code}:${detail}`);
}

function isFolder(node) {
    return node.type === 'folder' || (node.type === undefined && node.url === undefined);
}

async function tagFolders() {
    if (!firefox()) {
        return new Set();
    }
    try {
        return new Set((await api().getChildren(FIREFOX_TAGS)).map((node) => node.id));
    } catch (error) {
        return new Set();
    }
}

async function raw(id) {
    if (id === rootId() || id === FIREFOX_TAGS) {
        fail('not-found', id);
    }
    let node;
    try {
        [node] = await api().get(id);
    } catch (error) {
        fail('not-found', id);
    }
    if (!node || (firefox() && (node.parentId === FIREFOX_TAGS || (await tagFolders()).has(node.parentId)))) {
        fail('not-found', id);
    }
    return node;
}

async function visible(nodes) {
    const tags = await tagFolders();
    return nodes.filter((node) => node.id !== rootId() && node.id !== FIREFOX_TAGS && node.parentId !== FIREFOX_TAGS && !tags.has(node.parentId));
}

async function folder(id) {
    if (id === rootId()) {
        fail('root', id);
    }
    const node = await raw(id);
    if (!isFolder(node)) {
        fail('not-folder', id);
    }
    return node;
}

function changeable(node) {
    if (node.parentId === rootId()) {
        fail('top-level', node.id);
    }
    if (node.unmodifiable === 'managed') {
        fail('managed', node.id);
    }
}

function writable(parent) {
    if (parent.unmodifiable === 'managed') {
        fail('managed', parent.id);
    }
}

async function count(id) {
    return (await api().getChildren(id)).length;
}

function serial(task) {
    const run = queue.then(task, task);
    queue = run.catch(() => {});
    return run;
}

async function top() {
    const [root] = await api().getTree();
    return (root.children || []).filter((child) => child.id !== FIREFOX_TAGS);
}

async function create(details) {
    const parent = await folder(details.parentId);
    writable(parent);
    const length = await count(parent.id);
    if (details.index !== null && details.index > length) {
        fail('index', `${details.index}:${length}`);
    }
    const create = { parentId: parent.id, title: details.title };
    if (details.url !== null) {
        create.url = details.url;
    }
    if (details.index !== null) {
        create.index = details.index;
    }
    return api().create(create);
}

async function update(id, change) {
    const node = await raw(id);
    changeable(node);
    if (node.type === 'separator') {
        fail('separator', id);
    }
    if (change.url !== null && isFolder(node)) {
        fail('folder-url', id);
    }
    const update = {};
    if (change.title !== null && change.title !== (node.title ?? '')) {
        update.title = change.title;
    }
    if (change.url !== null && change.url !== node.url) {
        update.url = change.url;
    }
    if (Object.keys(update).length === 0) {
        return node;
    }
    return api().update(id, update);
}

async function move(id, parentId, index) {
    const node = await raw(id);
    changeable(node);
    const parent = await folder(parentId);
    writable(parent);
    for (let step = parent; step; ) {
        if (step.id === node.id) {
            fail('into-self', id);
        }
        if (!step.parentId || step.parentId === rootId()) {
            break;
        }
        [step] = await api().get(step.parentId);
    }
    const length = await count(parent.id);
    const same = node.parentId === parent.id;
    const last = same ? length - 1 : length;
    if (index !== null && index > last) {
        fail('index', `${index}:${last}`);
    }
    const target = index === null ? last : index;
    if (same && target === node.index) {
        return node;
    }
    const destination = { parentId: parent.id, index: !firefox() && same && target > node.index ? target + 1 : target };
    return api().move(id, destination);
}

async function remove(id, tree) {
    const node = await raw(id);
    changeable(node);
    if (isFolder(node)) {
        if (!tree && (await count(id)) > 0) {
            fail('not-empty', id);
        }
        await (tree ? api().removeTree(id) : api().remove(id));
    } else {
        await api().remove(id);
    }
    return null;
}

async function search(query) {
    const ask = {};
    for (const key of ['query', 'url', 'title']) {
        if (query[key] !== null && query[key] !== '') {
            ask[key] = query[key];
        }
    }
    const found = await visible(await api().search(ask));
    return query.title === null ? found : found.filter((node) => (node.title ?? '') === query.title);
}

async function call(op, a) {
    switch (op) {
        case 'top':
            return top();
        case 'get':
            return raw(a.id);
        case 'children':
            await folder(a.id);
            return visible(await api().getChildren(a.id));
        case 'subtree':
            await raw(a.id);
            return (await api().getSubTree(a.id))[0];
        case 'recent':
            return visible(await api().getRecent(a.count));
        case 'search':
            return search(a);
        case 'create':
            return serial(() => create(a));
        case 'update':
            return serial(() => update(a.id, a.change));
        case 'move':
            return serial(() => move(a.id, a.parentId, a.index));
        case 'remove':
            return serial(() => remove(a.id, false));
        case 'remove_tree':
            return serial(() => remove(a.id, true));
        default:
            throw new Error(`unknown bookmarks call ${op}`);
    }
}

export function bookmarks_call(op, args) {
    api();
    return call(op, JSON.parse(args)).then((value) => JSON.stringify(value ?? null));
}

export function on_bookmarks_created(callback) {
    globalThis.chrome?.bookmarks?.onCreated.addListener(callback);
}

export function on_bookmarks_removed(callback) {
    globalThis.chrome?.bookmarks?.onRemoved.addListener(callback);
}

export function on_bookmarks_changed(callback) {
    globalThis.chrome?.bookmarks?.onChanged.addListener(callback);
}

export function on_bookmarks_moved(callback) {
    globalThis.chrome?.bookmarks?.onMoved.addListener(callback);
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn bookmarks_call(op: &str, args: &str) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_name = on_bookmarks_created)]
    pub fn chrome_bookmarks_on_created_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_bookmarks_removed)]
    pub fn chrome_bookmarks_on_removed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_bookmarks_changed)]
    pub fn chrome_bookmarks_on_changed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_bookmarks_moved)]
    pub fn chrome_bookmarks_on_moved_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen(inline_js = r#"
let queue = Promise.resolve();

function api() {
    const history = globalThis.chrome?.history;
    if (!history) {
        throw new Error('oxichrome:no-permission');
    }
    return history;
}

function serial(task) {
    const run = queue.then(task, task);
    queue = run.catch(() => {});
    return run;
}

function inclusive(end) {
    return typeof globalThis.browser?.runtime?.getBrowserInfo === 'function' ? end : end + 0.0015;
}

async function search(a) {
    const query = { text: a.text, startTime: a.startTime, maxResults: a.maxResults };
    if (a.endTime !== null) {
        query.endTime = inclusive(a.endTime);
    }
    return api().search(query);
}

async function call(op, a) {
    switch (op) {
        case 'search':
            return search(a);
        case 'visits':
            return api().getVisits({ url: a.url });
        case 'add':
            return serial(() => api().addUrl({ url: a.url }));
        case 'delete':
            return serial(() => api().deleteUrl({ url: a.url }));
        case 'delete_range':
            return serial(() => api().deleteRange({ startTime: a.startTime, endTime: inclusive(a.endTime) }));
        case 'delete_all':
            return serial(() => api().deleteAll());
        default:
            throw new Error(`unknown history call ${op}`);
    }
}

export function history_call(op, args) {
    api();
    return call(op, JSON.parse(args)).then((value) => JSON.stringify(value ?? null));
}

export function on_history_visited(callback) {
    globalThis.chrome?.history?.onVisited.addListener(callback);
}

export function on_history_visit_removed(callback) {
    globalThis.chrome?.history?.onVisitRemoved.addListener(callback);
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn history_call(op: &str, args: &str) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_name = on_history_visited)]
    pub fn chrome_history_on_visited_add_listener(callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(js_name = on_history_visit_removed)]
    pub fn chrome_history_on_visit_removed_add_listener(callback: &Closure<dyn FnMut(JsValue)>);
}

#[wasm_bindgen(inline_js = r#"
export function search_query(details) {
    const search = globalThis.chrome?.search;
    if (!search?.query) {
        throw new Error('oxichrome:no-permission');
    }
    const query = JSON.parse(details);
    const run = async () => {
        if (query.disposition === 'CURRENT_TAB') {
            const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
            if (!tab) {
                throw new Error('oxichrome:no-current-tab');
            }
            delete query.disposition;
            query.tabId = tab.id;
        }
        await search.query(query);
    };
    return run();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn search_query(details: &str) -> Result<js_sys::Promise, JsValue>;
}
