use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;
use crate::permissions::Permission;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub may_disable: bool,
    pub install_type: InstallType,
    pub options_url: Option<String>,
    pub permissions: Vec<Permission>,
    pub host_permissions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstallType {
    Admin,
    Development,
    Normal,
    Sideload,
    Other,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSelf {
    id: String,
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    enabled: Option<bool>,
    may_disable: Option<bool>,
    install_type: Option<String>,
    options_url: Option<String>,
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default)]
    host_permissions: Vec<String>,
}

impl RawSelf {
    fn info(self) -> SelfInfo {
        let install_type = match self.install_type.as_deref() {
            Some("admin") => InstallType::Admin,
            Some("development") => InstallType::Development,
            Some("normal") => InstallType::Normal,
            Some("sideload") => InstallType::Sideload,
            _ => InstallType::Other,
        };
        let mut permissions: Vec<Permission> = Vec::new();
        for name in &self.permissions {
            let permission = Permission::custom(name);
            if !permissions.contains(&permission) {
                permissions.push(permission);
            }
        }
        SelfInfo {
            name: self.name.unwrap_or_default(),
            description: self.description.unwrap_or_default(),
            version: self.version.unwrap_or_default(),
            enabled: self.enabled.unwrap_or(true),
            may_disable: self.may_disable.unwrap_or(true),
            install_type,
            options_url: self.options_url.filter(|url| !url.is_empty()),
            permissions,
            host_permissions: self
                .host_permissions
                .into_iter()
                .filter(|pattern| !pattern.starts_with("moz-extension://") && !pattern.starts_with("chrome-extension://"))
                .collect(),
            id: self.id,
        }
    }
}

fn error_text(error: &JsValue) -> String {
    js_sys::Reflect::get(error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

fn browser_error(error: JsValue) -> OxichromeError {
    OxichromeError::Js(JsError::new(JsValue::from_str(&error_text(&error))))
}

pub async fn get_self() -> Result<SelfInfo> {
    let value = JsFuture::from(js_bridge::management_get_self().map_err(browser_error)?).await.map_err(browser_error)?;
    Ok(crate::json::from_js::<RawSelf>(&value)?.info())
}

pub async fn uninstall_self() -> Result<()> {
    JsFuture::from(js_bridge::management_uninstall_self(false).map_err(browser_error)?).await.map_err(browser_error)?;
    Ok(())
}

pub fn uninstall_on_click<E, F, Fut>(then: F) -> impl FnMut(E) + 'static
where
    E: AsRef<web_sys::Event>,
    F: FnMut(Result<bool>) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    let then = Rc::new(RefCell::new(then));
    move |event: E| {
        let started: Result<js_sys::Promise> = if event.as_ref().is_trusted() {
            js_bridge::management_uninstall_self(true).map_err(browser_error)
        } else {
            Err(OxichromeError::InvalidArgument(
                "only a real click or key press can ask to uninstall; this event was created by code".to_string(),
            ))
        };
        let then = then.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = match started {
                Ok(promise) => match JsFuture::from(promise).await {
                    Ok(_) => Ok(true),
                    Err(error) if error_text(&error).to_ascii_lowercase().contains("cancel") => Ok(false),
                    Err(error) => Err(browser_error(error)),
                },
                Err(error) => Err(error),
            };
            let future = (then.borrow_mut())(result);
            future.await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_and_firefox_read_the_same() {
        let chrome: RawSelf = serde_json::from_str(r#"{"description":"","enabled":true,"homepageUrl":"","hostPermissions":["https://a.example/*"],"id":"gck","installType":"development","isApp":false,"mayDisable":true,"name":"P","offlineEnabled":false,"optionsUrl":"","permissions":["history","tabs","newThing"],"shortName":"P","type":"extension","version":"1.0","versionName":""}"#).unwrap();
        let firefox: RawSelf = serde_json::from_str(r#"{"id":"p@example.com","name":"P","description":"","version":"1.0","mayDisable":true,"enabled":true,"optionsUrl":"moz-extension://u/options.html","installType":"normal","type":"extension","permissions":["tabs","history"],"hostPermissions":["moz-extension://u/*","https://a.example/*"],"shortName":""}"#).unwrap();
        let chrome = chrome.info();
        let firefox = firefox.info();
        assert_eq!(chrome.options_url, None);
        assert_eq!(firefox.options_url.as_deref(), Some("moz-extension://u/options.html"));
        assert_eq!(chrome.host_permissions, firefox.host_permissions);
        assert_eq!(chrome.install_type, InstallType::Development);
        assert_eq!(firefox.install_type, InstallType::Normal);
        assert_eq!(chrome.permissions, [Permission::History, Permission::Tabs, Permission::Custom("newThing")]);
        assert_eq!(firefox.permissions, [Permission::Tabs, Permission::History]);
    }
}
