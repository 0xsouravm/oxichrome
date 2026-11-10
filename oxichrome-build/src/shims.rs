//! Generates JS shims and HTML shells for loading WASM in Chrome extensions.

use crate::source_parser::ExtensionMetadata;

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
    if (isInternalMessage(message)) {
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

fn generate_listeners(metadata: &ExtensionMetadata) -> String {
    let mut events: Vec<(String, Vec<String>)> = Vec::new();
    for handler in &metadata.event_handlers {
        let event = format!(
            "chrome.{}.{}",
            snake_to_camel(&handler.namespace),
            snake_to_camel(&handler.event_name)
        );
        let export = format!("__oxichrome_on_{}", handler.fn_name);
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
        } else {
            listeners.push_str(&format!(
                r#"{event}.addListener((...args) => {{
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
    for bg in &metadata.background_functions {
        exports.push(format!("__oxichrome_bg_{bg}"));
    }
    for handler in &metadata.event_handlers {
        exports.push(format!("__oxichrome_on_{}", handler.fn_name));
    }

    let imports = if exports.is_empty() {
        String::new()
    } else {
        format!(", {{ {} }}", exports.join(", "))
    };

    let mut listeners = generate_listeners(metadata);
    if !metadata.background_functions.is_empty() {
        listeners.push_str("chrome.runtime.onStartup.addListener(() => {});\n");
    }

    let mut init_calls = String::new();
    for bg in &metadata.background_functions {
        init_calls.push_str(&format!("        __oxichrome_bg_{bg}();\n"));
    }

    format!(
        r#"// Auto-generated by oxichrome. Do not edit.
import init{imports} from './wasm/{wasm_name}.js';

function isInternalMessage(message) {{
    return (
        message !== null &&
        typeof message === 'object' &&
        typeof message.__oxichrome_notification_cleared === 'string'
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
{init_calls}        ready = true;
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

pub fn generate_popup_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>html, body { margin: 0; padding: 0; min-width: 240px; min-height: 120px; }</style>
</head>
<body>
<script type="module" src="popup.js"></script>
</body>
</html>
"#
    .to_string()
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

pub fn generate_options_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>html, body { margin: 0; padding: 0; }</style>
</head>
<body>
<script type="module" src="options.js"></script>
</body>
</html>
"#
    .to_string()
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

pub fn generate_content_script_js(fn_name: &str, crate_name: &str) -> String {
    let wasm_name = crate_name.replace('-', "_");
    let wrapper_name = format!("__oxichrome_cs_{fn_name}");

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
    use crate::source_parser::EventHandler;

    #[test]
    fn test_background_js_es_module() {
        let metadata = ExtensionMetadata {
            name: Some("Test".to_string()),
            version: Some("1.0.0".to_string()),
            description: None,
            permissions: vec![],
            host_permissions: vec![],
            background_functions: vec!["start".to_string()],
            event_handlers: vec![EventHandler {
                fn_name: "handle_install".to_string(),
                namespace: "runtime".to_string(),
                event_name: "on_installed".to_string(),
            }],
            has_popup: false,
            has_options_page: false,
            content_scripts: vec![],
            gecko_id: None,
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
            background_functions: vec![],
            event_handlers: handlers
                .iter()
                .map(|(fn_name, namespace, event_name)| EventHandler {
                    fn_name: fn_name.to_string(),
                    namespace: namespace.to_string(),
                    event_name: event_name.to_string(),
                })
                .collect(),
            has_popup: false,
            has_options_page: false,
            content_scripts: vec![],
            gecko_id: None,
        }
    }

    #[test]
    fn test_background_js_starts_with_the_browser_when_it_has_a_background_fn() {
        let mut metadata = metadata_with_handlers(&[]);
        metadata.background_functions = vec!["start".to_string()];
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
        let html = generate_popup_html();
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
        let html = generate_options_html();
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
        let js = generate_content_script_js("inject", "my-extension");
        assert!(js.contains("chrome.runtime.getURL('wasm/my_extension.js')"));
        assert!(js.contains("__oxichrome_cs_inject"));
        assert!(js.contains("await wasm.default()"));
        assert!(js.contains("Content script \"inject\" initialized."));
        // Must NOT start with a static import statement
        assert!(!js.starts_with("import"));
    }
}
