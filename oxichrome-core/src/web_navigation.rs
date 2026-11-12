use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameId(pub u64);

impl FrameId {
    pub const MAIN: FrameId = FrameId(0);
}

impl std::fmt::Display for FrameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Navigation {
    pub tab_id: i32,
    pub frame_id: FrameId,
    pub parent_frame_id: Option<FrameId>,
    pub url: String,
    pub time: f64,
    pub document_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    pub tab_id: i32,
    pub frame_id: FrameId,
    pub parent_frame_id: Option<FrameId>,
    pub url: String,
    pub time: f64,
    pub document_id: Option<String>,
    pub transition: Transition,
    pub qualifiers: Vec<Qualifier>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub tab_id: i32,
    pub frame_id: FrameId,
    pub parent_frame_id: Option<FrameId>,
    pub url: String,
    pub time: f64,
    pub error: NavError,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub source_tab_id: i32,
    pub source_frame_id: FrameId,
    pub tab_id: i32,
    pub url: String,
    pub time: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Replaced {
    pub replaced_tab_id: i32,
    pub tab_id: i32,
    pub time: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub frame_id: FrameId,
    pub parent_frame_id: Option<FrameId>,
    pub url: String,
    pub document_id: Option<String>,
}

macro_rules! top_level {
    ($($ty:ty),*) => {
        $(impl $ty {
            pub fn is_top_level(&self) -> bool {
                self.parent_frame_id.is_none()
            }
        })*
    };
}

top_level!(Navigation, Commit, Failure, Frame);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    Link,
    Typed,
    AutoBookmark,
    AutoSubframe,
    ManualSubframe,
    Generated,
    StartPage,
    FormSubmit,
    Reload,
    Keyword,
    KeywordGenerated,
    Other(String),
}

