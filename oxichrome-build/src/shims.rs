//! Generates JS shims and HTML shells for loading WASM in Chrome extensions.

use crate::metadata::ExtensionMetadata;

fn snake_to_camel(name: &str) -> String {
    let mut parts = name.split('_');
    let mut camel = parts.next().unwrap_or_default().to_string();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            camel.extend(first.to_uppercase());
            camel.push_str(chars.as_str());
        }
    }
    camel
}

const NOTIFICATION_HELPERS: &str = r#"const clearedNotifications = new Set();

globalThis.__oxichrome_notification_cleared = (id) => {
    clearedNotifications.add(id);
};

chrome.runtime.onMessage.addListener((message) => {
    if (isInternalMessage(message) && typeof message.__oxichrome_notification_cleared === 'string') {
        clearedNotifications.add(message.__oxichrome_notification_cleared);
        return Promise.resolve(true);
    }
});

function notificationUserId(id) {
    return typeof id === 'string' ? id.split('\u001f')[0] : id;
}

async function notificationClosedArgs(id, byUser) {
    if (clearedNotifications.delete(id)) {
        return null;
    }
    const userId = notificationUserId(id);
    const open = Object.keys(await chrome.notifications.getAll());
    if (open.some((other) => other !== id && notificationUserId(other) === userId)) {
        return null;
    }
    const reportsByUser = typeof chrome.notifications.update === 'function';
    return [userId, reportsByUser ? byUser : undefined];
}
"#;

fn menu_items_json(metadata: &ExtensionMetadata) -> serde_json::Value {
    let items = metadata
        .menus
        .iter()
        .map(|item| {
            let mut object = serde_json::Map::new();
            object.insert("id".into(), item.id.clone().into());
            object.insert("title".into(), item.title.clone().into());
            object.insert("contexts".into(), item.contexts.clone().into());
            if item.item_type == "checkbox" || item.item_type == "radio" {
                object.insert("type".into(), item.item_type.clone().into());
            }
            if let Some(checked) = item.checked {
                object.insert("checked".into(), checked.into());
            }
            if let Some(parent) = &item.parent {
                object.insert("parentId".into(), parent.clone().into());
            }
            if let Some(enabled) = item.enabled {
                object.insert("enabled".into(), enabled.into());
            }
            if let Some(visible) = item.visible {
                object.insert("visible".into(), visible.into());
            }
            if !item.document_url_patterns.is_empty() {
                object.insert("documentUrlPatterns".into(), item.document_url_patterns.clone().into());
            }
            if !item.target_url_patterns.is_empty() {
                object.insert("targetUrlPatterns".into(), item.target_url_patterns.clone().into());
            }
            serde_json::Value::Object(object)
        })
        .collect();
    serde_json::Value::Array(items)
}

fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn generate_menus(metadata: &ExtensionMetadata) -> String {
    if metadata.menus.is_empty() {
        return String::new();
    }
    let items = menu_items_json(metadata).to_string();
    let marker = format!("__oxichrome_menus_{:016x}", fnv1a(&items));
    let localized = metadata
        .menus
        .iter()
        .any(|item| item.title.starts_with("__MSG_") && item.title.ends_with("__"));
    let marker = if localized {
        format!("'{marker}_' + chrome.i18n.getUILanguage().replace(/[^A-Za-z0-9]/g, '_')")
    } else {
        format!("'{marker}'")
    };
    let handlers = metadata
        .menus
        .iter()
        .filter_map(|item| item.export.as_ref().map(|export| (&item.id, export)))
        .map(|(id, export)| format!("    {id}: {export},"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"const menuItems = {items};
const menuMarker = {marker};
const menuHandlers = {{
{handlers}
}};

function menuExists(id) {{
    return new Promise((resolve) => {{
        try {{
            chrome.contextMenus.update(id, {{}}, () => resolve(!chrome.runtime.lastError));
        }} catch (error) {{
            resolve(false);
        }}
    }});
}}

function createMenu(item) {{
    const message = /^__MSG_([A-Za-z0-9_@]+)__$/.exec(item.title);
    const title = message ? chrome.i18n.getMessage(message[1]) : item.title;
    const properties = {{ ...item, title: title.replaceAll('&', '&&') }};
    return new Promise((resolve, reject) => {{
        chrome.contextMenus.create(properties, () => {{
            const error = chrome.runtime.lastError;
            if (error) {{
                reject(new Error(`[oxichrome] couldn't create context menu item ${{item.id}}: ${{error.message}}`));
            }} else {{
                resolve();
            }}
        }});
    }});
}}

async function ensureMenus() {{
    if (await menuExists(menuMarker)) {{
        return;
    }}
    await chrome.contextMenus.removeAll();
    for (const item of menuItems) {{
        await createMenu(item);
    }}
    await createMenu({{ id: menuMarker, title: 'oxichrome', contexts: ['page'], visible: false, enabled: false }});
}}

const menusReady = ensureMenus().catch((error) => console.error(error));

chrome.contextMenus.onClicked.addListener((info, tab) => {{
    const handler = menuHandlers[info.menuItemId];
    if (!handler) {{
        return;
    }}
    const click = {{
        ...info,
        tabId: tab && tab.id >= 0 ? tab.id : undefined,
        windowId: tab && tab.windowId >= 0 ? tab.windowId : undefined,
    }};
    dispatch(() => handler(click));
}});
"#
    )
}

fn generate_commands(metadata: &ExtensionMetadata) -> String {
    if metadata.commands.is_empty() {
        return String::new();
    }
    let handlers = metadata
        .commands
        .iter()
        .map(|command| format!("    {}: {},", command.name, command.export))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"const commandHandlers = {{
{handlers}
}};

