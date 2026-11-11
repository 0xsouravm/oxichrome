use std::time::Duration;

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

const MAX_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SameSite {
    #[default]
    Unspecified,
    Lax,
    Strict,
    None,
}

impl SameSite {
    fn as_str(self) -> &'static str {
        match self {
            SameSite::Unspecified => "unspecified",
            SameSite::Lax => "lax",
            SameSite::Strict => "strict",
            SameSite::None => "no_restriction",
        }
    }

    fn parse(text: Option<&str>) -> Self {
        match text {
            Some("lax") => SameSite::Lax,
            Some("strict") => SameSite::Strict,
            Some("no_restriction") => SameSite::None,
            _ => SameSite::Unspecified,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub top_level_site: String,
    pub cross_site: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "Raw")]
#[non_exhaustive]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub host_only: bool,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSite,
    pub expires: Option<f64>,
    pub store: String,
    pub partition: Option<Partition>,
}

impl Cookie {
    pub fn url(&self) -> String {
        format!("{}://{}{}", if self.secure { "https" } else { "http" }, self.domain, self.path)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPartition {
    top_level_site: Option<String>,
    has_cross_site_ancestor: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    name: String,
    value: Option<String>,
    domain: String,
    host_only: Option<bool>,
    path: Option<String>,
    secure: Option<bool>,
    http_only: Option<bool>,
    same_site: Option<String>,
    session: Option<bool>,
    expiration_date: Option<f64>,
    store_id: Option<String>,
    partition_key: Option<RawPartition>,
}

impl From<Raw> for Cookie {
    fn from(raw: Raw) -> Self {
        let partition = raw.partition_key.and_then(|key| {
            key.top_level_site.filter(|site| !site.is_empty()).map(|site| Partition {
                top_level_site: site,
                cross_site: key.has_cross_site_ancestor.unwrap_or(true),
            })
        });
        Cookie {
            name: raw.name,
            value: raw.value.unwrap_or_default(),
            domain: raw.domain.trim_start_matches('.').to_string(),
            host_only: raw.host_only.unwrap_or(false),
            path: raw.path.filter(|path| !path.is_empty()).unwrap_or_else(|| "/".to_string()),
            secure: raw.secure.unwrap_or(false),
            http_only: raw.http_only.unwrap_or(false),
            same_site: SameSite::parse(raw.same_site.as_deref()),
            expires: if raw.session.unwrap_or(false) { None } else { raw.expiration_date },
            store: raw.store_id.unwrap_or_default(),
            partition,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    Explicit,
    Expired,
    Evicted,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Set(Cookie),
    Removed { cookie: Cookie, cause: Cause },
}

#[derive(Deserialize)]
struct RawChange {
    removed: bool,
    cause: Option<String>,
    cookie: Cookie,
}

impl FromEventArg for Change {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let raw: RawChange = crate::json::from_js(&value)?;
        Ok(if raw.removed {
            let cause = match raw.cause.as_deref() {
                Some("expired") => Cause::Expired,
                Some("evicted") => Cause::Evicted,
                _ => Cause::Explicit,
            };
            Change::Removed { cookie: raw.cookie, cause }
        } else {
            Change::Set(raw.cookie)
        })
    }
}

impl EventArg<events::CookiesOnChanged, 1> for Change {}

#[derive(Debug, Clone, PartialEq)]
pub struct Store {
    pub id: String,
    pub incognito: bool,
    pub tab_ids: Vec<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStore {
    id: String,
    incognito: Option<bool>,
    tab_ids: Option<Vec<i32>>,
}

struct Url {
    scheme: String,
    host: String,
    origin: String,
    path: String,
}

fn parse_url(url: &str) -> Result<Url> {
    let bad = |why: &str| Err(OxichromeError::InvalidArgument(format!("`{url}` {why}")));
    let Some((scheme, rest)) = url.split_once("://") else {
        return bad("isn't a full URL; write it like `https://example.com/`");
    };
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return bad("can't hold cookies here; cookies work on `http` and `https` URLs");
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return bad("contains spaces or line breaks");
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let host_port = authority.rsplit_once('@').map(|(_, host)| host).unwrap_or(authority);
    let host = if host_port.starts_with('[') {
        host_port.split_once(']').map(|(host, _)| format!("{host}]")).unwrap_or_default()
    } else {
        host_port.split(':').next().unwrap_or("").to_string()
    };
    if host.is_empty() {
        return bad("has no host; write it like `https://example.com/`");
    }
    let tail = &rest[end..];
    let path = if tail.starts_with('/') { tail.split(['?', '#']).next().unwrap_or("/").to_string() } else { "/".to_string() };
    Ok(Url { origin: format!("{scheme}://{host_port}"), scheme, host: host.to_ascii_lowercase(), path })
}

fn default_path(path: &str) -> String {
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(index) => path[..index].to_string(),
    }
}

fn is_ip(host: &str) -> bool {
    host.starts_with('[') || (!host.is_empty() && host.chars().all(|c| c.is_ascii_digit() || c == '.'))
}

fn check_name(name: &str) -> Result<()> {
    let bad = |why: String| Err(OxichromeError::InvalidArgument(format!("the cookie name `{name}` {why}")));
    if name.is_empty() {
        return Err(OxichromeError::InvalidArgument("a cookie needs a name (Firefox refuses an empty one)".to_string()));
    }
    if name.trim() != name {
        return bad("starts or ends with a space".to_string());
    }
    if let Some(c) = name.chars().find(|c| !c.is_ascii()) {
        return bad(format!("has {c:?}; use ASCII in names, Chrome and Firefox send other characters differently"));
    }
    if let Some(c) = name.chars().find(|c| c.is_control() || matches!(c, ';' | '=' | ',')) {
        return bad(format!("can't contain {c:?}"));
    }
    Ok(())
}

fn check_value(name: &str, value: &str) -> Result<()> {
    let bad = |why: String| Err(OxichromeError::InvalidArgument(format!("the value of cookie `{name}` {why}")));
    if value.trim() != value {
        return bad("starts or ends with a space, which the browser would drop".to_string());
    }
    if let Some(c) = value.chars().find(|c| c.is_control() || *c == ';') {
        return bad(format!("can't contain {c:?}; encode it (for example with percent encoding or base64)"));
    }
    if name.len() + value.len() > MAX_SIZE {
        return bad(format!("is too big: name and value together are {} bytes, the limit is {MAX_SIZE}", name.len() + value.len()));
    }
    Ok(())
}

fn check_path(path: &str) -> Result<()> {
    if !path.starts_with('/') {
        return Err(OxichromeError::InvalidArgument(format!("the cookie path `{path}` must start with `/`")));
    }
    if let Some(c) = path.chars().find(|c| c.is_control() || *c == ';') {
        return Err(OxichromeError::InvalidArgument(format!("the cookie path `{path}` can't contain {c:?}")));
    }
    Ok(())
}

fn check_domain(domain: &str) -> Result<String> {
    let domain = domain.trim_start_matches('.').to_ascii_lowercase();
    if domain.is_empty() || domain.contains(['/', ':', ' ', '?', '#', '@']) {
        return Err(OxichromeError::InvalidArgument(format!(
            "`{domain}` isn't a domain; write just the host, like `example.com`"
        )));
    }
    Ok(domain)
}

fn check_site(site: &str) -> Result<String> {
    let parsed = parse_url(site).map_err(|_| {
        OxichromeError::InvalidArgument(format!("the partition site `{site}` isn't a site; write it like `https://example.com`"))
    })?;
    if parsed.path != "/" || site.trim_end_matches('/').len() != parsed.origin.len() || parsed.origin.len() != parsed.scheme.len() + 3 + parsed.host.len() {
        return Err(OxichromeError::InvalidArgument(format!(
            "the partition site `{site}` should be just a scheme and host, like `https://example.com` (no port or path)"
        )));
    }
    Ok(format!("{}://{}", parsed.scheme, parsed.host))
}

#[derive(Debug, Clone, PartialEq)]
enum Expiry {
    Session,
    At(f64),
    In(Duration),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Set {
    url: String,
    name: String,
    value: String,
    domain: Option<String>,
    path: Option<String>,
    secure: bool,
    http_only: bool,
    same_site: SameSite,
    expiry: Expiry,
    store: Option<String>,
    partition: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawKey<'a> {
    top_level_site: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawSet<'a> {
    url: String,
    name: &'a str,
    value: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain: Option<&'a str>,
    path: &'a str,
    secure: bool,
    http_only: bool,
    same_site: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiration_date: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    store_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    partition_key: Option<RawKey<'a>>,
}

impl Set {
    pub fn new(url: impl Into<String>, name: impl Into<String>, value: impl Into<String>) -> Self {
        Set {
            url: url.into(),
            name: name.into(),
            value: value.into(),
            domain: None,
            path: None,
            secure: false,
            http_only: false,
            same_site: SameSite::Unspecified,
            expiry: Expiry::Session,
            store: None,
            partition: None,
        }
    }

    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }

    pub fn http_only(mut self) -> Self {
        self.http_only = true;
        self
    }

    pub fn same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = same_site;
        self
    }

    pub fn expires_at(mut self, seconds_since_epoch: f64) -> Self {
        self.expiry = Expiry::At(seconds_since_epoch);
        self
    }

    pub fn expires_in(mut self, duration: Duration) -> Self {
        self.expiry = Expiry::In(duration);
        self
    }

    pub fn store(mut self, store: impl Into<String>) -> Self {
        self.store = Some(store.into());
        self
    }

    pub fn partitioned(mut self, top_level_site: impl Into<String>) -> Self {
        self.partition = Some(top_level_site.into());
        self
    }

    fn check(&self, now: f64) -> Result<(String, String, Option<String>, Option<f64>, Option<String>)> {
        let url = parse_url(&self.url)?;
        check_name(&self.name)?;
        check_value(&self.name, &self.value)?;
        let bad = |why: String| Err(OxichromeError::InvalidArgument(format!("cookie `{}`: {why}", self.name)));
        let path = match &self.path {
            Some(path) => {
                check_path(path)?;
                path.clone()
            }
            None => default_path(&url.path),
        };
        let domain = match &self.domain {
            Some(domain) => {
                let domain = check_domain(domain)?;
                if is_ip(&url.host) {
                    if domain != url.host {
                        return bad(format!("the domain `{domain}` doesn't match `{}`", url.host));
                    }
                    None
                } else if url.host == domain || url.host.ends_with(&format!(".{domain}")) {
                    Some(domain)
                } else {
                    return bad(format!("the domain `{domain}` isn't `{}` or a parent of it, so the browser would refuse it", url.host));
                }
            }
            None => None,
        };
        let secure_needed = |why: &str| bad(format!("{why} needs `.secure()`"));
        if self.same_site == SameSite::None && !self.secure {
            return secure_needed("`SameSite::None`");
        }
        let lower = self.name.to_ascii_lowercase();
        if lower.starts_with("__secure-") && !self.secure {
            return secure_needed("a `__Secure-` name");
        }
        if lower.starts_with("__host-") {
            if !self.secure {
                return secure_needed("a `__Host-` name");
            }
            if domain.is_some() || self.domain.is_some() {
                return bad("a `__Host-` cookie can't set a domain".to_string());
            }
            if path != "/" {
                return bad("a `__Host-` cookie must have the path `/`".to_string());
            }
        }
        let partition = match &self.partition {
            Some(site) => {
                if !self.secure {
                    return secure_needed("a partitioned cookie");
                }
                Some(check_site(site)?)
            }
            None => None,
        };
        if self.secure && url.scheme != "https" {
            return bad(format!(
                "a secure cookie needs an `https://` URL; Firefox refuses it on `{}` (even on localhost)",
                self.url
            ));
        }
        let expires = match self.expiry {
            Expiry::Session => None,
            Expiry::At(at) if !at.is_finite() => return bad("the expiry isn't a number".to_string()),
            Expiry::At(at) if at <= now => return bad("the expiry is in the past; use `cookies::remove` to delete a cookie".to_string()),
            Expiry::At(at) => Some(at),
            Expiry::In(duration) if duration.is_zero() => return bad("expires in zero time; use `cookies::remove` to delete a cookie".to_string()),
            Expiry::In(duration) => Some(now + duration.as_secs_f64()),
        };
        if let Some(store) = &self.store {
            if store.is_empty() {
                return bad("the store id is empty".to_string());
            }
        }
        Ok((format!("{}{}", url.origin, path), path, domain, expires, partition))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum PartitionFilter {
    #[default]
    Unpartitioned,
    Any,
    Site(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    url: Option<String>,
    domain: Option<String>,
    name: Option<String>,
    path: Option<String>,
    secure: Option<bool>,
    session: Option<bool>,
    store: Option<String>,
    partition: PartitionFilter,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawQuery<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    store_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    partition_key: Option<serde_json::Value>,
}

impl Query {
    pub fn new() -> Self {
        Query::default()
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = Some(secure);
        self
    }

    pub fn session(mut self, session: bool) -> Self {
        self.session = Some(session);
        self
    }

    pub fn store(mut self, store: impl Into<String>) -> Self {
        self.store = Some(store.into());
        self
    }

    pub fn any_partition(mut self) -> Self {
        self.partition = PartitionFilter::Any;
        self
    }

    pub fn partitioned(mut self, top_level_site: impl Into<String>) -> Self {
        self.partition = PartitionFilter::Site(top_level_site.into());
        self
    }

    fn raw(&self) -> Result<RawQuery<'_>> {
        if let Some(url) = &self.url {
            parse_url(url)?;
        }
        if let Some(path) = &self.path {
            check_path(path)?;
        }
        let partition_key = match &self.partition {
            PartitionFilter::Unpartitioned => None,
            PartitionFilter::Any => Some(serde_json::json!({})),
            PartitionFilter::Site(site) => Some(serde_json::json!({ "topLevelSite": check_site(site)? })),
        };
        Ok(RawQuery {
            url: self.url.as_deref(),
            domain: self.domain.as_deref().map(check_domain).transpose()?,
            name: self.name.as_deref(),
            path: self.path.as_deref(),
            secure: self.secure,
            session: self.session,
            store_id: self.store.as_deref(),
            partition_key,
        })
    }
}

fn no_host_permission() -> OxichromeError {
    OxichromeError::InvalidArgument(
        "the extension has no host permission for this site; add it to `host_permissions` in #[oxichrome::extension], like `\"https://example.com/*\"`".to_string(),
    )
}

async fn reachable(url: &str) -> Result<()> {
    let promise = js_bridge::cookies_reachable(url).map_err(browser_error)?;
    match JsFuture::from(promise).await.map_err(browser_error)?.as_bool() {
        Some(false) => Err(no_host_permission()),
        _ => Ok(()),
    }
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    if message == "oxichrome:no-permission" {
        return OxichromeError::InvalidArgument(
            "the cookies API isn't available; add `Cookies` to `permissions` in #[oxichrome::extension] (if it's optional, request it first)".to_string(),
        );
    }
    if message.contains("No host permissions") || message.contains("Permission denied to set cookie") {
        return no_host_permission();
    }
    if message.contains("Invalid cookie store id") {
        return OxichromeError::InvalidArgument(format!("{}; list the stores with `cookies::stores()`", message.trim_end_matches('.')));
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

async fn call(method: &str, arg: JsValue) -> Result<JsValue> {
    let promise = js_bridge::cookies_call(method, &arg).map_err(browser_error)?;
    JsFuture::from(promise).await.map_err(browser_error)
}

fn now() -> f64 {
    js_sys::Date::now() / 1000.0
}

pub async fn set(cookie: Set) -> Result<Cookie> {
    let (url, path, domain, expires, partition) = cookie.check(now())?;
    let raw = RawSet {
        url,
        name: &cookie.name,
        value: &cookie.value,
        domain: domain.as_deref(),
        path: &path,
        secure: cookie.secure,
        http_only: cookie.http_only,
        same_site: cookie.same_site.as_str(),
        expiration_date: expires,
        store_id: cookie.store.as_deref(),
        partition_key: partition.as_deref().map(|site| RawKey { top_level_site: site }),
    };
    let refused = || {
        OxichromeError::InvalidArgument(format!(
            "the browser refused cookie `{}` for `{}` (a domain like `com` or `github.io` that anyone can register can't hold cookies)",
            cookie.name, cookie.url
        ))
    };
    let value = match call("set", crate::json::to_js(&raw)?).await {
        Ok(value) => value,
        Err(OxichromeError::Js(_)) => return Err(refused()),
        Err(error) if domain.is_some() && error.to_string().contains("no host permission") => {
            reachable(&raw.url).await?;
            return Err(refused());
        }
        Err(error) => return Err(error),
    };
    if value.is_null() || value.is_undefined() {
        return Err(OxichromeError::InvalidArgument(format!(
            "the browser refused cookie `{}` for `{}` without saying why",
            cookie.name, cookie.url
        )));
    }
    crate::json::from_js(&value)
}

pub async fn get(url: &str, name: &str) -> Result<Option<Cookie>> {
    parse_url(url)?;
    let value = call("get", crate::json::to_js(&serde_json::json!({ "url": url, "name": name }))?).await?;
    if value.is_null() || value.is_undefined() {
        reachable(url).await?;
        return Ok(None);
    }
    crate::json::from_js(&value).map(Some)
}

pub async fn all(query: Query) -> Result<Vec<Cookie>> {
    let value = call("getAll", crate::json::to_js(&query.raw()?)?).await?;
    let mut cookies: Vec<Cookie> = crate::json::from_js(&value)?;
    if let (true, Some(url)) = (cookies.is_empty(), &query.url) {
        reachable(url).await?;
    }
    cookies.sort_by(|a, b| (&a.domain, std::cmp::Reverse(a.path.len()), &a.name).cmp(&(&b.domain, std::cmp::Reverse(b.path.len()), &b.name)));
    Ok(cookies)
}

pub async fn remove_cookie(cookie: &Cookie) -> Result<bool> {
    let mut query = Query::new().name(cookie.name.clone()).domain(cookie.domain.clone()).store(cookie.store.clone());
    query = match &cookie.partition {
        Some(partition) => query.partitioned(partition.top_level_site.clone()),
        None => query,
    };
    let exists = all(query)
        .await?
        .iter()
        .any(|found| found.domain == cookie.domain && found.path == cookie.path && found.host_only == cookie.host_only && found.partition == cookie.partition);
    if !exists {
        return Ok(false);
    }
    let raw = RawSet {
        url: cookie.url(),
        name: &cookie.name,
        value: "",
        domain: if cookie.host_only { None } else { Some(&cookie.domain) },
        path: &cookie.path,
        secure: cookie.secure,
        http_only: cookie.http_only,
        same_site: cookie.same_site.as_str(),
        expiration_date: Some(1.0),
        store_id: if cookie.store.is_empty() { None } else { Some(&cookie.store) },
        partition_key: cookie.partition.as_ref().map(|partition| RawKey { top_level_site: &partition.top_level_site }),
    };
    call("set", crate::json::to_js(&raw)?).await?;
    Ok(true)
}

pub async fn remove(url: &str, name: &str) -> Result<Option<Cookie>> {
    let Some(cookie) = get(url, name).await? else {
        return Ok(None);
    };
    Ok(if remove_cookie(&cookie).await? { Some(cookie) } else { None })
}

pub async fn remove_all(query: Query) -> Result<Vec<Cookie>> {
    let mut removed = Vec::new();
    for cookie in all(query).await? {
        if remove_cookie(&cookie).await? {
            removed.push(cookie);
        }
    }
    Ok(removed)
}

pub async fn stores() -> Result<Vec<Store>> {
    let value = call("getAllCookieStores", JsValue::UNDEFINED).await?;
    let raw: Vec<RawStore> = crate::json::from_js(&value)?;
    Ok(raw
        .into_iter()
        .map(|store| Store {
            incognito: store.incognito.unwrap_or(store.id == "1"),
            id: store.id,
            tab_ids: store.tab_ids.unwrap_or_default(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: f64 = 1_800_000_000.0;

    fn err(cookie: Set) -> String {
        match cookie.check(NOW).unwrap_err() {
            OxichromeError::InvalidArgument(message) => message,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn paths_follow_rfc_6265() {
        let (url, path, ..) = Set::new("http://127.0.0.1:8080/a/b/c.html?x=1", "p", "1").check(NOW).unwrap();
        assert_eq!((url.as_str(), path.as_str()), ("http://127.0.0.1:8080/a/b", "/a/b"));
        assert_eq!(Set::new("https://example.com", "p", "1").check(NOW).unwrap().1, "/");
        assert_eq!(Set::new("https://example.com/top", "p", "1").check(NOW).unwrap().1, "/");
        let (url, path, ..) = Set::new("https://example.com/a/b", "p", "1").path("/zzz").check(NOW).unwrap();
        assert_eq!((url.as_str(), path.as_str()), ("https://example.com/zzz", "/zzz"));
    }

    #[test]
    fn refusals_from_either_browser_are_caught() {
        let site = "https://www.example.com/";
        for (cookie, why) in [
            (Set::new("example.com", "a", "1"), "full URL"),
            (Set::new("ftp://example.com/", "a", "1"), "`http` and `https`"),
            (Set::new("https:///x", "a", "1"), "no host"),
            (Set::new(site, "", "1"), "needs a name"),
            (Set::new(site, "a;b", "1"), "';'"),
            (Set::new(site, "a=b", "1"), "'='"),
            (Set::new(site, "é", "1"), "ASCII"),
            (Set::new(site, " a", "1"), "space"),
            (Set::new(site, "a", "x;y"), "';'"),
            (Set::new(site, "a", "x\ny"), "'\\n'"),
            (Set::new(site, "a", "x".repeat(4096)), "4096"),
            (Set::new(site, "a", "1").path("nolead"), "start with `/`"),
            (Set::new(site, "a", "1").domain("other.org"), "parent"),
            (Set::new("http://127.0.0.1/", "a", "1").domain("10.0.0.1"), "doesn't match"),
            (Set::new(site, "a", "1").same_site(SameSite::None), "`.secure()`"),
            (Set::new(site, "__Secure-a", "1"), "`.secure()`"),
            (Set::new(site, "__host-a", "1"), "`.secure()`"),
            (Set::new(site, "__Host-a", "1").secure().domain("example.com"), "domain"),
            (Set::new(site, "__Host-a", "1").secure().path("/x"), "path `/`"),
            (Set::new(site, "a", "1").partitioned("https://top.org"), "`.secure()`"),
            (Set::new(site, "a", "1").secure().partitioned("top.org"), "isn't a site"),
            (Set::new(site, "a", "1").secure().partitioned("https://top.org/x"), "no port or path"),
            (Set::new(site, "a", "1").secure().partitioned("https://top.org:8443"), "no port or path"),
            (Set::new("http://localhost/", "a", "1").secure(), "https://"),
            (Set::new(site, "a", "1").expires_at(1000.0), "past"),
            (Set::new(site, "a", "1").expires_at(f64::NAN), "isn't a number"),
            (Set::new(site, "a", "1").expires_in(Duration::ZERO), "zero"),
            (Set::new(site, "a", "1").store(""), "store id"),
        ] {
            let message = err(cookie.clone());
            assert!(message.contains(why), "{cookie:?}: {message}");
        }
    }

    #[test]
    fn good_cookies_pass() {
        let site = "https://www.example.com/";
        for cookie in [
            Set::new(site, "a b", "x y"),
            Set::new(site, "a", ""),
            Set::new(site, "a", "\"quoted\",é✓"),
            Set::new(site, "a", "x".repeat(4095)),
            Set::new(site, "a", "1").domain(".Example.com"),
            Set::new(site, "a", "1").domain("www.example.com"),
            Set::new("http://127.0.0.1:9/", "a", "1").domain("127.0.0.1"),
            Set::new(site, "__Host-a", "1").secure(),
            Set::new(site, "__Secure-a", "1").secure().domain("example.com"),
            Set::new(site, "a", "1").secure().same_site(SameSite::None).partitioned("https://top.org/"),
            Set::new(site, "a", "1").expires_in(Duration::from_secs(60)),
        ] {
            assert!(cookie.check(NOW).is_ok(), "{cookie:?}: {:?}", cookie.check(NOW));
        }
        let (_, _, domain, expires, partition) =
            Set::new(site, "a", "1").domain(".Example.com").secure().partitioned("https://Top.org/").expires_in(Duration::from_secs(60)).check(NOW).unwrap();
        assert_eq!((domain.as_deref(), expires, partition.as_deref()), (Some("example.com"), Some(NOW + 60.0), Some("https://top.org")));
        assert_eq!(Set::new("http://127.0.0.1/", "a", "1").domain("127.0.0.1").check(NOW).unwrap().2, None);
    }

    #[test]
    fn cookies_read_the_same_from_both_browsers() {
        let chrome = r#"{"domain":".example.com","hostOnly":false,"httpOnly":false,"name":"d","path":"/","sameSite":"no_restriction","secure":true,"session":false,"expirationDate":1828987772.9,"storeId":"0","value":"1"}"#;
        let firefox = r#"{"domain":".example.com","firstPartyDomain":"","hostOnly":false,"httpOnly":false,"name":"d","partitionKey":null,"path":"/","sameSite":"no_restriction","secure":true,"session":false,"expirationDate":1828987772.9,"storeId":"firefox-default","value":"1"}"#;
        let a: Cookie = serde_json::from_str(chrome).unwrap();
        let b: Cookie = serde_json::from_str(firefox).unwrap();
        assert_eq!((a.domain.as_str(), a.same_site, a.expires, a.partition.clone()), ("example.com", SameSite::None, Some(1828987772.9), None));
        assert_eq!(Cookie { store: a.store.clone(), ..b }, a);
        assert_eq!(a.url(), "https://example.com/");
        let session: Cookie = serde_json::from_str(r#"{"domain":"127.0.0.1","hostOnly":true,"name":"s","path":"/a","session":true,"expirationDate":5,"value":"","partitionKey":{"topLevelSite":"https://top.org","hasCrossSiteAncestor":true}}"#).unwrap();
        assert_eq!((session.expires, session.url(), session.partition.unwrap().top_level_site), (None, "http://127.0.0.1/a".to_string(), "https://top.org".to_string()));
    }

    #[test]
    fn queries_check_their_parts() {
        assert!(Query::new().url("nope").raw().is_err());
        assert!(Query::new().path("x").raw().is_err());
        assert!(Query::new().domain("https://x.com").raw().is_err());
        assert_eq!(Query::new().domain(".Example.com").raw().unwrap().domain.as_deref(), Some("example.com"));
        assert_eq!(Query::new().any_partition().raw().unwrap().partition_key, Some(serde_json::json!({})));
        assert!(Query::new().raw().unwrap().partition_key.is_none());
    }
}
