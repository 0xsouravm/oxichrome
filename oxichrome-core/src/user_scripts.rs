//! Register scripts the user brought (from a script manager, say) to run on pages, the same
//! way in Chrome and Firefox.
//!
//! The API needs `UserScripts` in `optional_permissions`, granted at runtime, and in Chrome the
//! user must also turn on "Allow User Scripts" for the extension. [`availability`] says which
//! step is missing.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

pub use crate::content_script::RunAt;
use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;

/// Which JavaScript world a script runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum World {
    /// Its own world: page globals are invisible, and `chrome.runtime.sendMessage` reaches
    /// `runtime::on_user_script_message` once [`configure_world`] turns messaging on.
    #[default]
    UserScript,
    /// The page's own world, sharing its globals, with no extension APIs.
    Main,
}

/// Where a script's code comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Code(String),
    /// A file in the extension, like `"scripts/hello.js"`.
    File(String),
}

/// A user script and where it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UserScript {
    pub id: String,
    pub matches: Vec<String>,
    pub exclude_matches: Vec<String>,
    pub js: Vec<Source>,
    pub run_at: RunAt,
    pub all_frames: bool,
    pub world: World,
}

impl UserScript {
    /// A script with this id (not empty, and not starting with `_`). Add at least one
    /// [`matches`](Self::matches) pattern and one [`code`](Self::code) or [`file`](Self::file).
    pub fn new(id: impl Into<String>) -> Self {
        UserScript { id: id.into(), matches: Vec::new(), exclude_matches: Vec::new(), js: Vec::new(), run_at: RunAt::DocumentIdle, all_frames: false, world: World::UserScript }
    }

    /// A match pattern like `"https://*.example.com/*"`. Leave the port out: Firefox never
    /// matches a pattern with a port, and `http://localhost/*` already matches every port.
    pub fn matches(mut self, pattern: impl Into<String>) -> Self {
        self.matches.push(pattern.into());
        self
    }

    pub fn exclude_matches(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_matches.push(pattern.into());
        self
    }

    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.js.push(Source::Code(code.into()));
        self
    }

    pub fn file(mut self, path: impl Into<String>) -> Self {
        self.js.push(Source::File(path.into()));
        self
    }

    /// When it runs; `DocumentIdle` unless set.
    pub fn run_at(mut self, run_at: RunAt) -> Self {
        self.run_at = run_at;
        self
    }

    pub fn all_frames(mut self) -> Self {
        self.all_frames = true;
        self
    }

    pub fn main_world(mut self) -> Self {
        self.world = World::Main;
        self
    }

    fn check(&self) -> Result<()> {
        let invalid = |reason: String| Err(OxichromeError::InvalidArgument(format!("user script `{}`: {reason}", self.id)));
        if self.id.is_empty() {
            return Err(OxichromeError::InvalidArgument("a user script needs an id".into()));
        }
        if self.id.starts_with('_') {
            return invalid("ids can't start with `_`".into());
        }
        if self.matches.is_empty() {
            return invalid("add at least one `matches` pattern".into());
        }
        if self.js.is_empty() {
            return invalid("add some `code` or a `file`".into());
        }
        for pattern in self.matches.iter().chain(&self.exclude_matches) {
            if let Err(reason) = check_pattern(pattern) {
                return invalid(format!("`{pattern}` {reason}"));
            }
        }
        Ok(())
    }

    fn to_raw(&self) -> RawScript {
        RawScript {
            id: self.id.clone(),
            matches: self.matches.clone(),
            exclude_matches: self.exclude_matches.clone(),
            js: self
                .js
                .iter()
                .map(|source| match source {
                    Source::Code(code) => RawSource { code: Some(code.clone()), file: None },
                    Source::File(file) => RawSource { code: None, file: Some(file.clone()) },
                })
                .collect(),
            run_at: match self.run_at {
                RunAt::DocumentStart => "document_start",
                RunAt::DocumentEnd => "document_end",
                RunAt::DocumentIdle => "document_idle",
            }
            .to_string(),
            all_frames: self.all_frames,
            world: match self.world {
                World::UserScript => "USER_SCRIPT",
                World::Main => "MAIN",
            }
            .to_string(),
        }
    }

    fn from_raw(raw: RawScript) -> Self {
        UserScript {
            id: raw.id,
            matches: raw.matches,
            exclude_matches: raw.exclude_matches,
            js: raw
                .js
                .into_iter()
                .filter_map(|source| match (source.code, source.file) {
                    (Some(code), _) => Some(Source::Code(code)),
                    (None, Some(file)) => Some(Source::File(file)),
                    (None, None) => None,
                })
                .collect(),
            run_at: match raw.run_at.as_str() {
                "document_start" => RunAt::DocumentStart,
                "document_end" => RunAt::DocumentEnd,
                _ => RunAt::DocumentIdle,
            },
            all_frames: raw.all_frames,
            world: if raw.world == "MAIN" { World::Main } else { World::UserScript },
        }
    }
}

