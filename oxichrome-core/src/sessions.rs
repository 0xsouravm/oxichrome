//! Recently closed tabs and windows, and bringing them back.
//!
//! Needs `Sessions` in `permissions`. The URL and title of a closed tab are only
//! there with `Tabs` too (or host permission for its site).
//!
//! ```ignore
//! for closed in sessions::recently_closed(10).await? {
//!     match closed {
//!         Closed::Tab(tab) => println!("tab {:?}", tab.url),
//!         Closed::Window(window) => println!("window with {} tabs", window.tabs.len()),
//!     }
//! }
//! let back = sessions::restore_latest().await?;
//! ```

use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;

/// The most `recently_closed` can list; both browsers keep at most this many.
pub const MAX_RESULTS: u32 = 25;

/// A tab or window the user closed.
#[derive(Debug, Clone, PartialEq)]
pub enum Closed {
    Tab(ClosedTab),
    Window(ClosedWindow),
}

impl Closed {
    /// The id to pass to [`restore`].
    pub fn session_id(&self) -> &str {
        match self {
            Closed::Tab(tab) => &tab.session_id,
            Closed::Window(window) => &window.session_id,
        }
    }

    /// When it was closed, in milliseconds since the Unix epoch.
    pub fn closed_at(&self) -> f64 {
        match self {
            Closed::Tab(tab) => tab.closed_at,
            Closed::Window(window) => window.closed_at,
        }
    }
}

/// A closed tab.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedTab {
    pub session_id: String,
    /// Milliseconds since the Unix epoch, in both browsers (Chrome reports seconds).
    pub closed_at: f64,
    /// `None` without the `Tabs` permission or host permission for the site.
    pub url: Option<String>,
    pub title: Option<String>,
    /// Where it was in its window.
    pub index: u32,
    pub pinned: bool,
}

/// A closed window and the tabs it had, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedWindow {
    pub session_id: String,
    pub closed_at: f64,
    pub tabs: Vec<WindowTab>,
}

/// A tab of a closed window. It comes back with its window, so it has no session id
/// of its own (Firefox doesn't give one).
#[derive(Debug, Clone, PartialEq)]
pub struct WindowTab {
    pub url: Option<String>,
    pub title: Option<String>,
    pub pinned: bool,
}

