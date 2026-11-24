//! Clearing what the browser keeps about the sites the user visits: cache, cookies,
//! history, downloads, saved form entries and passwords, and site storage.
//!
//! Needs `BrowsingData` in `permissions`.
//!
//! ```ignore
//! use browsing_data::{Clear, Data};
//! use std::time::Duration;
//!
//! // the last hour of history and downloads
//! browsing_data::clear(Clear::new([Data::History, Data::Downloads]).in_last(Duration::from_secs(3600))).await?;
//! // everything one site stored
//! browsing_data::clear(Clear::new([Data::Cookies, Data::LocalStorage, Data::IndexedDb]).site("https://example.com")).await?;
//! ```

use std::time::Duration;

use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;

/// A kind of data the browser keeps. Each one works in Chrome and Firefox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Data {
    /// The HTTP cache.
    Cache,
    Cookies,
    /// The download list (the files stay on disk).
    Downloads,
    /// Saved form entries.
    FormData,
    History,
    IndexedDb,
    LocalStorage,
    /// Saved passwords.
    Passwords,
    /// Service workers sites registered.
    ServiceWorkers,
}

impl Data {
    /// Every kind, in the order listed above.
    pub const ALL: [Data; 9] = [
        Data::Cache,
        Data::Cookies,
        Data::Downloads,
        Data::FormData,
        Data::History,
        Data::IndexedDb,
        Data::LocalStorage,
        Data::Passwords,
        Data::ServiceWorkers,
    ];

    /// The browser's name for it, like `localStorage`.
    pub fn name(self) -> &'static str {
        match self {
            Data::Cache => "cache",
            Data::Cookies => "cookies",
            Data::Downloads => "downloads",
            Data::FormData => "formData",
            Data::History => "history",
            Data::IndexedDb => "indexedDB",
            Data::LocalStorage => "localStorage",
            Data::Passwords => "passwords",
            Data::ServiceWorkers => "serviceWorkers",
        }
    }

    fn from_name(name: &str) -> Option<Data> {
        Data::ALL.into_iter().find(|data| data.name() == name)
    }

    /// Whether [`Clear::site`] can narrow it down: both browsers keep these per site.
    pub fn by_site(self) -> bool {
        matches!(self, Data::Cache | Data::Cookies | Data::IndexedDb | Data::LocalStorage | Data::ServiceWorkers)
    }
}

impl std::fmt::Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// What [`clear`] removes: which kinds of data, from when, and for which sites.
#[derive(Debug, Clone, PartialEq)]
pub struct Clear {
    data: Vec<Data>,
    since: f64,
    sites: Vec<String>,
}

impl Clear {
    /// These kinds of data, from all time and every site.
    pub fn new(data: impl IntoIterator<Item = Data>) -> Self {
        let mut data: Vec<Data> = data.into_iter().collect();
        data.sort();
        data.dedup();
        Clear { data, since: 0.0, sites: Vec::new() }
    }

    /// Only what was stored at or after this time, in milliseconds since the Unix epoch.
    pub fn since(mut self, millis: f64) -> Self {
        self.since = millis.max(0.0);
        self
    }

    /// Only what was stored in the last `span`.
    pub fn in_last(self, span: Duration) -> Self {
        let now = js_sys::Date::now();
        self.since(now - span.as_millis() as f64)
    }

    /// Only this site's data, given as an origin like `https://example.com`. Call it again
    /// for more sites. Chrome clears just that origin; Firefox clears that host and port
    /// for both `http` and `https`, since it narrows by host name. Cookies belong to the
    /// host in both browsers, whatever the port. Every kind of data must be one that
    /// [`Data::by_site`] allows.
    pub fn site(mut self, origin: impl Into<String>) -> Self {
        self.sites.push(origin.into());
        self
    }

