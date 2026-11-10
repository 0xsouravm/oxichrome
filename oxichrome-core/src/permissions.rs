use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

macro_rules! permissions {
    ($($(#[$doc:meta])* $variant:ident = $name:literal,)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[non_exhaustive]
        pub enum Permission {
            $($(#[$doc])* $variant,)*
        }

        impl Permission {
            pub const ALL: &'static [Permission] = &[$(Permission::$variant,)*];

            pub fn name(self) -> &'static str {
                match self {
                    $(Permission::$variant => $name,)*
                }
            }

            pub fn from_name(name: &str) -> Option<Permission> {
                match name {
                    $($name => Some(Permission::$variant),)*
                    "menus" => Some(Permission::ContextMenus),
                    _ => None,
                }
            }
        }
    };
}

permissions! {
    AccessibilityFeaturesModify = "accessibilityFeatures.modify",
    AccessibilityFeaturesRead = "accessibilityFeatures.read",
    ActiveTab = "activeTab",
    Alarms = "alarms",
    Audio = "audio",
    Background = "background",
    Bookmarks = "bookmarks",
    BrowserSettings = "browserSettings",
    BrowsingData = "browsingData",
    CaptivePortal = "captivePortal",
    CertificateProvider = "certificateProvider",
    ClipboardRead = "clipboardRead",
    ClipboardWrite = "clipboardWrite",
    ContentSettings = "contentSettings",
    ContextMenus = "contextMenus",
    ContextualIdentities = "contextualIdentities",
    Cookies = "cookies",
    Debugger = "debugger",
    DeclarativeContent = "declarativeContent",
    DeclarativeNetRequest = "declarativeNetRequest",
    DeclarativeNetRequestFeedback = "declarativeNetRequestFeedback",
    DeclarativeNetRequestWithHostAccess = "declarativeNetRequestWithHostAccess",
    DesktopCapture = "desktopCapture",
    Devtools = "devtools",
    Dns = "dns",
    DocumentScan = "documentScan",
    Downloads = "downloads",
    DownloadsOpen = "downloads.open",
    DownloadsUi = "downloads.ui",
    EnterpriseDeviceAttributes = "enterprise.deviceAttributes",
    EnterpriseHardwarePlatform = "enterprise.hardwarePlatform",
    EnterpriseNetworkingAttributes = "enterprise.networkingAttributes",
    EnterprisePlatformKeys = "enterprise.platformKeys",
    Favicon = "favicon",
    FileBrowserHandler = "fileBrowserHandler",
    FileSystemProvider = "fileSystemProvider",
    Find = "find",
    FontSettings = "fontSettings",
    Gcm = "gcm",
    Geolocation = "geolocation",
    History = "history",
    Identity = "identity",
    IdentityEmail = "identity.email",
    Idle = "idle",
    LoginState = "loginState",
    Management = "management",
    MenusOverrideContext = "menus.overrideContext",
    NativeMessaging = "nativeMessaging",
    Notifications = "notifications",
    Offscreen = "offscreen",
    PageCapture = "pageCapture",
    Pkcs11 = "pkcs11",
    PlatformKeys = "platformKeys",
    Power = "power",
    PrinterProvider = "printerProvider",
    Printing = "printing",
    PrintingMetrics = "printingMetrics",
    Privacy = "privacy",
    Processes = "processes",
    Proxy = "proxy",
    ReadingList = "readingList",
    Scripting = "scripting",
    Search = "search",
    Sessions = "sessions",
    SidePanel = "sidePanel",
    Storage = "storage",
    SystemCpu = "system.cpu",
    SystemDisplay = "system.display",
    SystemMemory = "system.memory",
    SystemStorage = "system.storage",
    TabCapture = "tabCapture",
    TabGroups = "tabGroups",
    TabHide = "tabHide",
    Tabs = "tabs",
    Theme = "theme",
    TopSites = "topSites",
    TrialMl = "trialML",
    Tts = "tts",
    TtsEngine = "ttsEngine",
    UnlimitedStorage = "unlimitedStorage",
    UserScripts = "userScripts",
    VpnProvider = "vpnProvider",
    Wallpaper = "wallpaper",
    WebAuthenticationProxy = "webAuthenticationProxy",
    WebNavigation = "webNavigation",
    WebRequest = "webRequest",
    WebRequestAuthProvider = "webRequestAuthProvider",
    WebRequestBlocking = "webRequestBlocking",
    WebRequestFilterResponse = "webRequestFilterResponse",
    WebRequestFilterResponseServiceWorkerScript = "webRequestFilterResponse.serviceWorkerScript",
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl Serialize for Permission {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Permission {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Permission::from_name(&name)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown permission `{name}`")))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PermissionSet {
    pub permissions: Vec<Permission>,
    pub origins: Vec<String>,
}

#[derive(Deserialize)]
struct RawSet {
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default)]
    origins: Vec<String>,
}

impl<'de> Deserialize<'de> for PermissionSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let raw = RawSet::deserialize(deserializer)?;
        let mut permissions: Vec<Permission> = raw
            .permissions
            .iter()
            .filter_map(|name| Permission::from_name(name))
            .collect();
        permissions.sort();
        permissions.dedup();
        let mut origins: Vec<String> = raw
            .origins
            .into_iter()
            .filter(|origin| !origin.starts_with("moz-extension:") && !origin.starts_with("chrome-extension:"))
            .collect();
        origins.sort();
        origins.dedup();
        Ok(PermissionSet { permissions, origins })
    }
}

impl PermissionSet {
    pub fn new() -> Self {
        PermissionSet::default()
    }

    pub fn permission(mut self, permission: Permission) -> Self {
        if !self.permissions.contains(&permission) {
            self.permissions.push(permission);
        }
        self
    }

    pub fn origin(mut self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        if !self.origins.contains(&pattern) {
            self.origins.push(pattern);
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        self.permissions.is_empty() && self.origins.is_empty()
    }
}

impl From<Permission> for PermissionSet {
    fn from(permission: Permission) -> Self {
        PermissionSet::new().permission(permission)
    }
}

impl<const N: usize> From<[Permission; N]> for PermissionSet {
    fn from(permissions: [Permission; N]) -> Self {
        permissions
            .into_iter()
            .fold(PermissionSet::new(), PermissionSet::permission)
    }
}

impl FromEventArg for PermissionSet {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        crate::json::from_js(&value)
    }
}

impl EventArg<events::PermissionsOnAdded, 1> for PermissionSet {}
impl EventArg<events::PermissionsOnRemoved, 1> for PermissionSet {}

fn js_error(message: String) -> OxichromeError {
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    if message.starts_with("[oxichrome]") {
        return OxichromeError::InvalidArgument(message.trim_start_matches("[oxichrome] ").to_string());
    }
    let lower = message.to_ascii_lowercase();
    if lower.contains("not a recognized permission") {
        return OxichromeError::InvalidArgument(format!(
            "{message} (this browser doesn't have it; the build leaves it out of this browser's manifest)"
        ));
    }
    if lower.contains("user gesture") || lower.contains("user input handler") {
        return OxichromeError::InvalidArgument(format!(
            "{message} (ask from the handler `permissions::request_on_click` returns, so the browser sees the click)"
        ));
    }
    if lower.contains("required permission") {
        return OxichromeError::InvalidArgument(format!(
            "{message} (only permissions from `optional_permissions` and `optional_host_permissions` can be removed)"
        ));
    }
    if lower.contains("origin") && (lower.contains("not declared") || lower.contains("only permissions specified")) {
        return OxichromeError::InvalidArgument(format!(
            "{message} (only sites covered by `optional_host_permissions` in #[oxichrome::extension] can be asked for)"
        ));
    }
    if lower.contains("only permissions specified in the manifest") || lower.contains("not declared in the manifest") || lower.contains("must be listed") {
        return OxichromeError::InvalidArgument(format!(
            "{message} (add it to `optional_permissions` or `optional_host_permissions` in #[oxichrome::extension])"
        ));
    }
    js_error(message)
}

#[derive(Deserialize)]
struct Declared {
    permissions: Vec<String>,
    optional: Vec<String>,
    hosts: Vec<String>,
    optional_hosts: Vec<String>,
}

fn split_pattern(pattern: &str) -> Option<(&str, &str, &str)> {
    let (scheme, rest) = pattern.split_once("://")?;
    let (host, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    let host = host.split(':').next().unwrap_or(host);
    Some((scheme, host, path))
}

fn glob(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, tail)) => {
            text.starts_with(head)
                && (head.len()..=text.len()).any(|start| text.is_char_boundary(start) && glob(tail, &text[start..]))
        }
    }
}

