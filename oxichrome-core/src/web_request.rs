//! Watch the requests the browser makes, typed the same in Chrome and Firefox.
//!
//! Handlers go on `#[oxichrome::on(web_request::...)]` with an optional filter:
//!
//! ```ignore
//! #[oxichrome::on(web_request::on_completed, urls = ["https://example.com/*"], types = [MainFrame, Script])]
//! async fn done(request: web_request::Request, response: web_request::Response) {}
//! ```
//!
//! Every request that starts ends in exactly one `on_completed` or `on_error_occurred`.
//! Watching is all these events do: to block or change requests in both browsers, use
//! [`declarative_net_request`](crate::declarative_net_request).

use std::collections::BTreeMap;

use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;
use crate::web_navigation::FrameId;

/// What a request loads. Firefox's finer kinds are folded into these
/// (`imageset` is `Image`, `beacon` is `Ping`, `object_subrequest` is `Object`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceType {
    MainFrame,
    SubFrame,
    Stylesheet,
    Script,
    Image,
    Font,
    Object,
    XmlHttpRequest,
    Ping,
    CspReport,
    Media,
    WebSocket,
    /// Anything else, with the browser's name for it.
    Other(String),
}

impl ResourceType {
    fn parse(kind: &str) -> Self {
        match kind {
            "main_frame" => ResourceType::MainFrame,
            "sub_frame" => ResourceType::SubFrame,
            "stylesheet" => ResourceType::Stylesheet,
            "script" => ResourceType::Script,
            "image" | "imageset" => ResourceType::Image,
            "font" => ResourceType::Font,
            "object" | "object_subrequest" => ResourceType::Object,
            "xmlhttprequest" => ResourceType::XmlHttpRequest,
            "ping" | "beacon" => ResourceType::Ping,
            "csp_report" => ResourceType::CspReport,
            "media" => ResourceType::Media,
            "websocket" => ResourceType::WebSocket,
            other => ResourceType::Other(other.to_string()),
        }
    }
}

/// The request, passed first to every `web_request` event.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Request {
    /// Stays the same through redirects and auth, so it ties the events of one request together.
    pub id: String,
    pub url: String,
    pub method: String,
    pub kind: ResourceType,
    /// The tab that made it; `None` for the background and other requests outside a tab.
    pub tab_id: Option<i32>,
    pub frame_id: FrameId,
    pub parent_frame_id: Option<FrameId>,
    /// The origin (`https://example.com`) of the page or extension that made the request,
    /// when the browser says.
    pub initiator: Option<String>,
    /// Milliseconds since the epoch.
    pub time: f64,
}

fn get(value: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(value, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

fn string(value: &JsValue, key: &str) -> Option<String> {
    get(value, key).as_string()
}

fn number(value: &JsValue, key: &str) -> Option<f64> {
    get(value, key).as_f64()
}

fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    (!host.is_empty()).then(|| format!("{scheme}://{host}"))
}

fn details(value: JsValue) -> Result<JsValue> {
    if value.is_object() {
        Ok(value)
    } else {
        Err(OxichromeError::UnexpectedValue(format!("expected request details, got {value:?}")))
    }
}

impl FromEventArg for Request {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let d = details(value)?;
        let frame = |key: &str| number(&d, key).filter(|id| *id >= 0.0).map(|id| FrameId(id as u64));
        let initiator = string(&d, "initiator")
            .filter(|origin| origin != "null")
            .or_else(|| string(&d, "originUrl").as_deref().and_then(origin_of));
        Ok(Request {
            id: string(&d, "requestId").unwrap_or_default(),
            url: string(&d, "url").unwrap_or_default(),
            method: string(&d, "method").unwrap_or_default(),
            kind: ResourceType::parse(&string(&d, "type").unwrap_or_default()),
            tab_id: number(&d, "tabId").filter(|id| *id >= 0.0).map(|id| id as i32),
            frame_id: frame("frameId").unwrap_or(FrameId::MAIN),
            parent_frame_id: frame("parentFrameId"),
            initiator,
            time: number(&d, "timeStamp").unwrap_or_default(),
        })
    }
}