impl Transition {
    fn parse(text: &str) -> Self {
        match text {
            "link" => Transition::Link,
            "typed" => Transition::Typed,
            "auto_bookmark" => Transition::AutoBookmark,
            "auto_subframe" => Transition::AutoSubframe,
            "manual_subframe" => Transition::ManualSubframe,
            "generated" => Transition::Generated,
            "start_page" => Transition::StartPage,
            "form_submit" => Transition::FormSubmit,
            "reload" => Transition::Reload,
            "keyword" => Transition::Keyword,
            "keyword_generated" => Transition::KeywordGenerated,
            other => Transition::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Qualifier {
    ClientRedirect,
    ServerRedirect,
    ForwardBack,
    FromAddressBar,
    Other(String),
}

impl Qualifier {
    fn parse(text: &str) -> Self {
        match text {
            "client_redirect" => Qualifier::ClientRedirect,
            "server_redirect" => Qualifier::ServerRedirect,
            "forward_back" => Qualifier::ForwardBack,
            "from_address_bar" => Qualifier::FromAddressBar,
            other => Qualifier::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavError {
    Aborted,
    NameNotResolved,
    ConnectionRefused,
    ConnectionReset,
    TimedOut,
    Offline,
    BlockedPort,
    Certificate,
    Blocked,
    Other(String),
}

const FIREFOX_NETWORK: u64 = 0x804B_0000;
const FIREFOX_SECURITY: u64 = 0x805A_0000;

impl NavError {
    fn parse(text: &str) -> Self {
        if let Some(name) = text.strip_prefix("net::ERR_") {
            return match name {
                "ABORTED" => NavError::Aborted,
                "NAME_NOT_RESOLVED" | "NAME_RESOLUTION_FAILED" => NavError::NameNotResolved,
                "CONNECTION_REFUSED" => NavError::ConnectionRefused,
                "CONNECTION_RESET" | "CONNECTION_CLOSED" | "EMPTY_RESPONSE" => NavError::ConnectionReset,
                "TIMED_OUT" | "CONNECTION_TIMED_OUT" => NavError::TimedOut,
                "INTERNET_DISCONNECTED" => NavError::Offline,
                "UNSAFE_PORT" => NavError::BlockedPort,
                "BLOCKED_BY_CLIENT" | "BLOCKED_BY_ADMINISTRATOR" | "BLOCKED_BY_RESPONSE" | "BLOCKED_BY_ORB" => NavError::Blocked,
                cert if cert.starts_with("CERT_") || cert.starts_with("SSL_") => NavError::Certificate,
                _ => NavError::Other(text.to_string()),
            };
        }
        if let Some(code) = text.strip_prefix("Error code ").and_then(|code| code.trim().parse::<u64>().ok()) {
            if code & 0xFFFF_0000 == FIREFOX_SECURITY {
                return NavError::Certificate;
            }
            if code & 0xFFFF_0000 == FIREFOX_NETWORK {
                return match code - FIREFOX_NETWORK {
                    0x02 => NavError::Aborted,
                    0x1E | 0x2A => NavError::NameNotResolved,
                    0x0D | 0x48 => NavError::ConnectionRefused,
                    0x14 | 0x47 => NavError::ConnectionReset,
                    0x0E => NavError::TimedOut,
                    0x10 => NavError::Offline,
                    0x13 => NavError::BlockedPort,
                    _ => NavError::Other(text.to_string()),
                };
            }
        }
        NavError::Other(text.to_string())
    }
}

impl std::fmt::Display for NavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NavError::Aborted => f.write_str("the navigation was stopped before the page loaded"),
            NavError::NameNotResolved => f.write_str("the site's address could not be found"),
            NavError::ConnectionRefused => f.write_str("the server refused the connection"),
            NavError::ConnectionReset => f.write_str("the connection was closed before the page loaded"),
            NavError::TimedOut => f.write_str("the server took too long to respond"),
            NavError::Offline => f.write_str("the computer is offline"),
            NavError::BlockedPort => f.write_str("the browser blocks this port"),
            NavError::Certificate => f.write_str("the site's certificate isn't trusted"),
            NavError::Blocked => f.write_str("the page was blocked"),
            NavError::Other(raw) => f.write_str(raw),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    tab_id: Option<i32>,
    frame_id: Option<f64>,
    parent_frame_id: Option<f64>,
    url: Option<String>,
    time_stamp: Option<f64>,
    document_id: Option<String>,
    transition_type: Option<String>,
    transition_qualifiers: Option<Vec<String>>,
    error: Option<String>,
    source_tab_id: Option<i32>,
    source_frame_id: Option<f64>,
    replaced_tab_id: Option<i32>,
}

fn frame_id(value: Option<f64>, what: &str) -> Result<FrameId> {
    match value {
        Some(number) if number.fract() == 0.0 && (0.0..=crate::json::MAX_SAFE_INTEGER as f64).contains(&number) => Ok(FrameId(number as u64)),
        other => Err(OxichromeError::UnexpectedValue(format!("expected a {what}, got {other:?}"))),
    }
}

fn parent(value: Option<f64>) -> Result<Option<FrameId>> {
    match value {
        None => Ok(None),
        Some(number) if number < 0.0 => Ok(None),
        some => frame_id(some, "parent frame id").map(Some),
    }
}

fn need<T>(value: Option<T>, what: &str) -> Result<T> {
    value.ok_or_else(|| OxichromeError::UnexpectedValue(format!("the navigation details have no {what}")))
}

impl Raw {
    fn read(value: &JsValue) -> Result<Self> {
        crate::json::from_js(value)
    }

    fn navigation(self) -> Result<Navigation> {
        Ok(Navigation {
            tab_id: need(self.tab_id, "tab id")?,
            frame_id: frame_id(self.frame_id, "frame id")?,
            parent_frame_id: parent(self.parent_frame_id)?,
            url: need(self.url, "url")?,
            time: need(self.time_stamp, "time")?,
            document_id: self.document_id,
        })
    }
}

impl FromEventArg for Navigation {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        Raw::read(&value)?.navigation()
    }
}

impl FromEventArg for Commit {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let mut raw = Raw::read(&value)?;
        let transition = Transition::parse(raw.transition_type.take().as_deref().unwrap_or("link"));
        let qualifiers = raw.transition_qualifiers.take().unwrap_or_default().iter().map(|q| Qualifier::parse(q)).collect();
        let nav = raw.navigation()?;
        Ok(Commit {
            tab_id: nav.tab_id,
            frame_id: nav.frame_id,
            parent_frame_id: nav.parent_frame_id,
            url: nav.url,
            time: nav.time,
            document_id: nav.document_id,
            transition,
            qualifiers,
        })
    }
}

impl FromEventArg for Failure {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let mut raw = Raw::read(&value)?;
        let error = NavError::parse(&raw.error.take().unwrap_or_default());
        let nav = raw.navigation()?;
        Ok(Failure {
            tab_id: nav.tab_id,
            frame_id: nav.frame_id,
            parent_frame_id: nav.parent_frame_id,
            url: nav.url,
            time: nav.time,
            error,
        })
    }
}

impl FromEventArg for Target {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let raw = Raw::read(&value)?;
        Ok(Target {
            source_tab_id: need(raw.source_tab_id, "source tab id")?,
            source_frame_id: frame_id(raw.source_frame_id, "source frame id")?,
            tab_id: need(raw.tab_id, "tab id")?,
            url: need(raw.url, "url")?,
            time: need(raw.time_stamp, "time")?,
        })
    }
}

impl FromEventArg for Replaced {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let raw = Raw::read(&value)?;
        Ok(Replaced {
            replaced_tab_id: need(raw.replaced_tab_id, "replaced tab id")?,
            tab_id: need(raw.tab_id, "tab id")?,
            time: need(raw.time_stamp, "time")?,
        })
    }
}

