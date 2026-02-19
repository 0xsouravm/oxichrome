use serde::Serialize;

use crate::Browser;
use crate::source_parser::ExtensionMetadata;

#[derive(Serialize)]
struct Manifest {
    manifest_version: u32,
    name: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    permissions: Vec<String>,
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
    default_popup: String,
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
        action: if metadata.has_popup {
            Some(Action {
                default_popup: "popup.html".to_string(),
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
        content_scripts: metadata.content_scripts.iter().map(|cs| {
            ContentScriptEntry {
                matches: cs.matches.clone(),
                js: vec![format!("content_script_{}.js", cs.fn_name)],
            }
        }).collect(),
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

        let gecko_id = format!("{}@oxichrome.dev", name.to_lowercase().replace(' ', "-"));
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

    fn test_metadata() -> ExtensionMetadata {
        ExtensionMetadata {
            name: Some("Test Extension".to_string()),
            version: Some("1.0.0".to_string()),
            description: Some("A test extension".to_string()),
            permissions: vec!["storage".to_string(), "tabs".to_string()],
            background_functions: vec!["start".to_string()],
            event_handlers: vec![],
            has_popup: true,
            has_options_page: true,
            content_scripts: vec![],
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
            background_functions: vec![],
            event_handlers: vec![],
            has_popup: false,
            has_options_page: false,
            content_scripts: vec![
                ContentScript {
                    fn_name: "inject".to_string(),
                    matches: vec!["<all_urls>".to_string()],
                },
            ],
        };
        let json = generate_manifest(&metadata, Browser::Chromium).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["content_scripts"][0]["matches"][0], "<all_urls>");
        assert_eq!(parsed["content_scripts"][0]["js"][0], "content_script_inject.js");
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
}