/// HTTP headers in the order the browser gives them.
///
/// Chrome leaves out some headers (like `Set-Cookie`) unless an extension asks for them;
/// oxichrome asks, so both browsers report them. Firefox also reports `Host` in request headers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers(pub Vec<(String, String)>);

impl Headers {
    /// The first value of a header, matching its name in any case.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    /// Every value of a header (`Set-Cookie` can repeat).
    pub fn get_all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.0.iter().filter(move |(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value.as_str()))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn read(list: &JsValue) -> Self {
        let Some(list) = list.dyn_ref::<js_sys::Array>() else { return Headers::default() };
        Headers(
            list.iter()
                .map(|header| {
                    let name = string(&header, "name").unwrap_or_default();
                    let value = string(&header, "value").unwrap_or_else(|| {
                        let bytes: Vec<u8> = get(&header, "binaryValue")
                            .dyn_ref::<js_sys::Array>()
                            .map(|bytes| bytes.iter().filter_map(|b| b.as_f64()).map(|b| b as u8).collect())
                            .unwrap_or_default();
                        String::from_utf8_lossy(&bytes).into_owned()
                    });
                    (name, value)
                })
                .collect(),
        )
    }
}

impl FromEventArg for Headers {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let d = details(value)?;
        Ok(Headers::read(&get(&d, "requestHeaders")))
    }
}

/// The body of a `POST` or `PUT`, in `on_before_request`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// A form (`application/x-www-form-urlencoded` or `multipart/form-data`), each field with its values.
    Form(BTreeMap<String, Vec<String>>),
    /// Anything else, as bytes.
    Raw(Vec<u8>),
}

impl FromEventArg for Option<Body> {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let d = details(value)?;
        let body = get(&d, "requestBody");
        if !body.is_object() {
            return Ok(None);
        }
        let form = get(&body, "formData");
        if form.is_object() {
            let mut fields = BTreeMap::new();
            for key in js_sys::Object::keys(form.unchecked_ref::<js_sys::Object>()).iter() {
                let name = key.as_string().unwrap_or_default();
                let values = get(&form, &name)
                    .dyn_ref::<js_sys::Array>()
                    .map(|values| values.iter().filter_map(|v| v.as_string()).collect())
                    .unwrap_or_default();
                fields.insert(name, values);
            }
            return Ok(Some(Body::Form(fields)));
        }
        let Some(parts) = get(&body, "raw").dyn_into::<js_sys::Array>().ok() else { return Ok(None) };
        let mut bytes = Vec::new();
        for part in parts.iter() {
            let buffer = get(&part, "bytes");
            if buffer.is_instance_of::<js_sys::ArrayBuffer>() {
                bytes.extend(js_sys::Uint8Array::new(&buffer).to_vec());
            }
        }
        Ok(Some(Body::Raw(bytes)))
    }
}

/// The response, in `on_headers_received`, `on_response_started` and `on_completed`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Response {
    pub status: u16,
    /// Like `HTTP/1.1 200 OK`.
    pub status_line: String,
    pub headers: Headers,
    /// The server's IP address, once the browser knows it.
    pub ip: Option<String>,
    pub from_cache: bool,
}

impl FromEventArg for Response {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let d = details(value)?;
        Ok(Response {
            status: number(&d, "statusCode").unwrap_or_default() as u16,
            status_line: string(&d, "statusLine").unwrap_or_default(),
            headers: Headers::read(&get(&d, "responseHeaders")),
            ip: string(&d, "ip").filter(|ip| !ip.is_empty()),
            from_cache: get(&d, "fromCache").as_bool().unwrap_or(false),
        })
    }
}

/// A redirect, in `on_before_redirect`. The request continues at `to` with the same id.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Redirect {
    pub to: String,
    pub status: u16,
    pub headers: Headers,
}

impl FromEventArg for Redirect {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let d = details(value)?;
        Ok(Redirect {
            to: string(&d, "redirectUrl").unwrap_or_default(),
            status: number(&d, "statusCode").unwrap_or_default() as u16,
            headers: Headers::read(&get(&d, "responseHeaders")),
        })
    }
}

