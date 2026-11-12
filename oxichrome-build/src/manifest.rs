use serde::Serialize;

use crate::metadata::{ActionIcon, ExtensionMetadata};
use crate::Browser;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct Manifest {
    manifest_version: u32,
    name: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_locale: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    permissions: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    optional_permissions: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    host_permissions: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    optional_host_permissions: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    icons: BTreeMap<u32, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<Action>,
    background: BackgroundConfig,
    content_security_policy: ContentSecurityPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    options_ui: Option<OptionsUi>,
    #[serde(skip_serializing_if = "Option::is_none")]
    side_panel: Option<ChromeSidePanel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sidebar_action: Option<SidebarAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    omnibox: Option<ManifestOmnibox>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    content_scripts: Vec<ContentScriptEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    web_accessible_resources: Vec<WebAccessibleResource>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    commands: BTreeMap<String, ManifestCommand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    declarative_net_request: Option<DeclarativeNetRequest>,
}

#[derive(Serialize)]
struct DeclarativeNetRequest {
    rule_resources: Vec<RuleResource>,
}

#[derive(Serialize)]
struct RuleResource {
    id: String,
    enabled: bool,
    path: String,
}

#[derive(Serialize)]
struct ManifestCommand {
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_key: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Serialize)]
struct Action {
    #[serde(skip_serializing_if = "Option::is_none")]
    default_popup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_icon: Option<DefaultIcon>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum DefaultIcon {
    Single(String),
    Sizes(BTreeMap<u32, String>),
}

#[derive(Serialize)]
struct BackgroundConfig {
    service_worker: String,
    #[serde(rename = "type")]
    worker_type: String,
}

#[derive(Serialize)]
struct ContentSecurityPolicy {
    extension_pages: String,
}

#[derive(Serialize)]
struct ManifestOmnibox {
    keyword: String,
}

#[derive(Serialize)]
struct ChromeSidePanel {
    default_path: String,
}

#[derive(Serialize)]
struct SidebarAction {
    default_panel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_title: Option<String>,
    open_at_install: bool,
}

#[derive(Serialize)]
struct OptionsUi {
    page: String,
    open_in_tab: bool,
}

#[derive(Serialize)]
struct ContentScriptEntry {
    matches: Vec<String>,
    js: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    all_frames: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    css: Vec<String>,
}

#[derive(Serialize)]
struct WebAccessibleResource {
    resources: Vec<String>,
    matches: Vec<String>,
}

pub fn left_out_permissions(metadata: &ExtensionMetadata, browser: Browser) -> &[(String, String)] {
    match browser {
        Browser::Chromium => &metadata.firefox_only,
        Browser::Firefox => &metadata.chrome_only,
    }
}

fn for_browser(names: &[String], metadata: &ExtensionMetadata, browser: Browser) -> Vec<String> {
    let left_out = left_out_permissions(metadata, browser);
    names
        .iter()
        .filter(|name| !left_out.iter().any(|(left, _)| left == *name))
        .cloned()
        .collect()
}

pub fn generated_gecko_id(name: &str) -> String {
    let mut local = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
            local.push(c);
        } else if !local.ends_with('-') {
            local.push('-');
        }
    }
    let local = local.trim_matches(|c| c == '-' || c == '.');
    let local = if local.is_empty() { "extension" } else { local };
    format!("{local}@oxichrome.dev")
}