/// Checks a match pattern against what both browsers accept and actually match.
fn check_pattern(pattern: &str) -> std::result::Result<(), &'static str> {
    if pattern == "<all_urls>" {
        return Ok(());
    }
    let Some((scheme, rest)) = pattern.split_once("://") else {
        return Err("isn't a match pattern; start with a scheme like `https://` or use `<all_urls>`");
    };
    if !matches!(scheme, "http" | "https" | "*" | "file" | "ftp") {
        return Err("has a scheme that isn't `http`, `https`, `*`, `file` or `ftp`");
    }
    if scheme == "file" {
        return if rest.starts_with('/') { Ok(()) } else { Err("needs a path, like `file:///*`") };
    }
    let Some(slash) = rest.find('/') else {
        return Err("needs a path after the host, like `/*`");
    };
    let host = &rest[..slash];
    if host.is_empty() {
        return Err("needs a host");
    }
    if host.rsplit_once(':').is_some_and(|(_, port)| !port.ends_with(']')) && !host.ends_with(']') {
        return Err("has a port, which Firefox never matches; leave it out (the pattern then matches every port)");
    }
    if host.contains('*') && host != "*" && !(host.starts_with("*.") && !host[2..].contains('*')) {
        return Err("can only have `*` alone or at the start, like `*.example.com`");
    }
    Ok(())
}

/// Firefox reports unset lists and flags as `null`.
fn null_as_default<'de, D: serde::Deserializer<'de>, T: Default + Deserialize<'de>>(deserializer: D) -> std::result::Result<T, D::Error> {
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawScript {
    id: String,
    #[serde(default, deserialize_with = "null_as_default")]
    matches: Vec<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    exclude_matches: Vec<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    js: Vec<RawSource>,
    #[serde(default, deserialize_with = "null_as_default")]
    run_at: String,
    #[serde(default, deserialize_with = "null_as_default")]
    all_frames: bool,
    #[serde(default, deserialize_with = "null_as_default")]
    world: String,
}

#[derive(Serialize, Deserialize)]
struct RawSource {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    file: Option<String>,
}

#[cfg(test)]
#[test]
fn firefox_nulls_read_as_empty() {
    let raw: RawScript = serde_json::from_str(r#"{"id":"a","matches":["<all_urls>"],"excludeMatches":null,"includeGlobs":null,"js":[{"code":"1","file":null}],"runAt":"document_end","allFrames":false,"world":"MAIN","worldId":""}"#).unwrap();
    let script = UserScript::from_raw(raw);
    assert!(script.exclude_matches.is_empty());
    assert_eq!(script.js, vec![Source::Code("1".into())]);
    assert_eq!(script.world, World::Main);
}

/// Whether user scripts can run, and what's missing if not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Ready,
    /// `UserScripts` isn't granted; request it with `permissions::request` from a click.
    NeedsPermission,
    /// Chrome only: the user hasn't turned on "Allow User Scripts" for the extension
    /// (on its details page in `chrome://extensions`). The API appears as soon as they do.
    NeedsUserToggle,
}

pub async fn availability() -> Availability {
    match JsFuture::from(js_bridge::user_scripts_state()).await.ok().and_then(|state| state.as_string()).as_deref() {
        Some("ready") => Availability::Ready,
        Some("toggle") => Availability::NeedsUserToggle,
        _ => Availability::NeedsPermission,
    }
}