fn covers(declared: &str, wanted: &str) -> bool {
    if declared == "<all_urls>" {
        return true;
    }
    if wanted == "<all_urls>" {
        return false;
    }
    let (Some((d_scheme, d_host, d_path)), Some((w_scheme, w_host, w_path))) = (split_pattern(declared), split_pattern(wanted)) else {
        return false;
    };
    let scheme = d_scheme == w_scheme || (d_scheme == "*" && matches!(w_scheme, "http" | "https" | "ws" | "wss"));
    let host = d_host == "*"
        || d_host == w_host
        || d_host.strip_prefix("*.").is_some_and(|base| {
            w_host == base || w_host.ends_with(&format!(".{base}")) || w_host.strip_prefix("*.").is_some_and(|w| w == base || w.ends_with(&format!(".{base}")))
        });
    scheme && host && glob(d_path, w_path)
}

enum Kind {
    Required,
    Optional,
    Missing,
}

impl Declared {
    fn read() -> Result<Declared> {
        crate::json::from_js(&js_bridge::permissions_declared())
    }

    fn kind(&self, permission: Permission) -> Kind {
        let listed = |names: &[String]| names.iter().any(|name| Permission::from_name(name) == Some(permission));
        if listed(&self.permissions) {
            Kind::Required
        } else if listed(&self.optional) {
            Kind::Optional
        } else {
            Kind::Missing
        }
    }

