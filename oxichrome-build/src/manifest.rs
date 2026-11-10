use serde::Serialize;

use crate::source_parser::{ActionIcon, ExtensionMetadata};
use crate::Browser;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct Manifest {
    manifest_version: u32,
    name: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    permissions: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    host_permissions: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    icons: BTreeMap<u32, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<Action>,
    background: BackgroundConfig,
    content_security_policy: ContentSecurityPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    options_ui: Option<OptionsUi>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    content_scripts: Vec<ContentScriptEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    web_accessible_resources: Vec<WebAccessibleResource>,
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
        permissions: metadata.permissions.clone(),
        host_permissions: metadata.host_permissions.clone(),
        icons: metadata.icons.iter().cloned().collect(),
        action: if metadata.has_popup
            || metadata.action.is_some()
            || metadata
                .event_handlers
                .iter()
                .any(|handler| handler.namespace == "action")
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
        content_scripts: metadata
            .content_scripts
            .iter()
            .map(|cs| ContentScriptEntry {
                matches: cs.matches.clone(),
                js: vec![format!("content_script_{}.js", cs.fn_name)],
                run_at: cs.run_at.clone(),
                all_frames: cs.all_frames,
                css: cs.css.clone(),
            })
            .collect(),
        web_accessible_resources: vec![WebAccessibleResource {
            resources: vec!["wasm/*".to_string()],
            matches: vec!["<all_urls>".to_string()],
        }],
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
            .unwrap_or_else(|| format!("{}@oxichrome.dev", name.to_lowercase().replace(' ', "-")));
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
    fn test_icons_and_action_in_manifest() {
        let source = r#"
            #[oxichrome::extension(
                name = "Icons", version = "1.0.0",
                icons = { 128 = "icons/128.png", 16 = "icons/16.png", 48 = "icons/48.png" },
                action = { default_title = "Focus", default_icon = { 16 = "icons/16.png" } },
            )]
            struct Ext;

            #[oxichrome::popup]
            fn Popup() {}
        "#;
        let metadata = crate::source_parser::parse_source_str(source).unwrap();
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
    fn test_action_args_alone_create_the_action() {
        let source = r#"
            #[oxichrome::extension(name = "A", version = "1.0.0", action = { default_icon = "a.png" })]
            struct Ext;
        "#;
        let metadata = crate::source_parser::parse_source_str(source).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&generate_manifest(&metadata, Browser::Chromium).unwrap())
                .unwrap();
        assert_eq!(
            parsed["action"],
            serde_json::json!({ "default_icon": "a.png" })
        );
        assert!(parsed.get("icons").is_none());

        let source = r#"
            #[oxichrome::extension(name = "A", version = "1.0.0", action = {})]
            struct Ext;
        "#;
        let metadata = crate::source_parser::parse_source_str(source).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&generate_manifest(&metadata, Browser::Chromium).unwrap())
                .unwrap();
        assert_eq!(parsed["action"], serde_json::json!({}));
    }
    use crate::source_parser::EventHandler;

    fn test_metadata() -> ExtensionMetadata {
        ExtensionMetadata {
            name: Some("Test Extension".to_string()),
            version: Some("1.0.0".to_string()),
            description: Some("A test extension".to_string()),
            permissions: vec!["storage".to_string(), "tabs".to_string()],
            host_permissions: vec![],
            background_functions: vec!["start".to_string()],
            event_handlers: vec![],
            has_popup: true,
            has_options_page: true,
            content_scripts: vec![],
            gecko_id: None,
            icons: vec![],
            action: None,
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
        use crate::source_parser::ContentScript;

        let metadata = ExtensionMetadata {
            name: Some("Test".to_string()),
            version: Some("1.0.0".to_string()),
            description: None,
            permissions: vec![],
            host_permissions: vec![],
            background_functions: vec![],
            event_handlers: vec![],
            has_popup: false,
            has_options_page: false,
            content_scripts: vec![ContentScript {
                fn_name: "inject".to_string(),
                matches: vec!["<all_urls>".to_string()],
                run_at: None,
                all_frames: None,
                css: vec![],
            }],
            gecko_id: None,
            icons: vec![],
            action: None,
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
        use crate::source_parser::ContentScript;

        let metadata = ExtensionMetadata {
            name: Some("Test".to_string()),
            version: Some("1.0.0".to_string()),
            description: None,
            permissions: vec![],
            host_permissions: vec![],
            background_functions: vec![],
            event_handlers: vec![],
            has_popup: false,
            has_options_page: false,
            content_scripts: vec![ContentScript {
                fn_name: "inject".to_string(),
                matches: vec!["<all_urls>".to_string()],
                run_at: Some("document_start".to_string()),
                all_frames: Some(true),
                css: vec!["styles.css".to_string()],
            }],
            gecko_id: None,
            icons: vec![],
            action: None,
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
            icons: vec![],
            action: None,
            permissions: vec!["storage".to_string()],
            host_permissions: vec![
                "*://developer.mozilla.org/*".to_string(),
                "*://*.example.org/*".to_string(),
            ],
            background_functions: vec![],
            event_handlers: vec![],
            has_popup: false,
            has_options_page: false,
            content_scripts: vec![],
        };
        let json = generate_manifest(&metadata, Browser::Chromium).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["host_permissions"][0], "*://developer.mozilla.org/*");
        assert_eq!(parsed["host_permissions"][1], "*://*.example.org/*");
        assert_eq!(parsed["permissions"][0], "storage");
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
}
