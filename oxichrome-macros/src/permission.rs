use syn::parse::ParseStream;
use syn::{Ident, LitStr, Token};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Support {
    No,
    Any,
    RequiredOnly,
    OptionalOnly,
}

#[derive(Debug)]
pub struct Info {
    pub ident: &'static str,
    pub name: &'static str,
    pub chrome: Support,
    pub firefox: Support,
}

use Support::{Any, No, OptionalOnly, RequiredOnly};

macro_rules! table {
    ($($ident:ident $name:literal $chrome:ident $firefox:ident;)*) => {
        pub const TABLE: &[Info] = &[$(Info { ident: stringify!($ident), name: $name, chrome: $chrome, firefox: $firefox },)*];
    };
}

table! {
    AccessibilityFeaturesModify "accessibilityFeatures.modify" Any No;
    AccessibilityFeaturesRead "accessibilityFeatures.read" Any No;
    ActiveTab "activeTab" Any Any;
    Alarms "alarms" Any RequiredOnly;
    Audio "audio" Any No;
    Background "background" Any No;
    Bookmarks "bookmarks" Any Any;
    BrowserSettings "browserSettings" No Any;
    BrowsingData "browsingData" Any Any;
    CaptivePortal "captivePortal" No RequiredOnly;
    CertificateProvider "certificateProvider" Any No;
    ClipboardRead "clipboardRead" Any Any;
    ClipboardWrite "clipboardWrite" Any Any;
    ContentSettings "contentSettings" Any No;
    ContextMenus "contextMenus" Any RequiredOnly;
    ContextualIdentities "contextualIdentities" No RequiredOnly;
    Cookies "cookies" Any Any;
    Debugger "debugger" RequiredOnly No;
    DeclarativeContent "declarativeContent" Any No;
    DeclarativeNetRequest "declarativeNetRequest" RequiredOnly RequiredOnly;
    DeclarativeNetRequestFeedback "declarativeNetRequestFeedback" Any Any;
    DeclarativeNetRequestWithHostAccess "declarativeNetRequestWithHostAccess" Any RequiredOnly;
    DesktopCapture "desktopCapture" Any No;
    Devtools "devtools" No Any;
    Dns "dns" Any RequiredOnly;
    DocumentScan "documentScan" Any No;
    Downloads "downloads" Any Any;
    DownloadsOpen "downloads.open" Any Any;
    DownloadsUi "downloads.ui" Any No;
    EnterpriseDeviceAttributes "enterprise.deviceAttributes" Any No;
    EnterpriseHardwarePlatform "enterprise.hardwarePlatform" Any No;
    EnterpriseNetworkingAttributes "enterprise.networkingAttributes" Any No;
    EnterprisePlatformKeys "enterprise.platformKeys" Any No;
    Favicon "favicon" Any No;
    FileBrowserHandler "fileBrowserHandler" Any No;
    FileSystemProvider "fileSystemProvider" Any No;
    Find "find" No Any;
    FontSettings "fontSettings" RequiredOnly No;
    Gcm "gcm" Any No;
    Geolocation "geolocation" RequiredOnly Any;
    History "history" Any Any;
    Identity "identity" Any RequiredOnly;
    IdentityEmail "identity.email" Any No;
    Idle "idle" Any Any;
    LoginState "loginState" Any No;
    Management "management" Any Any;
    MenusOverrideContext "menus.overrideContext" No Any;
    NativeMessaging "nativeMessaging" Any Any;
    Notifications "notifications" Any Any;
    Offscreen "offscreen" Any No;
    PageCapture "pageCapture" Any No;
    Pkcs11 "pkcs11" No Any;
    PlatformKeys "platformKeys" Any No;
    Power "power" Any No;
    PrinterProvider "printerProvider" Any No;
    Printing "printing" Any No;
    PrintingMetrics "printingMetrics" Any No;
    Privacy "privacy" Any Any;
    Processes "processes" Any No;
    Proxy "proxy" RequiredOnly Any;
    ReadingList "readingList" Any No;
    Scripting "scripting" Any Any;
    Search "search" Any Any;
    Sessions "sessions" Any Any;
    SidePanel "sidePanel" Any No;
    Storage "storage" Any RequiredOnly;
    SystemCpu "system.cpu" Any No;
    SystemDisplay "system.display" Any No;
    SystemMemory "system.memory" Any No;
    SystemStorage "system.storage" Any No;
    TabCapture "tabCapture" Any No;
    TabGroups "tabGroups" Any Any;
    TabHide "tabHide" No Any;
    Tabs "tabs" Any Any;
    Theme "theme" No RequiredOnly;
    TopSites "topSites" Any Any;
    TrialMl "trialML" No OptionalOnly;
    Tts "tts" RequiredOnly No;
    TtsEngine "ttsEngine" RequiredOnly No;
    UnlimitedStorage "unlimitedStorage" RequiredOnly RequiredOnly;
    UserScripts "userScripts" Any OptionalOnly;
    VpnProvider "vpnProvider" Any No;
    Wallpaper "wallpaper" Any No;
    WebAuthenticationProxy "webAuthenticationProxy" RequiredOnly No;
    WebNavigation "webNavigation" Any Any;
    WebRequest "webRequest" Any Any;
    WebRequestAuthProvider "webRequestAuthProvider" Any Any;
    WebRequestBlocking "webRequestBlocking" No Any;
    WebRequestFilterResponse "webRequestFilterResponse" No Any;
    WebRequestFilterResponseServiceWorkerScript "webRequestFilterResponse.serviceWorkerScript" No Any;
}