    fn missing(&self, permission: Permission) -> OxichromeError {
        OxichromeError::InvalidArgument(format!(
            "`{permission:?}` isn't in `optional_permissions` in #[oxichrome::extension], or this browser doesn't have it (the build leaves out permissions a browser doesn't know)"
        ))
    }

    fn check_origins(&self, set: &PermissionSet) -> Result<()> {
        for origin in &set.origins {
            if !self.hosts.iter().chain(&self.optional_hosts).any(|declared| covers(declared, origin)) {
                return Err(OxichromeError::InvalidArgument(format!(
                    "`{origin}` isn't covered by `optional_host_permissions` in #[oxichrome::extension]; only sites listed there can be asked for"
                )));
            }
        }
        Ok(())
    }

    fn optional_part(&self, set: &PermissionSet) -> Result<PermissionSet> {
        self.check_origins(set)?;
        let mut part = PermissionSet { permissions: Vec::new(), origins: set.origins.clone() };
        for permission in &set.permissions {
            match self.kind(*permission) {
                Kind::Required => {}
                Kind::Optional => part.permissions.push(*permission),
                Kind::Missing => return Err(self.missing(*permission)),
            }
        }
        Ok(part)
    }
}

fn set_to_js(set: &PermissionSet) -> Result<JsValue> {
    crate::json::to_js(set)
}

async fn settle(promise: js_sys::Promise) -> Result<bool> {
    let value = JsFuture::from(promise).await.map_err(browser_error)?;
    Ok(value.as_bool() == Some(true))
}

pub fn request_on_click<E, F, Fut>(set: impl Into<PermissionSet>, then: F) -> impl FnMut(E) + 'static
where
    E: AsRef<web_sys::Event>,
    F: FnMut(Result<bool>) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    let set = set.into();
    let then = Rc::new(RefCell::new(then));
    move |event: E| {
        let started: Result<Option<js_sys::Promise>> = if !event.as_ref().is_trusted() {
            Err(OxichromeError::InvalidArgument(
                "only a real click or key press can ask for permissions; this event was created by code".to_string(),
            ))
        } else if set.is_empty() {
            Err(OxichromeError::InvalidArgument(
                "the set is empty; add a permission or an origin to ask for".to_string(),
            ))
        } else {
            Declared::read()
                .and_then(|declared| declared.optional_part(&set))
                .and_then(|part| {
                    if part.is_empty() {
                        return Ok(None);
                    }
                    let js = set_to_js(&part)?;
                    js_bridge::permissions_request(&js).map(Some).map_err(browser_error)
                })
        };
        let then = then.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = match started {
                Ok(Some(promise)) => settle(promise).await,
                Ok(None) => Ok(true),
                Err(err) => Err(err),
            };
            let future = (then.borrow_mut())(result);
            future.await;
        });
    }
}