fn browser_error(err: JsValue) -> OxichromeError {
    let message = JsError::new(err).to_string();
    match message.as_str() {
        "oxichrome:needs-permission" => OxichromeError::InvalidArgument(
            "user scripts need the `UserScripts` permission: list it in `optional_permissions` and request it with `permissions::request` from a click".to_string(),
        ),
        "oxichrome:needs-toggle" => OxichromeError::InvalidArgument(
            "user scripts are off for this extension: the user has to turn on \"Allow User Scripts\" on its details page in chrome://extensions".to_string(),
        ),
        _ => {
            let duplicate = message
                .strip_prefix("Duplicate script ID '")
                .and_then(|rest| rest.strip_suffix("'"))
                .or_else(|| message.strip_prefix("User script with id \"").and_then(|rest| rest.strip_suffix("\" is already registered.")));
            if let Some(id) = duplicate {
                return OxichromeError::InvalidArgument(format!("a user script with id `{id}` is already registered; use `update` to change it"));
            }
            let missing = message
                .strip_prefix("Script with ID '")
                .and_then(|rest| rest.strip_suffix("' does not exist or is not fully registered"))
                .or_else(|| message.strip_prefix("User script with id \"").and_then(|rest| rest.strip_suffix("\" does not exist.")));
            if let Some(id) = missing {
                return OxichromeError::InvalidArgument(format!("no user script with id `{id}`"));
            }
            OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
        }
    }
}

async fn call(method: &str, args: &[JsValue]) -> Result<JsValue> {
    let promise = js_bridge::user_scripts_call(method, &args.iter().cloned().collect::<js_sys::Array>()).map_err(browser_error)?;
    JsFuture::from(promise).await.map_err(browser_error)
}

fn check_scripts(scripts: &[UserScript]) -> Result<()> {
    let mut seen: Vec<&str> = Vec::new();
    for script in scripts {
        script.check()?;
        if seen.contains(&script.id.as_str()) {
            return Err(OxichromeError::InvalidArgument(format!("user script id `{}` is given twice", script.id)));
        }
        seen.push(&script.id);
    }
    Ok(())
}

fn check_all(scripts: &[UserScript]) -> Result<JsValue> {
    check_scripts(scripts)?;
    crate::json::to_js(&scripts.iter().map(UserScript::to_raw).collect::<Vec<_>>())
}

/// Registers scripts. They keep running on matching pages, across browser restarts,
/// until they are unregistered. An id that is already registered is an error.
pub async fn register(scripts: &[UserScript]) -> Result<()> {
    if scripts.is_empty() {
        return Ok(());
    }
    call("register", &[check_all(scripts)?]).await?;
    Ok(())
}

/// Replaces registered scripts (matched by id) with these definitions.
pub async fn update(scripts: &[UserScript]) -> Result<()> {
    if scripts.is_empty() {
        return Ok(());
    }
    call("update", &[check_all(scripts)?]).await?;
    Ok(())
}

/// Every registered script.
pub async fn get_scripts() -> Result<Vec<UserScript>> {
    let raw: Vec<RawScript> = crate::json::from_js(&call("getScripts", &[]).await?)?;
    Ok(raw.into_iter().map(UserScript::from_raw).collect())
}

/// One registered script, if there is one with this id.
pub async fn get(id: &str) -> Result<Option<UserScript>> {
    let filter = crate::json::to_js(&serde_json::json!({ "ids": [id] }))?;
    let raw: Vec<RawScript> = crate::json::from_js(&call("getScripts", &[filter]).await?)?;
    Ok(raw.into_iter().map(UserScript::from_raw).next())
}

/// Unregisters these scripts; ids that aren't registered are skipped.
pub async fn unregister(ids: &[&str]) -> Result<()> {
    let registered: Vec<String> = get_scripts().await?.into_iter().map(|script| script.id).collect();
    let ids: Vec<&str> = ids.iter().copied().filter(|id| registered.iter().any(|known| known == id)).collect();
    if ids.is_empty() {
        return Ok(());
    }
    call("unregister", &[crate::json::to_js(&serde_json::json!({ "ids": ids }))?]).await?;
    Ok(())
}