/// What [`restore`] brought back. The tabs may still be loading: look them up with
/// `tabs::get` for their URL once they have.
#[derive(Debug, Clone, PartialEq)]
pub enum Restored {
    Tab { tab_id: i32, window_id: i32 },
    Window { window_id: i32, tab_ids: Vec<i32> },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSession {
    #[serde(default)]
    last_modified: f64,
    tab: Option<RawTab>,
    window: Option<RawWindow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTab {
    session_id: Option<String>,
    id: Option<f64>,
    window_id: Option<f64>,
    url: Option<String>,
    title: Option<String>,
    index: Option<f64>,
    pinned: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawWindow {
    session_id: Option<String>,
    id: Option<f64>,
    #[serde(default)]
    tabs: Vec<RawTab>,
}

/// Chrome reports seconds, Firefox milliseconds; no closing happened before 1973.
fn millis(value: f64) -> f64 {
    if value < 1e11 {
        value * 1000.0
    } else {
        value
    }
}

fn text(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn read_closed(raw: RawSession) -> Option<Closed> {
    let closed_at = millis(raw.last_modified);
    if let Some(tab) = raw.tab {
        return Some(Closed::Tab(ClosedTab {
            session_id: tab.session_id?,
            closed_at,
            url: text(tab.url),
            title: text(tab.title),
            index: tab.index.unwrap_or(0.0).max(0.0) as u32,
            pinned: tab.pinned.unwrap_or(false),
        }));
    }
    let window = raw.window?;
    Some(Closed::Window(ClosedWindow {
        session_id: window.session_id?,
        closed_at,
        tabs: window
            .tabs
            .into_iter()
            .map(|tab| WindowTab { url: text(tab.url), title: text(tab.title), pinned: tab.pinned.unwrap_or(false) })
            .collect(),
    }))
}

fn read_restored(raw: RawSession) -> Option<Restored> {
    if let Some(tab) = raw.tab {
        return Some(Restored::Tab { tab_id: tab.id? as i32, window_id: tab.window_id? as i32 });
    }
    let window = raw.window?;
    Some(Restored::Window {
        window_id: window.id? as i32,
        tab_ids: window.tabs.iter().filter_map(|tab| tab.id).map(|id| id as i32).collect(),
    })
}

#[derive(Debug, PartialEq)]
enum Failure {
    NoPermission,
    NothingClosed,
    NoSuchSession,
    Other(String),
}

fn classify(message: &str, session_id: Option<&str>) -> Failure {
    if message == "oxichrome:no-permission" {
        return Failure::NoPermission;
    }
    if message.contains("no recently closed sessions") || (session_id.is_none() && message.contains("Could not restore object")) {
        return Failure::NothingClosed;
    }
    if message.contains("Invalid session id") || message.contains("Invalid sessionId") || message.contains("Could not restore object") {
        return Failure::NoSuchSession;
    }
    Failure::Other(message.to_string())
}

fn browser_error(error: JsValue, session_id: Option<&str>) -> OxichromeError {
    let message = JsError::new(error).to_string();
    match classify(&message, session_id) {
        Failure::NoPermission => OxichromeError::InvalidArgument(
            "the sessions API isn't available; add `Sessions` to `permissions` in #[oxichrome::extension]".into(),
        ),
        Failure::NothingClosed => OxichromeError::InvalidArgument("nothing was closed recently, so there's nothing to restore".into()),
        Failure::NoSuchSession => OxichromeError::InvalidArgument(format!(
            "no recently closed tab or window has session id `{}` (it may have been restored already)",
            session_id.unwrap_or_default()
        )),
        Failure::Other(message) => OxichromeError::Js(JsError::new(JsValue::from_str(&message))),
    }
}

async fn call(op: &str, arg: JsValue, session_id: Option<&str>) -> Result<String> {
    let promise = js_bridge::sessions_call(op, &arg).map_err(|error| browser_error(error, session_id))?;
    let value = JsFuture::from(promise).await.map_err(|error| browser_error(error, session_id))?;
    Ok(value.as_string().unwrap_or_else(|| "null".into()))
}

fn check_max(max: u32) -> Result<()> {
    if max > MAX_RESULTS {
        return Err(OxichromeError::InvalidArgument(format!(
            "`recently_closed` lists at most {MAX_RESULTS} (`sessions::MAX_RESULTS`), not {max}"
        )));
    }
    Ok(())
}

/// The tabs and windows closed most recently, newest first, at most `max` of them
/// (up to [`MAX_RESULTS`]).
pub async fn recently_closed(max: u32) -> Result<Vec<Closed>> {
    check_max(max)?;
    if max == 0 {
        return Ok(Vec::new());
    }
    let text = call("recent", JsValue::from(max), None).await?;
    let raw: Vec<RawSession> = serde_json::from_str(&text).map_err(|err| OxichromeError::Serde(err.to_string()))?;
    Ok(raw.into_iter().filter_map(read_closed).collect())
}

/// Reopens the closed tab or window with this session id.
pub async fn restore(session_id: &str) -> Result<Restored> {
    if session_id.trim().is_empty() {
        return Err(OxichromeError::InvalidArgument("the session id is empty; take one from `recently_closed`".into()));
    }
    restored(call("restore", JsValue::from_str(session_id), Some(session_id)).await?)
}

/// Reopens whatever was closed last.
pub async fn restore_latest() -> Result<Restored> {
    restored(call("restore", JsValue::UNDEFINED, None).await?)
}

fn restored(text: String) -> Result<Restored> {
    let raw: RawSession = serde_json::from_str(&text).map_err(|err| OxichromeError::Serde(err.to_string()))?;
    read_restored(raw).ok_or_else(|| OxichromeError::UnexpectedValue(format!("the browser restored something oxichrome can't read: {text}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_reads_the_same_in_both_browsers() {
        let chrome = r#"[{"lastModified":1795221146,"window":{"sessionId":"28","tabs":[{"sessionId":"29","url":"http://a/","title":"a","index":0,"pinned":true,"windowId":0}]}},
            {"lastModified":1795221144,"tab":{"sessionId":"23","url":"http://b/","title":"b","index":2,"pinned":false,"windowId":0}}]"#;
        let firefox = r#"[{"lastModified":1795221146000,"window":{"sessionId":"28","tabs":[{"sessionId":"undefined","index":0,"windowId":null,"url":"http://a/","title":"a","pinned":true}]}},
            {"lastModified":1795221144000,"tab":{"sessionId":"23","index":2,"windowId":3,"url":"http://b/","title":"b","pinned":false}}]"#;
        let read = |text: &str| serde_json::from_str::<Vec<RawSession>>(text).unwrap().into_iter().filter_map(read_closed).collect::<Vec<_>>();
        let a = read(chrome);
        assert_eq!(a, read(firefox));
        assert_eq!(a[0].closed_at(), 1795221146000.0);
        assert_eq!(a[0], Closed::Window(ClosedWindow { session_id: "28".into(), closed_at: 1795221146000.0, tabs: vec![WindowTab { url: Some("http://a/".into()), title: Some("a".into()), pinned: true }] }));
        assert_eq!(a[1].session_id(), "23");
    }

    #[test]
    fn no_tabs_permission_means_no_url() {
        let raw: RawSession = serde_json::from_str(r#"{"lastModified":1,"tab":{"sessionId":"1","index":1}}"#).unwrap();
        let Some(Closed::Tab(tab)) = read_closed(raw) else { panic!() };
        assert_eq!((tab.url, tab.title), (None, None));
    }

    #[test]
    fn restored_gives_ids() {
        let raw: RawSession = serde_json::from_str(r#"{"lastModified":1,"window":{"id":45,"tabs":[{"id":7,"windowId":45},{"id":8,"windowId":45}]}}"#).unwrap();
        assert_eq!(read_restored(raw), Some(Restored::Window { window_id: 45, tab_ids: vec![7, 8] }));
        let raw: RawSession = serde_json::from_str(r#"{"lastModified":1,"tab":{"id":6,"windowId":3,"url":"about:blank"}}"#).unwrap();
        assert_eq!(read_restored(raw), Some(Restored::Tab { tab_id: 6, window_id: 3 }));
    }

    #[test]
    fn errors_read_the_same_in_both_browsers() {
        assert_eq!(classify("There are no recently closed sessions.", None), Failure::NothingClosed);
        assert_eq!(classify("Could not restore object using sessionId undefined.", None), Failure::NothingClosed);
        assert_eq!(classify("Invalid session id: \"nope\".", Some("nope")), Failure::NoSuchSession);
        assert_eq!(classify("Invalid sessionId: nope.", Some("nope")), Failure::NoSuchSession);
        assert_eq!(classify("Could not restore object using sessionId 999999.", Some("999999")), Failure::NoSuchSession);
        assert_eq!(classify("oxichrome:no-permission", None), Failure::NoPermission);
        assert!(check_max(26).is_err() && check_max(25).is_ok());
    }
}