/// Why a request failed, named the same in both browsers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NetErrorKind {
    /// Cancelled: the page went away, the fetch was aborted, or an extension cancelled it.
    Aborted,
    ConnectionRefused,
    /// The host name doesn't resolve.
    NameNotResolved,
    TimedOut,
    /// Blocked by an extension rule or the browser. Chrome only: Firefox reports a blocked
    /// request as `Aborted`.
    Blocked,
    /// The server kept asking for credentials.
    TooManyAuthAttempts,
    Other,
}

/// A failed request, in `on_error_occurred`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct NetError {
    pub kind: NetErrorKind,
    /// The browser's own text, like `net::ERR_CONNECTION_REFUSED` or `NS_ERROR_CONNECTION_REFUSED`.
    pub message: String,
}

impl NetErrorKind {
    fn parse(message: &str) -> Self {
        let code = message.trim_start_matches("net::");
        match code {
            "ERR_ABORTED" | "NS_ERROR_ABORT" | "NS_BINDING_ABORTED" => NetErrorKind::Aborted,
            "ERR_CONNECTION_REFUSED" | "NS_ERROR_CONNECTION_REFUSED" => NetErrorKind::ConnectionRefused,
            "ERR_NAME_NOT_RESOLVED" | "NS_ERROR_UNKNOWN_HOST" => NetErrorKind::NameNotResolved,
            "ERR_TIMED_OUT" | "ERR_CONNECTION_TIMED_OUT" | "NS_ERROR_NET_TIMEOUT" => NetErrorKind::TimedOut,
            "ERR_TOO_MANY_RETRIES" => NetErrorKind::TooManyAuthAttempts,
            _ if code.starts_with("ERR_BLOCKED") || code.starts_with("NS_ERROR_BLOCKED") || code.contains("BLOCKED_BY") => NetErrorKind::Blocked,
            _ => NetErrorKind::Other,
        }
    }
}

impl FromEventArg for NetError {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let d = details(value)?;
        let message = string(&d, "error").unwrap_or_default();
        let kind = if get(&d, "__oxichrome_auth_gave_up").as_bool() == Some(true) {
            NetErrorKind::TooManyAuthAttempts
        } else {
            NetErrorKind::parse(&message)
        };
        Ok(NetError { kind, message })
    }
}

/// A server or proxy asking for credentials, in `on_auth_required`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AuthChallenge {
    /// Like `basic` or `digest`.
    pub scheme: String,
    pub realm: Option<String>,
    pub host: String,
    pub port: u16,
    /// A proxy is asking, not the site.
    pub is_proxy: bool,
    /// 1 the first time this request is challenged, 2 after the first credentials were refused, and so on.
    pub attempt: u32,
    pub status: u16,
    pub headers: Headers,
}

impl FromEventArg for AuthChallenge {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let d = details(value)?;
        let challenger = get(&d, "challenger");
        Ok(AuthChallenge {
            scheme: string(&d, "scheme").unwrap_or_default().to_ascii_lowercase(),
            realm: string(&d, "realm").filter(|realm| !realm.is_empty()),
            host: string(&challenger, "host").unwrap_or_default(),
            port: number(&challenger, "port").unwrap_or_default() as u16,
            is_proxy: get(&d, "isProxy").as_bool().unwrap_or(false),
            attempt: number(&d, "__oxichrome_attempt").unwrap_or(1.0) as u32,
            status: number(&d, "statusCode").unwrap_or_default() as u16,
            headers: Headers::read(&get(&d, "responseHeaders")),
        })
    }
}

/// How an `on_auth_required` handler answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Auth {
    /// Send these credentials. When they are refused, the handler is asked again with a
    /// higher `attempt`; after [`MAX_AUTH_ATTEMPTS`] refusals oxichrome cancels the request.
    Credentials { username: String, password: String },
    /// Give up. Chrome then finishes the request with the server's 401 (or 407) response;
    /// Firefox fails it, and `on_error_occurred` reports `Aborted`.
    Cancel,
    /// Let the browser ask the user, as if the extension weren't there.
    Prompt,
}

impl Auth {
    pub fn credentials(username: impl Into<String>, password: impl Into<String>) -> Self {
        Auth::Credentials { username: username.into(), password: password.into() }
    }
}

/// How many times oxichrome sends credentials for one request before cancelling it
/// (Chrome would retry about 30 times, Firefox forever).
pub const MAX_AUTH_ATTEMPTS: u32 = 3;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthReply<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_credentials: Option<Credentials<'a>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    cancel: bool,
}