chrome.commands.onCommand.addListener((name, tab) => {{
    const handler = commandHandlers[name];
    if (!handler) {{
        return;
    }}
    const press = {{
        tabId: tab && tab.id >= 0 ? tab.id : undefined,
        windowId: tab && tab.windowId >= 0 ? tab.windowId : undefined,
    }};
    dispatch(() => handler(press));
}});
"#
    )
}

fn generate_omnibox(metadata: &ExtensionMetadata) -> String {
    let Some(omnibox) = &metadata.omnibox else {
        return String::new();
    };
    let default = serde_json::to_string(&omnibox.default).unwrap_or_else(|_| "null".to_string());
    format!(
        r#"const omniboxFirefox = typeof globalThis.browser?.runtime?.getBrowserInfo === 'function';
const omniboxDefault = {default};
let omniboxTurn = 0;

function omniboxEscape(text) {{
    return text.replace(/[&<>"']/g, (c) => ({{ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' }})[c]);
}}

if (omniboxDefault !== null) {{
    const description = omniboxFirefox
        ? omniboxDefault.split('{{input}}').join('').replace(/\s+/g, ' ').trim()
        : omniboxEscape(omniboxDefault).split('{{input}}').join('%s');
    try {{
        Promise.resolve(chrome.omnibox.setDefaultSuggestion({{ description }})).catch((error) => console.error('[oxichrome] Omnibox:', error));
    }} catch (error) {{
        console.error('[oxichrome] Omnibox:', error);
    }}
}}

chrome.omnibox.onInputChanged.addListener((text, suggest) => {{
    const turn = ++omniboxTurn;
    dispatch(() => {{
        Promise.resolve(__oxichrome_omnibox_suggest(text)).then((list) => {{
            if (turn !== omniboxTurn || !Array.isArray(list)) {{
                return;
            }}
            suggest(list.map((item) => ({{ content: item.content, description: omniboxFirefox ? item.plain : item.markup }})));
        }}, (error) => console.error('[oxichrome] `{name}` failed:', error));
    }});
}});

"#,
        name = omnibox.fn_name
    )
}

fn generate_side_panel(metadata: &ExtensionMetadata) -> String {
    let Some(panel) = &metadata.side_panel else {
        return String::new();
    };
    let firefox = if panel.open_on_action_click {
        r#" else if (globalThis.browser?.sidebarAction) {
    chrome.action.onClicked.addListener(() => {
        globalThis.browser.sidebarAction.toggle().catch((error) => console.error('[oxichrome] Side panel:', error));
    });
}"#
    } else {
        ""
    };
    format!(
        r#"if (globalThis.chrome.sidePanel) {{
    chrome.sidePanel.setPanelBehavior({{ openPanelOnActionClick: {} }}).catch((error) => console.error('[oxichrome] Side panel:', error));
}}{firefox}

"#,
        panel.open_on_action_click
    )
}

pub const CHROME_ONLY_EVENTS: &[&str] = &["windows.onBoundsChanged", "sidePanel.onOpened", "sidePanel.onClosed"];