    fn check(&self) -> Result<Vec<Site>> {
        if self.data.is_empty() {
            return Err(OxichromeError::InvalidArgument("`Clear::new` got no kinds of data to clear".into()));
        }
        let sites = self.sites.iter().map(|origin| Site::parse(origin)).collect::<Result<Vec<_>>>()?;
        if !sites.is_empty() {
            let whole: Vec<String> = self.data.iter().filter(|data| !data.by_site()).map(|data| format!("`Data::{data:?}`")).collect();
            if !whole.is_empty() {
                return Err(OxichromeError::InvalidArgument(format!(
                    "{} can't be cleared for one site in both browsers; clear {} without `site`, or leave {} out",
                    whole.join(", "),
                    if whole.len() == 1 { "it" } else { "them" },
                    if whole.len() == 1 { "it" } else { "them" },
                )));
            }
        }
        Ok(sites)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Site {
    origin: String,
    /// What Firefox narrows by: the host for cookies, and host and port for site storage.
    hostnames: Vec<String>,
}

impl Site {
    fn parse(origin: &str) -> Result<Site> {
        let bad = || {
            OxichromeError::InvalidArgument(format!(
                "`{origin}` isn't an origin; write the scheme and host, like `https://example.com`"
            ))
        };
        let (scheme, rest) = origin.split_once("://").ok_or_else(bad)?;
        if !matches!(scheme, "http" | "https") {
            return Err(bad());
        }
        let authority = rest.strip_suffix('/').unwrap_or(rest);
        if authority.is_empty() || authority.contains(['/', '?', '#', '@', '*']) {
            return Err(bad());
        }
        let host = if let Some(inner) = authority.strip_prefix('[') {
            inner.split(']').next().unwrap_or_default().to_string()
        } else {
            authority.rsplit_once(':').map_or(authority, |(host, port)| if port.chars().all(|c| c.is_ascii_digit()) { host } else { authority }).to_string()
        };
        if host.is_empty() {
            return Err(bad());
        }
        let authority = authority.to_ascii_lowercase();
        let host = host.to_ascii_lowercase();
        let mut hostnames = vec![host.clone()];
        if authority != host && authority != format!("[{host}]") {
            hostnames.push(authority.clone());
        }
        Ok(Site { origin: format!("{scheme}://{authority}"), hostnames })
    }
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = JsError::new(error).to_string();
    match message.as_str() {
        "oxichrome:no-permission" => OxichromeError::InvalidArgument(
            "the browsingData API isn't available; add `BrowsingData` to `permissions` in #[oxichrome::extension]".into(),
        ),
        _ if message.contains("localStorage with 'since'") => OxichromeError::InvalidArgument(
            "Firefox can't clear local storage from a given time; clear `Data::LocalStorage` without `since` or `in_last`".into(),
        ),
        _ => OxichromeError::Js(JsError::new(JsValue::from_str(&message))),
    }
}

/// Removes what `what` describes, and finishes once the browser has.
pub async fn clear(what: Clear) -> Result<()> {
    let sites = what.check()?;
    let names = js_sys::Array::new();
    for data in &what.data {
        names.push(&JsValue::from_str(data.name()));
    }
    let origins = js_sys::Array::new();
    let hosts = js_sys::Array::new();
    for site in &sites {
        origins.push(&JsValue::from_str(&site.origin));
        for host in &site.hostnames {
            hosts.push(&JsValue::from_str(host));
        }
    }
    let promise = js_bridge::browsing_data_clear(&names, what.since, &origins, &hosts).map_err(browser_error)?;
    JsFuture::from(promise).await.map_err(browser_error)?;
    Ok(())
}

/// What the browser's own "clear browsing data" dialog is set to.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The kinds of data ticked in the dialog.
    pub selected: Vec<Data>,
    /// The kinds an extension may clear; a policy can forbid some.
    pub permitted: Vec<Data>,
    /// The start of the time range picked there, in milliseconds since the Unix epoch
    /// (`0` for all time).
    pub since: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSettings {
    #[serde(default)]
    options: RawOptions,
    #[serde(default)]
    data_to_remove: std::collections::BTreeMap<String, bool>,
    #[serde(default)]
    data_removal_permitted: std::collections::BTreeMap<String, bool>,
}

#[derive(Deserialize, Default)]
struct RawOptions {
    #[serde(default)]
    since: f64,
}

fn read_settings(raw: RawSettings) -> Settings {
    let kinds = |map: &std::collections::BTreeMap<String, bool>| {
        let mut kinds: Vec<Data> = map.iter().filter(|(_, on)| **on).filter_map(|(name, _)| Data::from_name(name)).collect();
        kinds.sort();
        kinds
    };
    Settings { selected: kinds(&raw.data_to_remove), permitted: kinds(&raw.data_removal_permitted), since: raw.options.since.max(0.0) }
}

/// What the user picked in the browser's "clear browsing data" dialog, to offer the same.
pub async fn settings() -> Result<Settings> {
    let promise = js_bridge::browsing_data_settings().map_err(browser_error)?;
    let value = JsFuture::from(promise).await.map_err(browser_error)?;
    let raw: RawSettings = crate::json::from_js(&value)?;
    Ok(read_settings(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_are_origins() {
        assert_eq!(Site::parse("https://Example.com").unwrap(), Site { origin: "https://example.com".into(), hostnames: vec!["example.com".into()] });
        assert_eq!(
            Site::parse("http://127.0.0.1:8080/").unwrap(),
            Site { origin: "http://127.0.0.1:8080".into(), hostnames: vec!["127.0.0.1".into(), "127.0.0.1:8080".into()] }
        );
        assert_eq!(Site::parse("http://[::1]:80").unwrap().hostnames, vec!["::1".to_string(), "[::1]:80".to_string()]);
        assert_eq!(Site::parse("http://[::1]").unwrap().hostnames, vec!["::1".to_string()]);
        for bad in ["example.com", "ftp://example.com", "https://", "https://example.com/path", "https://*.example.com", "https://a@b.com"] {
            assert!(Site::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn site_only_with_kinds_both_browsers_keep_per_site() {
        let err = Clear::new([Data::Cookies, Data::History]).site("https://a.com").check().unwrap_err().to_string();
        assert!(err.contains("`Data::History` can't be cleared for one site"), "{err}");
        assert!(Clear::new([Data::Cookies, Data::LocalStorage]).site("https://a.com").check().is_ok());
        assert!(Clear::new([Data::History]).check().is_ok());
        assert!(Clear::new([]).check().is_err());
        assert_eq!(Clear::new([Data::History, Data::Cache, Data::History]).data, vec![Data::Cache, Data::History]);
    }

    #[test]
    fn settings_read_the_same_in_both_browsers() {
        let chrome = r#"{"dataRemovalPermitted":{"cache":true,"cacheStorage":true,"cookies":true,"downloads":true,"fileSystems":true,"formData":true,"history":true,"indexedDB":true,"localStorage":true,"passwords":true,"pluginData":true,"serviceWorkers":true},"dataToRemove":{"cache":true,"cacheStorage":true,"cookies":true,"downloads":true,"formData":false,"history":true,"passwords":false},"options":{"originTypes":{"unprotectedWeb":true},"since":1795490551668.017}}"#;
        let firefox = r#"{"options":{"since":1795490521317},"dataToRemove":{"cache":true,"cookies":true,"history":true,"formData":true,"downloads":true},"dataRemovalPermitted":{"cache":true,"cookies":true,"history":true,"formData":true,"downloads":true}}"#;
        let a = read_settings(serde_json::from_str(chrome).unwrap());
        assert_eq!(a.selected, vec![Data::Cache, Data::Cookies, Data::Downloads, Data::History]);
        assert_eq!(a.permitted, Data::ALL.to_vec());
        let b = read_settings(serde_json::from_str(firefox).unwrap());
        assert_eq!(b.selected, vec![Data::Cache, Data::Cookies, Data::Downloads, Data::FormData, Data::History]);
        assert_eq!(b.since, 1795490521317.0);
    }
}