pub async fn contains(set: impl Into<PermissionSet>) -> Result<bool> {
    let set = set.into();
    let declared = Declared::read()?;
    let mut part = PermissionSet { permissions: Vec::new(), origins: set.origins.clone() };
    for permission in &set.permissions {
        match declared.kind(*permission) {
            Kind::Required => {}
            Kind::Optional => part.permissions.push(*permission),
            Kind::Missing => return Ok(false),
        }
    }
    if part.is_empty() {
        return Ok(true);
    }
    let js = set_to_js(&part)?;
    settle(js_bridge::permissions_contains(&js).map_err(browser_error)?).await
}

pub async fn remove(set: impl Into<PermissionSet>) -> Result<bool> {
    let set = set.into();
    let declared = Declared::read()?;
    for permission in &set.permissions {
        match declared.kind(*permission) {
            Kind::Optional => {}
            Kind::Required => {
                return Err(OxichromeError::InvalidArgument(format!(
                    "`{permission:?}` is in `permissions`, so it stays granted; only `optional_permissions` can be removed"
                )))
            }
            Kind::Missing => return Err(declared.missing(*permission)),
        }
    }
    if let Some(origin) = set.origins.iter().find(|origin| declared.hosts.contains(origin)) {
        return Err(OxichromeError::InvalidArgument(format!(
            "`{origin}` is in `host_permissions`, so it stays granted; only `optional_host_permissions` can be removed"
        )));
    }
    if set.is_empty() {
        return Ok(false);
    }
    let js = set_to_js(&set)?;
    settle(js_bridge::permissions_remove(&js).map_err(browser_error)?).await
}

pub async fn get_all() -> Result<PermissionSet> {
    let value = JsFuture::from(js_bridge::permissions_get_all().map_err(browser_error)?)
        .await
        .map_err(browser_error)?;
    crate::json::from_js(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_coverage() {
        assert!(covers("<all_urls>", "https://a.com/*"));
        assert!(covers("*://a.com/*", "https://a.com/*"));
        assert!(covers("https://*.a.com/*", "https://a.com/*"));
        assert!(covers("https://*.a.com/*", "https://x.y.a.com/path*"));
        assert!(covers("https://*.a.com/*", "https://*.b.a.com/*"));
        assert!(covers("http://127.0.0.1/*", "http://127.0.0.1/*"));
        assert!(covers("https://a.com/docs/*", "https://a.com/docs/x"));
        assert!(!covers("https://a.com/docs/*", "https://a.com/*"));
        assert!(!covers("https://a.com/*", "https://b.com/*"));
        assert!(!covers("https://a.com/*", "http://a.com/*"));
        assert!(!covers("*://*/*", "<all_urls>"));
        assert!(!covers("https://*.a.com/*", "https://ba.com/*"));
    }

    #[test]
    fn names_round_trip() {
        for permission in Permission::ALL {
            assert_eq!(Permission::from_name(permission.name()), Some(*permission));
        }
        assert_eq!(Permission::from_name("menus"), Some(Permission::ContextMenus));
        assert_eq!(Permission::from_name("tab"), None);
    }

    #[test]
    fn sets_skip_unknown_names_and_extension_origins() {
        let set: PermissionSet = serde_json::from_str(
            r#"{"permissions":["tabs","internal:svgContextPropertiesAllowed","menus","contextMenus"],"origins":["moz-extension://x/*","https://a.com/*"]}"#,
        )
        .unwrap();
        assert_eq!(set.permissions, [Permission::ContextMenus, Permission::Tabs]);
        assert_eq!(set.origins, ["https://a.com/*"]);
        let built = PermissionSet::from([Permission::History, Permission::History]).origin("https://a.com/*");
        assert_eq!(serde_json::to_string(&built).unwrap(), r#"{"permissions":["history"],"origins":["https://a.com/*"]}"#);
    }
}