pub fn left_out_events(metadata: &ExtensionMetadata, browser: crate::Browser) -> Vec<String> {
    if browser != crate::Browser::Firefox {
        return Vec::new();
    }
    let mut left_out: Vec<String> = metadata
        .event_handlers
        .iter()
        .filter(|handler| {
            CHROME_ONLY_EVENTS.contains(&format!("{}.{}", snake_to_camel(&handler.namespace), snake_to_camel(&handler.event_name)).as_str())
        })
        .map(|handler| format!("`{}` (`{}::{}`)", handler.fn_name, handler.namespace, handler.event_name))
        .collect();
    left_out.sort();
    left_out
}

fn generate_listeners(metadata: &ExtensionMetadata) -> String {
    let mut events: Vec<(String, Vec<String>)> = Vec::new();
    for handler in &metadata.event_handlers {
        let event = format!(
            "chrome.{}.{}",
            snake_to_camel(&handler.namespace),
            snake_to_camel(&handler.event_name)
        );
        let export = handler.export.clone();
        match events.iter_mut().find(|(name, _)| *name == event) {
            Some((_, exports)) => exports.push(export),
            None => events.push((event, vec![export])),
        }
    }

    let mut listeners = String::new();
    if events
        .iter()
        .any(|(event, _)| event.starts_with("chrome.notifications."))
    {
        listeners.push_str(NOTIFICATION_HELPERS);
    }
    for (event, exports) in &events {
        let handlers = exports.join(", ");
        if event == "chrome.runtime.onMessage" {
            listeners.push_str(&format!(
                r#"const openReplies = new Set();

function releaseRepliesOnTrap(error) {{
    if (error instanceof WebAssembly.RuntimeError) {{
        openReplies.forEach((reply) => reply());
        openReplies.clear();
    }}
}}

self.addEventListener('error', (event) => releaseRepliesOnTrap(event.error));
self.addEventListener('unhandledrejection', (event) => releaseRepliesOnTrap(event.reason));

{event}.addListener((...args) => {{
    if (isInternalMessage(args[0])) {{
        return;
    }}
    const reply = args[2];
    openReplies.add(reply);
    dispatch(() => {{
        Promise.allSettled([{handlers}].map(async (handler) => handler(...args)))
            .then(() => {{
                openReplies.delete(reply);
                reply();
            }});
    }});
    return true;
}});
"#
            ));
        } else if event == "chrome.notifications.onClosed" {
            listeners.push_str(&format!(
                r#"{event}.addListener((id, byUser) => {{
    dispatch(() => {{
        notificationClosedArgs(id, byUser).then((args) => {{
            if (args) {{
                [{handlers}].forEach((handler) => handler(...args));
            }}
        }});
    }});
}});
"#
            ));
        } else if event.starts_with("chrome.notifications.") {
            listeners.push_str(&format!(
                r#"{event}.addListener((id, ...rest) => {{
    dispatch(() => [{handlers}].forEach((handler) => handler(notificationUserId(id), ...rest)));
}});
"#
            ));
        } else if event == "chrome.downloads.onChanged" {
            listeners.push_str(&format!(
                r#"const downloadsSeen = new Map();
const downloadsSettling = new Map();

chrome.downloads.onErased.addListener((id) => {{
    downloadsSeen.delete(id);
    clearTimeout(downloadsSettling.get(id));
    downloadsSettling.delete(id);
}});

{event}.addListener((delta) => {{
    clearTimeout(downloadsSettling.get(delta.id));
    downloadsSettling.set(delta.id, setTimeout(() => {{
        downloadsSettling.delete(delta.id);
        dispatch(() => {{
            chrome.downloads.search({{ id: delta.id }}).then(([item]) => {{
                if (!item) {{
                    return;
                }}
                const {{ bytesReceived, estimatedEndTime, ...rest }} = item;
                const key = JSON.stringify(rest);
                if (downloadsSeen.get(item.id) === key) {{
                    return;
                }}
                downloadsSeen.set(item.id, key);
                [{handlers}].forEach((handler) => handler(item));
            }});
        }});
    }}, 200));
}});
"#
            ));
        } else if event == "chrome.windows.onFocusChanged" {
            listeners.push_str(&format!(
                r#"{event}.addListener((id) => {{
    dispatch(() => [{handlers}].forEach((handler) => handler(id === chrome.windows.WINDOW_ID_NONE ? null : id)));
}});
"#
            ));
        } else {
            let (event, call) = if CHROME_ONLY_EVENTS.contains(&event.trim_start_matches("chrome.")) {
                (event.replacen("chrome.", "globalThis.chrome.", 1).replace('.', "?.").replacen("globalThis?.", "globalThis.", 1), "?.addListener")
            } else {
                (event.clone(), ".addListener")
            };
            listeners.push_str(&format!(
                r#"{event}{call}((...args) => {{
    dispatch(() => [{handlers}].forEach((handler) => handler(...args)));
}});
"#
            ));
        }
    }
    listeners
}