#[derive(Serialize)]
struct Credentials<'a> {
    username: &'a str,
    password: &'a str,
}

#[doc(hidden)]
pub fn __auth_answer(answer: &Auth) -> std::result::Result<JsValue, JsValue> {
    let reply = match answer {
        Auth::Credentials { username, password } => AuthReply { auth_credentials: Some(Credentials { username, password }), cancel: false },
        Auth::Cancel => AuthReply { auth_credentials: None, cancel: true },
        Auth::Prompt => AuthReply { auth_credentials: None, cancel: false },
    };
    crate::json::to_js(&reply).map_err(|err| JsValue::from_str(&err.to_string()))
}

macro_rules! args {
    ($($marker:ident: $second:ty),* $(,)?) => {
        $(
            impl EventArg<events::$marker, 1> for Request {}
            impl EventArg<events::$marker, 2> for $second {}
        )*
    };
}

args! {
    WebRequestOnBeforeRequest: Option<Body>,
    WebRequestOnBeforeSendHeaders: Headers,
    WebRequestOnSendHeaders: Headers,
    WebRequestOnHeadersReceived: Response,
    WebRequestOnResponseStarted: Response,
    WebRequestOnBeforeRedirect: Redirect,
    WebRequestOnCompleted: Response,
    WebRequestOnErrorOccurred: NetError,
    WebRequestOnAuthRequired: AuthChallenge,
}

/// Tells the browser that handlers changed, so it drops what it cached about them.
/// Chrome allows 20 calls in 10 minutes.
pub async fn handler_behavior_changed() -> Result<()> {
    let promise = js_bridge::web_request_handler_behavior_changed().map_err(no_permission)?;
    JsFuture::from(promise).await.map_err(no_permission)?;
    Ok(())
}

fn no_permission(err: JsValue) -> OxichromeError {
    let message = JsError::new(err).to_string();
    if message == "oxichrome:no-permission" {
        return OxichromeError::InvalidArgument(
            "the webRequest API isn't available; add `WebRequest` to `permissions` in #[oxichrome::extension] (if it's optional, request it first)".to_string(),
        );
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_read_the_same_in_both_browsers() {
        for (chrome, firefox, kind) in [
            ("net::ERR_CONNECTION_REFUSED", "NS_ERROR_CONNECTION_REFUSED", NetErrorKind::ConnectionRefused),
            ("net::ERR_ABORTED", "NS_ERROR_ABORT", NetErrorKind::Aborted),
            ("net::ERR_NAME_NOT_RESOLVED", "NS_ERROR_UNKNOWN_HOST", NetErrorKind::NameNotResolved),
            ("net::ERR_BLOCKED_BY_CLIENT", "NS_ERROR_BLOCKED_BY_POLICY", NetErrorKind::Blocked),
        ] {
            assert_eq!(NetErrorKind::parse(chrome), kind);
            assert_eq!(NetErrorKind::parse(firefox), kind);
        }
        assert_eq!(NetErrorKind::parse("something new"), NetErrorKind::Other);
    }

    #[test]
    fn firefox_kinds_fold_into_shared_ones() {
        assert_eq!(ResourceType::parse("imageset"), ResourceType::Image);
        assert_eq!(ResourceType::parse("beacon"), ResourceType::Ping);
        assert_eq!(ResourceType::parse("webbundle"), ResourceType::Other("webbundle".into()));
    }

    #[test]
    fn origins() {
        assert_eq!(origin_of("moz-extension://abc/probe.html").as_deref(), Some("moz-extension://abc"));
        assert_eq!(origin_of("http://127.0.0.1:8000/a?b").as_deref(), Some("http://127.0.0.1:8000"));
        assert_eq!(origin_of("about:blank"), None);
    }

    #[test]
    fn headers_match_any_case() {
        let headers = Headers(vec![("Set-Cookie".into(), "a=1".into()), ("set-cookie".into(), "b=2".into())]);
        assert_eq!(headers.get("SET-COOKIE"), Some("a=1"));
        assert_eq!(headers.get_all("set-cookie").collect::<Vec<_>>(), vec!["a=1", "b=2"]);
    }
}