/// Unregisters every script.
pub async fn unregister_all() -> Result<()> {
    call("unregister", &[]).await?;
    Ok(())
}

/// Settings for the world user scripts run in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorldConfig {
    /// Lets scripts call `chrome.runtime.sendMessage`, which reaches
    /// `runtime::on_user_script_message`. Off by default.
    pub messaging: bool,
    /// A Content Security Policy for the world, like `"script-src 'self'"`.
    pub csp: Option<String>,
}

/// Configures the world every `World::UserScript` script runs in. It applies to pages loaded afterwards.
pub async fn configure_world(config: &WorldConfig) -> Result<()> {
    let mut properties = serde_json::json!({ "messaging": config.messaging });
    if let Some(csp) = &config.csp {
        properties["csp"] = csp.clone().into();
    }
    call("configureWorld", &[crate::json::to_js(&properties)?]).await?;
    Ok(())
}

/// Puts the user script world back to the browser's defaults (no messaging).
pub async fn reset_world() -> Result<()> {
    call("resetWorldConfiguration", &[]).await?;
    Ok(())
}

/// The result of running code once in one frame.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Injected<T> {
    pub frame_id: u64,
    pub result: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawInjected {
    #[serde(default)]
    frame_id: u64,
    #[serde(default)]
    result: serde_json::Value,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

/// Runs code once in a tab's main frame, in the user script world, and returns the value
/// of its last expression.
pub async fn execute<T: DeserializeOwned>(tab_id: i32, code: &str) -> Result<Injected<T>> {
    let injection = crate::json::to_js(&serde_json::json!({ "target": { "tabId": tab_id }, "js": [{ "code": code }] }))?;
    let raw: Vec<RawInjected> = crate::json::from_js(&call("execute", &[injection]).await.map_err(|err| match err {
        OxichromeError::Js(js) => {
            let message = js.to_string();
            if message.starts_with("No tab with id") || message.starts_with("Invalid tab ID") {
                OxichromeError::InvalidArgument(format!("no tab with id {tab_id}"))
            } else {
                OxichromeError::Js(js)
            }
        }
        other => other,
    })?)?;
    let first = raw.into_iter().next().ok_or_else(|| OxichromeError::UnexpectedValue("the browser ran the code in no frame".into()))?;
    if let Some(error) = first.error {
        let text = error.as_str().map(str::to_string).unwrap_or_else(|| error.to_string());
        return Err(OxichromeError::Js(JsError::new(JsValue::from_str(&format!("the user script threw: {text}")))));
    }
    let result = serde_json::from_value(first.result).map_err(|err| OxichromeError::UnexpectedValue(format!("the script's value doesn't read as the type asked for: {err}")))?;
    Ok(Injected { frame_id: first.frame_id, result })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns() {
        assert!(check_pattern("<all_urls>").is_ok());
        assert!(check_pattern("https://*.example.com/*").is_ok());
        assert!(check_pattern("http://127.0.0.1/*").is_ok());
        assert!(check_pattern("file:///home/*").is_ok());
        assert!(check_pattern("http://127.0.0.1:8080/*").unwrap_err().contains("port"));
        assert!(check_pattern("http://[::1]/*").is_ok());
        assert!(check_pattern("example.com").is_err());
        assert!(check_pattern("wss://example.com/*").is_err());
        assert!(check_pattern("https://example.com").is_err());
        assert!(check_pattern("https://a.*.com/*").is_err());
    }

    #[test]
    fn scripts_are_checked() {
        let ok = UserScript::new("a").matches("https://example.com/*").code("1");
        assert!(ok.check().is_ok());
        assert!(UserScript::new("_a").matches("https://example.com/*").code("1").check().is_err());
        assert!(UserScript::new("a").code("1").check().is_err());
        assert!(UserScript::new("a").matches("https://example.com/*").check().is_err());
        assert!(check_scripts(&[ok.clone(), ok]).is_err());
    }
}