pub fn generate_background_js(metadata: &ExtensionMetadata, crate_name: &str) -> String {
    let wasm_name = crate_name.replace('-', "_");

    let mut exports = Vec::new();
    exports.extend(metadata.background_exports.iter().cloned());
    exports.extend(metadata.event_handlers.iter().map(|handler| handler.export.clone()));
    exports.extend(metadata.menus.iter().filter_map(|item| item.export.clone()));
    exports.extend(metadata.commands.iter().map(|command| command.export.clone()));
    if metadata.omnibox.is_some() {
        exports.push("__oxichrome_omnibox_suggest".to_string());
    }

    let imports = if exports.is_empty() {
        String::new()
    } else {
        format!(", {{ {} }}", exports.join(", "))
    };

    let mut listeners = generate_side_panel(metadata);
    listeners.push_str(&generate_omnibox(metadata));
    listeners.push_str(&generate_menus(metadata));
    listeners.push_str(&generate_commands(metadata));
    listeners.push_str(&generate_listeners(metadata));
    if !metadata.background_exports.is_empty() {
        listeners.push_str("chrome.runtime.onStartup.addListener(() => {});\n");
    }

    let menus_ready = if metadata.menus.is_empty() {
        ""
    } else {
        "        await menusReady;\n"
    };

    let mut init_calls = String::new();
    for bg in &metadata.background_exports {
        init_calls.push_str(&format!("        {bg}();\n"));
    }

    format!(
        r#"// Auto-generated by oxichrome. Do not edit.
import init{imports} from './wasm/{wasm_name}.js';

function isInternalMessage(message) {{
    return (
        message !== null &&
        typeof message === 'object' &&
        (typeof message.__oxichrome_notification_cleared === 'string' ||
            typeof message.__oxichrome_download_save === 'object')
    );
}}

let ready = false;
const pending = [];

function dispatch(run) {{
    if (ready) {{
        run();
    }} else {{
        pending.push(run);
    }}
}}

{listeners}
async function start() {{
    try {{
        await init();
{menus_ready}{init_calls}        ready = true;
        pending.splice(0).forEach((run) => run());
        console.log('[oxichrome] Background service worker initialized.');
    }} catch (e) {{
        console.error('[oxichrome] Failed to initialize WASM:', e);
    }}
}}

start();
"#
    )
}

pub fn generate_popup_html(styles: &[String]) -> String {
    let links: String = styles.iter().map(|style| format!("<link rel=\"stylesheet\" href=\"{style}\">\n")).collect();
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<meta name="color-scheme" content="light dark">
<style>html, body {{ margin: 0; padding: 0; min-width: 240px; min-height: 120px; }}</style>
{links}</head>
<body>
<script type="module" src="popup.js"></script>
</body>
</html>
"#
    )
}

pub fn generate_popup_js(crate_name: &str) -> String {
    let wasm_name = crate_name.replace('-', "_");

    format!(
        r#"// Auto-generated by oxichrome. Do not edit.
import init, {{ __oxichrome_mount_popup }} from './wasm/{wasm_name}.js';

try {{
    await init();
    __oxichrome_mount_popup();
}} catch (e) {{
    console.error('[oxichrome] Popup init failed:', e);
    document.body.textContent = 'Error: ' + e.message;
}}
"#
    )
}

pub fn generate_side_panel_html(styles: &[String]) -> String {
    let links: String = styles.iter().map(|style| format!("<link rel=\"stylesheet\" href=\"{style}\">\n")).collect();
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<meta name="color-scheme" content="light dark">
<style>html, body {{ margin: 0; padding: 0; }}</style>
{links}</head>
<body>
<script type="module" src="sidepanel.js"></script>
</body>
</html>
"#
    )
}