impl EventArg<events::WebNavigationOnBeforeNavigate, 1> for Navigation {}
impl EventArg<events::WebNavigationOnCommitted, 1> for Commit {}
impl EventArg<events::WebNavigationOnDomContentLoaded, 1> for Navigation {}
impl EventArg<events::WebNavigationOnCompleted, 1> for Navigation {}
impl EventArg<events::WebNavigationOnErrorOccurred, 1> for Failure {}
impl EventArg<events::WebNavigationOnHistoryStateUpdated, 1> for Commit {}
impl EventArg<events::WebNavigationOnReferenceFragmentUpdated, 1> for Commit {}
impl EventArg<events::WebNavigationOnCreatedNavigationTarget, 1> for Target {}
impl EventArg<events::WebNavigationOnTabReplaced, 1> for Replaced {}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    if message == "oxichrome:no-permission" {
        return OxichromeError::InvalidArgument(
            "the webNavigation API isn't available; add `WebNavigation` to `permissions` in #[oxichrome::extension] (if it's optional, request it first)".to_string(),
        );
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

fn gone(message: &str) -> bool {
    message.starts_with("Invalid tab ID") || message.starts_with("No frame found")
}

async fn call(method: &str, arg: JsValue) -> Result<Option<JsValue>> {
    let promise = js_bridge::web_navigation_call(method, &arg).map_err(browser_error)?;
    match JsFuture::from(promise).await {
        Ok(value) if value.is_null() || value.is_undefined() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(error) => match browser_error(error) {
            OxichromeError::Js(js) if gone(&js.to_string()) => Ok(None),
            other => Err(other),
        },
    }
}

fn tab_arg(tab_id: i32) -> Result<()> {
    if tab_id < 0 {
        return Err(OxichromeError::InvalidArgument(format!("the tab id {tab_id} can't be negative")));
    }
    Ok(())
}

fn read_frame(raw: Raw, frame: Option<FrameId>) -> Result<Frame> {
    Ok(Frame {
        frame_id: match frame {
            Some(frame) => frame,
            None => frame_id(raw.frame_id, "frame id")?,
        },
        parent_frame_id: parent(raw.parent_frame_id)?,
        url: need(raw.url, "url")?,
        document_id: raw.document_id,
    })
}

fn frame_query(tab_id: i32, frame: Option<FrameId>) -> Result<JsValue> {
    if let Some(frame) = frame {
        if frame.0 > crate::json::MAX_SAFE_INTEGER {
            return Err(OxichromeError::InvalidArgument(format!("the frame id {frame} is larger than any browser uses")));
        }
    }
    let object = js_sys::Object::new();
    js_sys::Reflect::set(&object, &"tabId".into(), &JsValue::from(tab_id))?;
    if let Some(frame) = frame {
        js_sys::Reflect::set(&object, &"frameId".into(), &JsValue::from(frame.0 as f64))?;
    }
    Ok(object.into())
}

pub async fn frame(tab_id: i32, frame_id: FrameId) -> Result<Option<Frame>> {
    tab_arg(tab_id)?;
    match call("getFrame", frame_query(tab_id, Some(frame_id))?).await? {
        Some(value) => read_frame(Raw::read(&value)?, Some(frame_id)).map(Some),
        None => Ok(None),
    }
}

pub async fn frames(tab_id: i32) -> Result<Option<Vec<Frame>>> {
    tab_arg(tab_id)?;
    let Some(value) = call("getAllFrames", frame_query(tab_id, None)?).await? else {
        return Ok(None);
    };
    let raw: Vec<Raw> = crate::json::from_js(&value)?;
    let mut frames = raw.into_iter().map(|raw| read_frame(raw, None)).collect::<Result<Vec<_>>>()?;
    frames.sort_by_key(|frame| frame.frame_id);
    Ok(Some(frames))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_mean_the_same_in_both_browsers() {
        for (chrome, firefox, want) in [
            ("net::ERR_ABORTED", "Error code 2152398850", NavError::Aborted),
            ("net::ERR_NAME_NOT_RESOLVED", "Error code 2152398878", NavError::NameNotResolved),
            ("net::ERR_CONNECTION_REFUSED", "Error code 2152398861", NavError::ConnectionRefused),
            ("net::ERR_CONNECTION_RESET", "Error code 2152398868", NavError::ConnectionReset),
            ("net::ERR_TIMED_OUT", "Error code 2152398862", NavError::TimedOut),
            ("net::ERR_INTERNET_DISCONNECTED", "Error code 2152398864", NavError::Offline),
            ("net::ERR_UNSAFE_PORT", "Error code 2152398867", NavError::BlockedPort),
            ("net::ERR_CERT_AUTHORITY_INVALID", "Error code 2153390067", NavError::Certificate),
        ] {
            assert_eq!(NavError::parse(chrome), want, "{chrome}");
            assert_eq!(NavError::parse(firefox), want, "{firefox}");
        }
        assert_eq!(NavError::parse("net::ERR_CONNECTION_TIMED_OUT"), NavError::TimedOut);
        assert_eq!(NavError::parse("net::ERR_BLOCKED_BY_CLIENT"), NavError::Blocked);
        assert_eq!(NavError::parse("net::ERR_SOMETHING_NEW"), NavError::Other("net::ERR_SOMETHING_NEW".into()));
        assert_eq!(NavError::parse("Error code 2152398999"), NavError::Other("Error code 2152398999".into()));
        assert_eq!(NavError::parse("Error code nope"), NavError::Other("Error code nope".into()));
        assert_eq!(NavError::parse(""), NavError::Other(String::new()));
    }

    #[test]
    fn transitions_and_qualifiers_parse() {
        assert_eq!(Transition::parse("auto_subframe"), Transition::AutoSubframe);
        assert_eq!(Transition::parse("keyword_generated"), Transition::KeywordGenerated);
        assert_eq!(Transition::parse("new_kind"), Transition::Other("new_kind".into()));
        assert_eq!(Qualifier::parse("forward_back"), Qualifier::ForwardBack);
        assert_eq!(Qualifier::parse("server_redirect"), Qualifier::ServerRedirect);
        assert_eq!(Qualifier::parse("x"), Qualifier::Other("x".into()));
    }

    #[test]
    fn frame_ids_cover_firefox_sizes() {
        assert_eq!(frame_id(Some(15032385537.0), "frame id").unwrap(), FrameId(15032385537));
        assert_eq!(parent(Some(-1.0)).unwrap(), None);
        assert_eq!(parent(None).unwrap(), None);
        assert_eq!(parent(Some(0.0)).unwrap(), Some(FrameId::MAIN));
        assert!(frame_id(Some(1.5), "frame id").is_err());
        assert!(frame_id(Some(-2.0), "frame id").is_err());
        assert!(frame_id(None, "frame id").is_err());
    }
}