#[derive(Debug)]
pub struct Declared {
    pub ident: Ident,
    pub info: &'static Info,
}

pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.to_ascii_lowercase().chars().collect();
    let b: Vec<char> = b.to_ascii_lowercase().chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let current = row[j + 1];
            row[j + 1] = (previous + usize::from(ca != cb)).min(row[j] + 1).min(row[j + 1] + 1);
            previous = current;
        }
    }
    row[b.len()]
}

fn unknown(word: &str) -> String {
    if word.eq_ignore_ascii_case("menus") {
        return "write `ContextMenus`; Firefox's `menus` is the same permission, and `ContextMenus` works in both browsers".to_string();
    }
    let compact: String = word.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if let Some(info) = TABLE.iter().find(|info| info.ident.eq_ignore_ascii_case(&compact)) {
        return format!("write `{}`", info.ident);
    }
    let mut close: Vec<&Info> = TABLE.iter().filter(|info| distance(info.ident, &compact) <= 2).collect();
    close.sort_by_key(|info| distance(info.ident, &compact));
    match close.first() {
        Some(info) => format!("`{word}` isn't a permission; did you mean `{}`?", info.ident),
        None => format!("`{word}` isn't a permission either browser knows; see `oxichrome::Permission` for the list"),
    }
}

fn looks_like_host(text: &str) -> bool {
    text == "<all_urls>" || text.contains("://")
}

pub fn parse_list(input: ParseStream, key: &str) -> syn::Result<Vec<Declared>> {
    let optional = key == "optional_permissions";
    let content;
    syn::bracketed!(content in input);
    let mut declared: Vec<Declared> = Vec::new();
    while !content.is_empty() {
        if content.peek(LitStr) {
            let text: LitStr = content.parse()?;
            let value = text.value();
            let host_key = if optional { "optional_host_permissions" } else { "host_permissions" };
            let message = if looks_like_host(&value) {
                format!("`{value}` is a site pattern; move it to `{host_key}` (Chrome ignores site patterns in `{key}`)")
            } else if let Some(info) = TABLE.iter().find(|info| info.name == value) {
                format!("write `{}` without quotes; permissions are checked names, not strings", info.ident)
            } else {
                unknown(&value)
            };
            return Err(syn::Error::new(text.span(), message));
        }
        let ident: Ident = content.parse()?;
        let name = ident.to_string();
        let Some(info) = TABLE.iter().find(|info| info.ident == name) else {
            return Err(syn::Error::new(ident.span(), unknown(&name)));
        };
        if declared.iter().any(|seen| seen.info.ident == info.ident) {
            return Err(syn::Error::new(ident.span(), format!("`{name}` is listed twice in `{key}`")));
        }
        let browsers = [("Chrome", info.chrome), ("Firefox", info.firefox)];
        for (browser, support) in browsers {
            if optional && support == RequiredOnly {
                return Err(syn::Error::new(
                    ident.span(),
                    format!("{browser} can't ask for `{name}` later, only grant it at install; move it to `permissions`"),
                ));
            }
            if !optional && support == OptionalOnly {
                return Err(syn::Error::new(
                    ident.span(),
                    format!("{browser} only grants `{name}` when the extension asks for it; move it to `optional_permissions`"),
                ));
            }
        }
        declared.push(Declared { ident, info });
        if !content.is_empty() {
            content.parse::<Token![,]>()?;
        }
    }
    Ok(declared)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(tokens: proc_macro2::TokenStream, key: &str) -> String {
        syn::parse::Parser::parse2(|input: ParseStream| parse_list(input, key), tokens)
            .err()
            .unwrap()
            .to_string()
    }

    #[test]
    fn checks_names_and_where_they_can_go() {
        assert!(err(quote::quote! { ["tabs"] }, "permissions").contains("write `Tabs` without quotes"));
        assert!(err(quote::quote! { ["<all_urls>"] }, "permissions").contains("move it to `host_permissions`"));
        assert!(err(quote::quote! { ["https://a.com/*"] }, "optional_permissions").contains("move it to `optional_host_permissions`"));
        assert!(err(quote::quote! { [Tab] }, "permissions").contains("did you mean `Tabs`?"));
        assert!(err(quote::quote! { [tabs] }, "permissions").contains("write `Tabs`"));
        assert!(err(quote::quote! { [Menus] }, "permissions").contains("write `ContextMenus`"));
        assert!(err(quote::quote! { [Nope] }, "permissions").contains("isn't a permission either browser knows"));
        assert!(err(quote::quote! { [Tabs, Tabs] }, "permissions").contains("listed twice"));
        assert!(err(quote::quote! { [Storage] }, "optional_permissions").contains("Firefox can't ask for `Storage` later"));
        assert!(err(quote::quote! { [Debugger] }, "optional_permissions").contains("Chrome can't ask for `Debugger` later"));
        assert!(err(quote::quote! { [UserScripts] }, "permissions").contains("Firefox only grants `UserScripts` when"));
        let ok = syn::parse::Parser::parse2(|input: ParseStream| parse_list(input, "optional_permissions"), quote::quote! { [History, Bookmarks, UserScripts] }).unwrap();
        assert_eq!(ok.iter().map(|d| d.info.name).collect::<Vec<_>>(), ["history", "bookmarks", "userScripts"]);
    }

    #[test]
    fn table_names_are_unique() {
        for (i, info) in TABLE.iter().enumerate() {
            assert!(TABLE[..i].iter().all(|other| other.ident != info.ident && other.name != info.name), "{}", info.ident);
            assert!(info.chrome != No || info.firefox != No, "{}", info.ident);
        }
    }
}