pub fn generate_manifest(metadata: &ExtensionMetadata, browser: Browser) -> anyhow::Result<String> {
    let name = metadata
        .name
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("extension name is required"))?;
    let version = metadata
        .version
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("extension version is required"))?;

    let manifest = Manifest {
        manifest_version: 3,
        name: name.to_string(),
        version: version.to_string(),
        description: metadata.description.clone(),
        default_locale: metadata.default_locale.clone(),
        permissions: {
            let mut permissions = for_browser(&metadata.permissions, metadata, browser);
            if !metadata.menus.is_empty() && !permissions.iter().any(|p| p == "contextMenus") {
                permissions.push("contextMenus".to_string());
            }
            if metadata.event_handlers.iter().any(|handler| handler.namespace == "idle") && !permissions.iter().any(|p| p == "storage") {
                permissions.push("storage".to_string());
            }
            if metadata.side_panel.is_some() && browser == Browser::Chromium && !permissions.iter().any(|p| p == "sidePanel") {
                permissions.push("sidePanel".to_string());
            }
            permissions
        },
        optional_permissions: for_browser(&metadata.optional_permissions, metadata, browser),
        host_permissions: metadata.host_permissions.clone(),
        optional_host_permissions: metadata.optional_host_permissions.clone(),
        icons: metadata.icons.iter().cloned().collect(),
        action: if metadata.has_popup
            || metadata.action.is_some()
            || metadata.side_panel.as_ref().is_some_and(|panel| panel.open_on_action_click)
            || metadata
                .event_handlers
                .iter()
                .any(|handler| handler.namespace == "action")
            || metadata
                .menus
                .iter()
                .any(|item| item.contexts.iter().any(|context| context == "action"))
        {
            let configured = metadata.action.clone().unwrap_or_default();
            Some(Action {
                default_popup: metadata.has_popup.then(|| "popup.html".to_string()),
                default_title: configured.default_title,
                default_icon: configured.default_icon.map(|icon| match icon {
                    ActionIcon::Single(path) => DefaultIcon::Single(path),
                    ActionIcon::Sizes(sizes) => DefaultIcon::Sizes(sizes.into_iter().collect()),
                }),
            })
        } else {
            None
        },
        background: BackgroundConfig {
            service_worker: "background.js".to_string(),
            worker_type: "module".to_string(),
        },
        content_security_policy: ContentSecurityPolicy {
            extension_pages: "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'".to_string(),
        },
        options_ui: if metadata.has_options_page {
            Some(OptionsUi {
                page: "options.html".to_string(),
                open_in_tab: true,
            })
        } else {
            None
        },
        side_panel: match (&metadata.side_panel, browser) {
            (Some(_), Browser::Chromium) => Some(ChromeSidePanel {
                default_path: "sidepanel.html".to_string(),
            }),
            _ => None,
        },
        sidebar_action: match (&metadata.side_panel, browser) {
            (Some(panel), Browser::Firefox) => Some(SidebarAction {
                default_panel: "sidepanel.html".to_string(),
                default_title: panel.title.clone(),
                open_at_install: false,
            }),
            _ => None,
        },
        omnibox: metadata.omnibox.as_ref().map(|omnibox| ManifestOmnibox {
            keyword: omnibox.keyword.clone(),
        }),
        content_scripts: metadata
            .content_scripts
            .iter()
            .map(|cs| ContentScriptEntry {
                matches: cs.matches.clone(),
                js: vec![cs.file_name()],
                run_at: cs.run_at.clone(),
                all_frames: cs.all_frames,
                css: cs.css.clone(),
            })
            .collect(),
        web_accessible_resources: {
            let mut resources = vec![WebAccessibleResource {
                resources: vec!["wasm/*".to_string()],
                matches: vec!["<all_urls>".to_string()],
            }];
            if !metadata.web_accessible.is_empty() {
                resources.push(WebAccessibleResource {
                    resources: metadata.web_accessible.clone(),
                    matches: vec!["<all_urls>".to_string()],
                });
            }
            resources
        },
        declarative_net_request: (!metadata.rule_sets.is_empty()).then(|| DeclarativeNetRequest {
            rule_resources: metadata
                .rule_sets
                .iter()
                .map(|set| RuleResource { id: set.id.clone(), enabled: set.enabled, path: set.path.clone() })
                .collect(),
        }),
        commands: {
            let mut commands: BTreeMap<String, ManifestCommand> = metadata
                .commands
                .iter()
                .map(|command| {
                    (
                        command.name.clone(),
                        ManifestCommand {
                            suggested_key: command.shortcut.clone(),
                            description: Some(command.description.clone()),
                        },
                    )
                })
                .collect();
            if let Some(shortcut) = metadata.action.as_ref().and_then(|action| action.shortcut.clone()) {
                commands.insert(
                    "_execute_action".to_string(),
                    ManifestCommand {
                        suggested_key: Some(shortcut),
                        description: None,
                    },
                );
            }
            commands
        },
    };

    let mut value = serde_json::to_value(&manifest)?;

    if browser == Browser::Firefox {
        let obj = value.as_object_mut().unwrap();

        obj.insert(
            "background".to_string(),
            serde_json::json!({
                "scripts": ["background.js"],
                "type": "module"
            }),
        );

        let gecko_id = metadata
            .gecko_id
            .clone()
            .unwrap_or_else(|| generated_gecko_id(metadata.display_name.as_deref().unwrap_or(name)));
        obj.insert(
            "browser_specific_settings".to_string(),
            serde_json::json!({
                "gecko": {
                    "id": gecko_id
                }
            }),
        );
    }

    let json = serde_json::to_string_pretty(&value)?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_gecko_ids_are_valid() {
        assert_eq!(generated_gecko_id("My Extension"), "my-extension@oxichrome.dev");
        assert_eq!(generated_gecko_id("Café & Tabs!"), "caf-tabs@oxichrome.dev");
        assert_eq!(generated_gecko_id("v1.2 tool_x"), "v1.2-tool_x@oxichrome.dev");
        assert_eq!(generated_gecko_id("日本語"), "extension@oxichrome.dev");
    }

    #[test]
    fn test_icons_and_action_in_manifest() {
        let source = r#"{"kind":"extension","name":"Icons","version":"1.0.0","description":null,"gecko_id":null,"permissions":[],"host_permissions":[],"icons":[[16,"icons/16.png"],[48,"icons/48.png"],[128,"icons/128.png"]],"action":{"default_title":"Focus","default_icon":[[16,"icons/16.png"]]}}
{"kind":"popup","fn_name":"Popup"}"#;
        let metadata = crate::metadata::parse_entries(source).unwrap();
        for browser in [Browser::Chromium, Browser::Firefox] {
            let parsed: serde_json::Value =
                serde_json::from_str(&generate_manifest(&metadata, browser).unwrap()).unwrap();
            assert_eq!(
                parsed["icons"],
                serde_json::json!({ "16": "icons/16.png", "48": "icons/48.png", "128": "icons/128.png" })
            );
            assert_eq!(
                parsed["action"],
                serde_json::json!({
                    "default_popup": "popup.html",
                    "default_title": "Focus",
                    "default_icon": { "16": "icons/16.png" }
                })
            );
        }
    }

    #[test]
    fn side_panel_keys_match_each_browser() {
        let source = r#"{"kind":"extension","name":"P","version":"1.0.0","description":null,"gecko_id":null,"permissions":["storage"],"host_permissions":[],"icons":[],"action":null}
{"kind":"side_panel","fn_name":"Panel","title":"Notes","open_on_action_click":false}"#;
        let metadata = crate::metadata::parse_entries(source).unwrap();
        let chrome: serde_json::Value = serde_json::from_str(&generate_manifest(&metadata, Browser::Chromium).unwrap()).unwrap();
        assert_eq!(chrome["side_panel"], serde_json::json!({"default_path": "sidepanel.html"}));
        assert_eq!(chrome["permissions"], serde_json::json!(["storage", "sidePanel"]));
        assert!(chrome.get("sidebar_action").is_none() && chrome.get("action").is_none());
        let firefox: serde_json::Value = serde_json::from_str(&generate_manifest(&metadata, Browser::Firefox).unwrap()).unwrap();
        assert_eq!(firefox["sidebar_action"], serde_json::json!({"default_panel": "sidepanel.html", "default_title": "Notes", "open_at_install": false}));
        assert_eq!(firefox["permissions"], serde_json::json!(["storage"]));
        assert!(firefox.get("side_panel").is_none() && firefox.get("action").is_none());

        let source = source.replace("\"title\":\"Notes\",\"open_on_action_click\":false", "\"title\":null,\"open_on_action_click\":true").replace("[\"storage\"]", "[\"storage\",\"sidePanel\"]");
        let metadata = crate::metadata::parse_entries(&source).unwrap();
        let chrome: serde_json::Value = serde_json::from_str(&generate_manifest(&metadata, Browser::Chromium).unwrap()).unwrap();
        assert_eq!(chrome["permissions"], serde_json::json!(["storage", "sidePanel"]));
        assert!(chrome.get("action").is_some());
        let firefox: serde_json::Value = serde_json::from_str(&generate_manifest(&metadata, Browser::Firefox).unwrap()).unwrap();
        assert!(firefox.get("action").is_some() && firefox["sidebar_action"].get("default_title").is_none());
    }

    #[test]
    fn test_action_args_alone_create_the_action() {
        let source = r#"{"kind":"extension","name":"A","version":"1.0.0","description":null,"gecko_id":null,"permissions":[],"host_permissions":[],"icons":[],"action":{"default_title":null,"default_icon":"a.png"}}"#;
        let metadata = crate::metadata::parse_entries(source).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&generate_manifest(&metadata, Browser::Chromium).unwrap())
                .unwrap();
        assert_eq!(
            parsed["action"],
            serde_json::json!({ "default_icon": "a.png" })
        );
        assert!(parsed.get("icons").is_none());

        let source = r#"{"kind":"extension","name":"A","version":"1.0.0","description":null,"gecko_id":null,"permissions":[],"host_permissions":[],"icons":[],"action":{"default_title":null,"default_icon":null}}"#;
        let metadata = crate::metadata::parse_entries(source).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&generate_manifest(&metadata, Browser::Chromium).unwrap())
                .unwrap();
        assert_eq!(parsed["action"], serde_json::json!({}));
    }
    use crate::metadata::EventHandler;

    fn test_metadata() -> ExtensionMetadata {
        ExtensionMetadata {
            name: Some("Test Extension".to_string()),
            version: Some("1.0.0".to_string()),
            description: Some("A test extension".to_string()),
            permissions: vec!["storage".to_string(), "tabs".to_string()],
            host_permissions: vec![],
            optional_permissions: vec![],
            optional_host_permissions: vec![],
            chrome_only: vec![],
            firefox_only: vec![],
            background_exports: vec!["__oxichrome_bg_start".to_string()],
            event_handlers: vec![],
            has_popup: true,
            has_options_page: true,
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
            rule_sets: vec![],
            web_accessible: vec![],
        }
    }

    #[test]
    fn test_generate_manifest_chromium() {
        let metadata = test_metadata();
        let json = generate_manifest(&metadata, Browser::Chromium).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["manifest_version"], 3);
        assert_eq!(parsed["name"], "Test Extension");
        assert_eq!(parsed["version"], "1.0.0");
        assert_eq!(parsed["permissions"][0], "storage");
        assert_eq!(parsed["background"]["service_worker"], "background.js");
        assert_eq!(parsed["action"]["default_popup"], "popup.html");
        assert_eq!(parsed["options_ui"]["page"], "options.html");
        assert!(parsed.get("browser_specific_settings").is_none());
    }

    #[test]
    fn test_generate_manifest_with_content_scripts() {
        use crate::metadata::ContentScript;

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
            background_exports: vec![],
            event_handlers: vec![],
            has_popup: false,
            has_options_page: false,
            side_panel: None,
            omnibox: None,
            popup_styles: Vec::new(),
            options_styles: Vec::new(),
            content_scripts: vec![ContentScript {
                fn_name: "inject".to_string(),
                export: "__oxichrome_cs_inject".to_string(),
                matches: vec!["<all_urls>".to_string()],
                run_at: None,
                all_frames: None,
                css: vec![],
            }],
            gecko_id: None,
            display_name: None,
            default_locale: None,
            messages: Vec::new(),
            icons: vec![],
            action: None,
            menus: vec![],
            commands: vec![],
            rule_sets: vec![],
            web_accessible: vec![],
        };
        let json = generate_manifest(&metadata, Browser::Chromium).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["content_scripts"][0]["matches"][0], "<all_urls>");
        assert_eq!(
            parsed["content_scripts"][0]["js"][0],
            "content_script_inject.js"
        );
        assert!(parsed["content_scripts"][0].get("run_at").is_none());
        assert!(parsed["content_scripts"][0].get("all_frames").is_none());
        assert!(parsed["content_scripts"][0].get("css").is_none());
    }

    #[test]
    fn test_generate_manifest_content_scripts_with_options() {
        use crate::metadata::ContentScript;

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
            background_exports: vec![],
            event_handlers: vec![],
            has_popup: false,
            has_options_page: false,
            side_panel: None,
            omnibox: None,
            popup_styles: Vec::new(),
            options_styles: Vec::new(),
            content_scripts: vec![ContentScript {
                fn_name: "inject".to_string(),
                export: "__oxichrome_cs_inject".to_string(),
                matches: vec!["<all_urls>".to_string()],
                run_at: Some("document_start".to_string()),
                all_frames: Some(true),
                css: vec!["styles.css".to_string()],
            }],
            gecko_id: None,
            display_name: None,
            default_locale: None,
            messages: Vec::new(),
            icons: vec![],
            action: None,
            menus: vec![],
            commands: vec![],
            rule_sets: vec![],
            web_accessible: vec![],
        };
        let json = generate_manifest(&metadata, Browser::Chromium).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["content_scripts"][0]["run_at"], "document_start");
        assert_eq!(parsed["content_scripts"][0]["all_frames"], true);
        assert_eq!(parsed["content_scripts"][0]["css"][0], "styles.css");
    }

    #[test]
    fn test_generate_manifest_firefox() {
        let metadata = test_metadata();
        let json = generate_manifest(&metadata, Browser::Firefox).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["manifest_version"], 3);
        assert_eq!(parsed["background"]["scripts"][0], "background.js");
        assert_eq!(parsed["background"]["type"], "module");
        assert!(parsed["background"].get("service_worker").is_none());
        assert_eq!(
            parsed["content_security_policy"]["extension_pages"],
            "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'"
        );
        assert_eq!(
            parsed["browser_specific_settings"]["gecko"]["id"],
            "test-extension@oxichrome.dev"
        );
    }

    #[test]
    fn test_generate_manifest_with_host_permissions() {
        let metadata = ExtensionMetadata {
            name: Some("Test".to_string()),
            version: Some("1.0.0".to_string()),
            description: None,
            gecko_id: None,
            display_name: None,
            default_locale: None,
            messages: Vec::new(),
            icons: vec![],
            action: None,
            menus: vec![],
            commands: vec![],
            rule_sets: vec![],
            web_accessible: vec![],
            permissions: vec!["storage".to_string()],
            host_permissions: vec![
                "*://developer.mozilla.org/*".to_string(),
                "*://*.example.org/*".to_string(),
            ],
            optional_permissions: vec![],
            optional_host_permissions: vec![],
            chrome_only: vec![],
            firefox_only: vec![],
            background_exports: vec![],
            event_handlers: vec![],
            has_popup: false,
            has_options_page: false,
            side_panel: None,
            omnibox: None,
            popup_styles: Vec::new(),
            options_styles: Vec::new(),
            content_scripts: vec![],
        };
        let json = generate_manifest(&metadata, Browser::Chromium).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["host_permissions"][0], "*://developer.mozilla.org/*");
        assert_eq!(parsed["host_permissions"][1], "*://*.example.org/*");
        assert_eq!(parsed["permissions"][0], "storage");
    }

    #[test]
    fn rule_sets_and_web_accessible_files_reach_both_manifests() {
        let text = r#"{"kind":"extension","name":"R","version":"1.0.0","description":null,"gecko_id":"r@x.y","permissions":["declarativeNetRequest"],"host_permissions":[],"icons":[],"action":null,"rule_sets":[{"id":"ads","path":"rules/ads.json","enabled":true},{"id":"beta","path":"rules/beta.json","enabled":false}],"web_accessible":["blocked.html","pages/*"]}"#;
        let metadata = crate::metadata::parse_entries(text).unwrap();
        for browser in [Browser::Chromium, Browser::Firefox] {
            let parsed: serde_json::Value = serde_json::from_str(&generate_manifest(&metadata, browser).unwrap()).unwrap();
            assert_eq!(
                parsed["declarative_net_request"],
                serde_json::json!({"rule_resources": [{"id": "ads", "enabled": true, "path": "rules/ads.json"}, {"id": "beta", "enabled": false, "path": "rules/beta.json"}]})
            );
            assert_eq!(
                parsed["web_accessible_resources"],
                serde_json::json!([{"resources": ["wasm/*"], "matches": ["<all_urls>"]}, {"resources": ["blocked.html", "pages/*"], "matches": ["<all_urls>"]}])
            );
        }
        let plain = crate::metadata::parse_entries(r#"{"kind":"extension","name":"P","version":"1.0.0","description":null,"gecko_id":null,"permissions":[],"host_permissions":[],"icons":[],"action":null}"#).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&generate_manifest(&plain, Browser::Chromium).unwrap()).unwrap();
        assert!(parsed.get("declarative_net_request").is_none());
        assert_eq!(parsed["web_accessible_resources"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_generate_manifest_without_host_permissions() {
        let metadata = test_metadata();
        let json = generate_manifest(&metadata, Browser::Chromium).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("host_permissions").is_none());
    }

    #[test]
    fn test_generate_manifest_firefox_custom_gecko_id() {
        let mut metadata = test_metadata();
        metadata.gecko_id = Some("my-ext@example.com".to_string());
        let json = generate_manifest(&metadata, Browser::Firefox).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(
            parsed["browser_specific_settings"]["gecko"]["id"],
            "my-ext@example.com"
        );
    }

    #[test]
    fn test_action_declared_for_on_clicked_without_popup() {
        let mut metadata = test_metadata();
        metadata.has_popup = false;
        metadata.event_handlers = vec![EventHandler {
            fn_name: "clicked".to_string(),
            export: "__oxichrome_on_clicked".to_string(),
            namespace: "action".to_string(),
            event_name: "on_clicked".to_string(),
        }];
        let json = generate_manifest(&metadata, Browser::Chromium).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["action"], serde_json::json!({}));
    }

    #[test]
    fn test_action_omitted_without_popup_or_handler() {
        let mut metadata = test_metadata();
        metadata.has_popup = false;
        metadata.event_handlers = vec![];
        let json = generate_manifest(&metadata, Browser::Chromium).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("action").is_none());
    }

    #[test]
    fn test_menus_add_their_permission_and_toolbar_button() {
        let mut metadata = test_metadata();
        metadata.has_popup = false;
        metadata.menus = vec![crate::metadata::MenuItem {
            id: "a".to_string(),
            handler: Some("a".to_string()),
            export: Some("__oxichrome_menu_a".to_string()),
            item_type: "normal".to_string(),
            title: "T & a".to_string(),
            contexts: vec!["page".to_string()],
            checked: None,
            parent: None,
            enabled: None,
            visible: None,
            document_url_patterns: vec![],
            target_url_patterns: vec![],
        }];
        let parsed: serde_json::Value =
            serde_json::from_str(&generate_manifest(&metadata, Browser::Firefox).unwrap()).unwrap();
        assert_eq!(parsed["permissions"], serde_json::json!(["storage", "tabs", "contextMenus"]));
        assert!(parsed.get("action").is_none());

        metadata.permissions.push("contextMenus".to_string());
        metadata.menus[0].contexts = vec!["page".to_string(), "action".to_string()];
        let parsed: serde_json::Value =
            serde_json::from_str(&generate_manifest(&metadata, Browser::Chromium).unwrap()).unwrap();
        assert_eq!(parsed["permissions"], serde_json::json!(["storage", "tabs", "contextMenus"]));
        assert_eq!(parsed["action"], serde_json::json!({}));
    }

    #[test]
    fn test_commands_and_action_shortcut_in_manifest() {
        let mut metadata = test_metadata();
        let shortcut = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(platform, key)| (platform.to_string(), key.to_string()))
                .collect::<crate::metadata::Shortcut>()
        };
        metadata.commands = vec![
            crate::metadata::Command {
                name: "toggle".to_string(),
                export: "__oxichrome_command_toggle_0".to_string(),
                description: "Toggle".to_string(),
                shortcut: Some(shortcut(&[("default", "Alt+Shift+K"), ("mac", "MacCtrl+Shift+K")])),
            },
            crate::metadata::Command {
                name: "later".to_string(),
                export: "__oxichrome_command_later_0".to_string(),
                description: "Later".to_string(),
                shortcut: None,
            },
        ];
        metadata.action = Some(crate::metadata::ActionMetadata {
            default_title: None,
            default_icon: None,
            shortcut: Some(shortcut(&[("default", "Alt+Shift+O")])),
        });
        for browser in [Browser::Chromium, Browser::Firefox] {
            let parsed: serde_json::Value =
                serde_json::from_str(&generate_manifest(&metadata, browser).unwrap()).unwrap();
            assert_eq!(
                parsed["commands"],
                serde_json::json!({
                    "_execute_action": { "suggested_key": { "default": "Alt+Shift+O" } },
                    "later": { "description": "Later" },
                    "toggle": {
                        "description": "Toggle",
                        "suggested_key": { "default": "Alt+Shift+K", "mac": "MacCtrl+Shift+K" }
                    }
                })
            );
            assert!(parsed.get("action").is_some());
        }
        metadata.commands.clear();
        metadata.action = None;
        metadata.has_popup = false;
        let parsed: serde_json::Value =
            serde_json::from_str(&generate_manifest(&metadata, Browser::Chromium).unwrap()).unwrap();
        assert!(parsed.get("commands").is_none());
    }
}