pub fn generate_side_panel_js(crate_name: &str) -> String {
    let wasm_name = crate_name.replace('-', "_");

    format!(
        r#"// Auto-generated by oxichrome. Do not edit.
import init, {{ __oxichrome_mount_side_panel }} from './wasm/{wasm_name}.js';

try {{
    await init();
    __oxichrome_mount_side_panel();
}} catch (e) {{
    console.error('[oxichrome] Side panel init failed:', e);
    document.body.textContent = 'Error: ' + e.message;
}}
"#
    )
}

pub fn generate_options_html(styles: &[String]) -> String {
    let links: String = styles.iter().map(|style| format!("<link rel=\"stylesheet\" href=\"{style}\">\n")).collect();
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<meta name="color-scheme" content="light dark">
<style>html, body {{ margin: 0; padding: 0; }}</style>
{links}</head>
<body>
<script type="module" src="options.js"></script>
</body>
</html>
"#
    )
}

pub fn generate_options_js(crate_name: &str) -> String {
    let wasm_name = crate_name.replace('-', "_");

    format!(
        r#"// Auto-generated by oxichrome. Do not edit.
import init, {{ __oxichrome_mount_options }} from './wasm/{wasm_name}.js';

try {{
    await init();
    __oxichrome_mount_options();
}} catch (e) {{
    console.error('[oxichrome] Options init failed:', e);
    document.body.textContent = 'Error: ' + e.message;
}}
"#
    )
}

