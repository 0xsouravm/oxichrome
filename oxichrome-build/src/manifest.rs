use serde::Serialize;

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
struct WebAccessibleResource {
    resources: Vec<String>,
    matches: Vec<String>,
}

pub fn generate_manifest(metadata: &ExtensionMetadata) -> anyhow::Result<String> {
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
        web_accessible_resources: vec![WebAccessibleResource {
            resources: vec!["wasm/*".to_string()],
            matches: vec!["<all_urls>".to_string()],
        }],
    };

    let json = serde_json::to_string_pretty(&manifest)?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_manifest() {
        let metadata = ExtensionMetadata {
            name: Some("Test Extension".to_string()),
            version: Some("1.0.0".to_string()),
            description: Some("A test extension".to_string()),
            permissions: vec!["storage".to_string(), "tabs".to_string()],
            background_functions: vec!["start".to_string()],
            event_handlers: vec![],
            has_popup: true,
            has_options_page: true,
        };

        let json = generate_manifest(&metadata).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["manifest_version"], 3);
        assert_eq!(parsed["name"], "Test Extension");
        assert_eq!(parsed["version"], "1.0.0");
        assert_eq!(parsed["permissions"][0], "storage");
        assert_eq!(parsed["background"]["service_worker"], "background.js");
        assert_eq!(parsed["action"]["default_popup"], "popup.html");
        assert_eq!(parsed["options_ui"]["page"], "options.html");
    }
}