pub fn generate_content_script_js(fn_name: &str, wrapper_name: &str, crate_name: &str) -> String {
    let wasm_name = crate_name.replace('-', "_");

    format!(
        r#"// Auto-generated by oxichrome. Do not edit.
// Content script: {fn_name}
(async () => {{
    try {{
        const src = chrome.runtime.getURL('wasm/{wasm_name}.js');
        const wasm = await import(src);
        await wasm.default();
        wasm.{wrapper_name}();
        console.log('[oxichrome] Content script "{fn_name}" initialized.');
    }} catch (e) {{
        console.error('[oxichrome] Content script "{fn_name}" failed to initialize:', e);
    }}
}})();
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::EventHandler;

    #[test]
    fn test_background_js_es_module() {
        let metadata = ExtensionMetadata {
            name: Some("Test".to_string()),
            version: Some("1.0.0".to_string()),
            description: None,
            permissions: vec![],
            host_permissions: vec![],
            optional_permissions: vec![],
            optional_host_permissions: vec![],
            chrome_only: vec![],
            firefox_only: vec![],
            background_exports: vec!["__oxichrome_bg_start".to_string()],
            event_handlers: vec![EventHandler {
                fn_name: "handle_install".to_string(),
            export: "__oxichrome_on_handle_install".to_string(),
                namespace: "runtime".to_string(),
                event_name: "on_installed".to_string(),
            }],
            has_popup: false,
            has_options_page: false,
            side_panel: None,
            omnibox: None,
            popup_styles: Vec::new(),
            options_styles: Vec::new(),
            content_scripts: vec![],
            gecko_id: None,
            display_name: None,
            default_locale: None,
            messages: Vec::new(),
            icons: vec![],
            action: None,
            menus: vec![],
            commands: vec![],
        };

        let js = generate_background_js(&metadata, "counter-extension");
        assert!(js.contains("import init, { __oxichrome_bg_start, __oxichrome_on_handle_install }"));
        assert!(js.contains("from './wasm/counter_extension.js'"));
        assert!(js.contains("await init();"));
        assert!(js.contains("__oxichrome_bg_start();"));

        let listener = js.find("chrome.runtime.onInstalled.addListener").unwrap();
        let init = js.find("await init();").unwrap();
        assert!(listener < init);
        assert!(js.contains("[__oxichrome_on_handle_install].forEach"));
    }

    fn metadata_with_handlers(handlers: &[(&str, &str, &str)]) -> ExtensionMetadata {
        ExtensionMetadata {
            name: Some("Test".to_string()),
            version: Some("1.0.0".to_string()),
            description: None,
            permissions: vec![],
            host_permissions: vec![],
            optional_permissions: vec![],
            optional_host_permissions: vec![],
            chrome_only: vec![],
            firefox_only: vec![],
            background_exports: vec![],
            event_handlers: handlers
                .iter()
                .map(|(fn_name, namespace, event_name)| EventHandler {
                    fn_name: fn_name.to_string(),
                    export: format!("__oxichrome_on_{fn_name}"),
                    namespace: namespace.to_string(),
                    event_name: event_name.to_string(),
                })
                .collect(),
            has_popup: false,
            has_options_page: false,
            side_panel: None,
            omnibox: None,
            popup_styles: Vec::new(),
            options_styles: Vec::new(),
            content_scripts: vec![],
            gecko_id: None,
            display_name: None,
            default_locale: None,
            messages: Vec::new(),
            icons: vec![],
            action: None,
            menus: vec![],
            commands: vec![],
        }
    }

    #[test]
    fn test_background_js_starts_with_the_browser_when_it_has_a_background_fn() {
        let mut metadata = metadata_with_handlers(&[]);
        metadata.background_exports = vec!["__oxichrome_bg_start".to_string()];
        let js = generate_background_js(&metadata, "ext");

        let startup = js.find("chrome.runtime.onStartup.addListener").unwrap();
        let init = js.find("await init();").unwrap();
        assert!(startup < init);
        assert_eq!(
            js.matches("chrome.runtime.onStartup.addListener").count(),
            1
        );
    }

    #[test]
    fn test_background_js_without_a_background_fn_does_not_start_with_the_browser() {
        let metadata = metadata_with_handlers(&[("clicked", "action", "on_clicked")]);
        let js = generate_background_js(&metadata, "ext");

        assert!(!js.contains("onStartup"));
    }

    #[test]
    fn test_background_js_routes_notification_events() {
        let metadata = metadata_with_handlers(&[
            ("closed", "notifications", "on_closed"),
            ("clicked", "notifications", "on_clicked"),
            ("button", "notifications", "on_button_clicked"),
        ]);
        let js = generate_background_js(&metadata, "ext");

        assert_eq!(js.matches("const clearedNotifications").count(), 1);
        assert!(js.contains("globalThis.__oxichrome_notification_cleared"));
        assert!(js.contains("notificationClosedArgs(id, byUser)"));
        assert!(js.contains(
            "[__oxichrome_on_clicked].forEach((handler) => handler(notificationUserId(id), ...rest))"
        ));
        assert!(js.contains(
            "[__oxichrome_on_button].forEach((handler) => handler(notificationUserId(id), ...rest))"
        ));
        let helpers = js.find("const clearedNotifications").unwrap();
        let init = js.find("await init();").unwrap();
        assert!(helpers < init);
    }

    #[test]
    fn test_background_js_without_notification_handlers_has_no_helpers() {
        let metadata = metadata_with_handlers(&[("removed", "tabs", "on_removed")]);
        let js = generate_background_js(&metadata, "ext");

        assert!(!js.contains("clearedNotifications"));
        assert!(!js.contains("notificationUserId"));
    }

    #[test]
    fn test_background_js_message_handlers_skip_internal_messages() {
        let metadata = metadata_with_handlers(&[
            ("message", "runtime", "on_message"),
            ("closed", "notifications", "on_closed"),
        ]);
        let js = generate_background_js(&metadata, "ext");

        let listener = js
            .find("chrome.runtime.onMessage.addListener((...args)")
            .unwrap();
        let skip = js[listener..]
            .find("if (isInternalMessage(args[0]))")
            .unwrap();
        let reply = js[listener..].find("const reply = args[2];").unwrap();
        assert!(skip < reply);
    }

    #[test]
    fn test_background_js_groups_handlers_of_one_event() {
        let metadata = metadata_with_handlers(&[
            ("first", "tabs", "on_removed"),
            ("second", "tabs", "on_removed"),
        ]);
        let js = generate_background_js(&metadata, "ext");

        assert_eq!(js.matches("chrome.tabs.onRemoved.addListener").count(), 1);
        assert!(js.contains("[__oxichrome_on_first, __oxichrome_on_second]"));
    }

    #[test]
    fn test_background_js_keeps_message_channel_open() {
        let metadata = metadata_with_handlers(&[("msg", "runtime", "on_message")]);
        let js = generate_background_js(&metadata, "ext");

        assert!(js.contains("chrome.runtime.onMessage.addListener"));
        assert!(js.contains("return true;"));
        assert!(js.contains("Promise.allSettled"));
        assert!(js.contains("WebAssembly.RuntimeError"));
    }

    #[test]
    fn test_background_js_other_events_do_not_hold_a_channel() {
        let metadata = metadata_with_handlers(&[("clicked", "action", "on_clicked")]);
        let js = generate_background_js(&metadata, "ext");

        assert!(js.contains("chrome.action.onClicked.addListener"));
        assert!(!js.contains("return true;"));
        assert!(!js.contains("openReplies"));
    }

    #[test]
    fn test_snake_to_camel() {
        assert_eq!(snake_to_camel("runtime"), "runtime");
        assert_eq!(snake_to_camel("on_installed"), "onInstalled");
        assert_eq!(snake_to_camel("context_menus"), "contextMenus");
    }

    #[test]
    fn test_popup_html() {
        let html = generate_popup_html(&[]);
        assert!(html.contains("src=\"popup.js\""));
        assert!(html.contains("min-width: 240px"));
        assert!(!html.contains("import init"));
    }

    #[test]
    fn test_popup_js() {
        let js = generate_popup_js("counter-extension");
        assert!(js.contains("__oxichrome_mount_popup"));
        assert!(js.contains("counter_extension.js"));
        assert!(js.contains("await init();"));
    }

    #[test]
    fn test_options_html() {
        let html = generate_options_html(&[]);
        assert!(html.contains("src=\"options.js\""));
        assert!(!html.contains("import init"));
    }

    #[test]
    fn test_options_js() {
        let js = generate_options_js("my-ext");
        assert!(js.contains("__oxichrome_mount_options"));
        assert!(js.contains("my_ext.js"));
        assert!(js.contains("await init();"));
    }

    #[test]
    fn test_content_script_js() {
        let js = generate_content_script_js("inject", "__oxichrome_cs_inject_1234abcd", "my-extension");
        assert!(js.contains("chrome.runtime.getURL('wasm/my_extension.js')"));
        assert!(js.contains("wasm.__oxichrome_cs_inject_1234abcd();"));
        assert!(js.contains("await wasm.default()"));
        assert!(js.contains("Content script \"inject\" initialized."));
        // Must NOT start with a static import statement
        assert!(!js.starts_with("import"));
    }

    fn metadata_with_menus() -> ExtensionMetadata {
        let mut metadata = metadata_with_handlers(&[]);
        metadata.menus = vec![
            crate::metadata::MenuItem {
            id: "Tools".to_string(),
            handler: None,
            export: None,
            item_type: "submenu".to_string(),
            title: "T & Tools".to_string(),
            contexts: vec!["page".to_string()],
            checked: None,
            parent: None,
            enabled: None,
            visible: None,
            document_url_patterns: vec![],
            target_url_patterns: vec![],
        },
            crate::metadata::MenuItem {
            id: "dark".to_string(),
            handler: Some("dark".to_string()),
            export: Some("__oxichrome_menu_dark".to_string()),
            item_type: "checkbox".to_string(),
            title: "T & dark".to_string(),
            contexts: vec!["page".to_string()],
            checked: Some(true),
            parent: Some("Tools".to_string()),
            enabled: None,
            visible: None,
            document_url_patterns: vec![],
            target_url_patterns: vec![],
        },
        ];
        metadata
    }

    #[test]
    fn test_menus_are_created_once_per_definition_and_routed_by_id() {
        let js = generate_background_js(&metadata_with_menus(), "ext");
        assert!(js.contains("import init, { __oxichrome_menu_dark } from './wasm/ext.js';"), "{js}");
        assert!(js.contains(r#"const menuItems = [{"contexts":["page"],"id":"Tools","title":"T & Tools"},{"checked":true,"contexts":["page"],"id":"dark","parentId":"Tools","title":"T & dark","type":"checkbox"}];"#), "{js}");
        assert!(js.contains("    dark: __oxichrome_menu_dark,"), "{js}");
        assert!(js.contains("const menuMarker = '__oxichrome_menus_"), "{js}");
        assert!(js.contains("await menusReady;"), "{js}");
        assert!(js.contains("replaceAll('&', '&&')"), "{js}");

        let mut changed = metadata_with_menus();
        changed.menus[1].title = "Other".to_string();
        let marker = |js: &str| js.split("const menuMarker = '").nth(1).unwrap().split('\'').next().unwrap().to_string();
        assert_ne!(marker(&js), marker(&generate_background_js(&changed, "ext")));
        assert_eq!(marker(&js), marker(&generate_background_js(&metadata_with_menus(), "ext")));
    }

    #[test]
    fn test_no_menus_means_no_menu_code() {
        let js = generate_background_js(&metadata_with_handlers(&[]), "ext");
        assert!(!js.contains("contextMenus"), "{js}");
        assert!(!js.contains("menusReady"), "{js}");
    }

    #[test]
    fn test_commands_are_routed_by_name_to_their_export() {
        let mut metadata = metadata_with_handlers(&[]);
        metadata.commands = vec![crate::metadata::Command {
            name: "toggle".to_string(),
            export: "__oxichrome_command_toggle_1a2b3c4d".to_string(),
            description: "d".to_string(),
            shortcut: None,
        }];
        let js = generate_background_js(&metadata, "my-ext");
        assert!(js.contains("import init, { __oxichrome_command_toggle_1a2b3c4d }"), "{js}");
        assert!(js.contains("    toggle: __oxichrome_command_toggle_1a2b3c4d,"), "{js}");
        assert!(js.contains("chrome.commands.onCommand.addListener((name, tab) => {"), "{js}");
        assert!(js.contains("tabId: tab && tab.id >= 0 ? tab.id : undefined"), "{js}");
        metadata.commands.clear();
        assert!(!generate_background_js(&metadata, "my-ext").contains("chrome.commands"));
    }

    #[test]
    fn side_panel_background() {
        let source = r#"{"kind":"extension","name":"P","version":"1.0.0","description":null,"gecko_id":null,"permissions":[],"host_permissions":[],"icons":[],"action":null}
{"kind":"side_panel","fn_name":"Panel","title":null,"open_on_action_click":false}
{"kind":"event","fn_name":"opened","export":"__opened","namespace":"side_panel","event_name":"on_opened"}"#;
        let metadata = crate::metadata::parse_entries(source).unwrap();
        let js = generate_background_js(&metadata, "ext");
        assert!(js.contains("setPanelBehavior({ openPanelOnActionClick: false })"), "{js}");
        assert!(!js.contains("sidebarAction"), "{js}");
        assert!(js.contains("globalThis.chrome?.sidePanel?.onOpened?.addListener("), "{js}");
        assert_eq!(left_out_events(&metadata, crate::Browser::Firefox), vec!["`opened` (`side_panel::on_opened`)".to_string()]);
        assert!(left_out_events(&metadata, crate::Browser::Chromium).is_empty());
        let metadata = crate::metadata::parse_entries(&source.replace("\"open_on_action_click\":false", "\"open_on_action_click\":true")).unwrap();
        let js = generate_background_js(&metadata, "ext");
        assert!(js.contains("setPanelBehavior({ openPanelOnActionClick: true })"), "{js}");
        assert!(
            js.contains("chrome.action.onClicked.addListener(() => {\n        globalThis.browser.sidebarAction.toggle()"),
            "the Firefox listener must call toggle right away, not through dispatch: {js}"
        );
        assert!(generate_side_panel_js("my-ext").contains("import init, { __oxichrome_mount_side_panel } from './wasm/my_ext.js'"));
        assert!(generate_side_panel_html(&[]).contains("src=\"sidepanel.js\""));
    }

    #[test]
    fn download_changes_send_the_settled_download_once() {
        let metadata = metadata_with_handlers(&[("changed", "downloads", "on_changed")]);
        let js = generate_background_js(&metadata, "test");
        assert_eq!(js.matches("chrome.downloads.onChanged.addListener").count(), 1);
        assert!(js.contains("chrome.downloads.search({ id: delta.id })"));
        assert!(js.contains("}, 200));"));
        assert!(js.contains("[__oxichrome_on_changed].forEach((handler) => handler(item))"));
        assert!(js.contains("typeof message.__oxichrome_download_save === 'object'"));
    }

    #[test]
    fn pages_follow_the_browser_theme() {
        for html in [generate_popup_html(&[]), generate_options_html(&[]), generate_side_panel_html(&[])] {
            assert!(html.contains(r#"<meta name="color-scheme" content="light dark">"#), "{html}");
        }
    }

    #[test]
    fn pages_link_their_stylesheets_after_the_base_style() {
        let styles = ["base.css".to_string(), "css/popup.css".to_string()];
        for html in [generate_popup_html(&styles), generate_options_html(&styles), generate_side_panel_html(&styles)] {
            let base = html.find("<style>").unwrap();
            let first = html.find(r#"<link rel="stylesheet" href="base.css">"#).unwrap();
            let second = html.find(r#"<link rel="stylesheet" href="css/popup.css">"#).unwrap();
            assert!(base < first && first < second && second < html.find("</head>").unwrap(), "{html}");
        }
        assert!(!generate_popup_html(&[]).contains("<link"));
    }
}
