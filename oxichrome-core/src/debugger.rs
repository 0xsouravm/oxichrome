//! Chrome's DevTools protocol, driven from the extension: attach to a tab, a worker or another
//! extension and send it any protocol command. Chrome only, with `Debugger` in `permissions`
//! (Chrome can't ask for it later). While attached, Chrome shows a bar saying the extension is
//! debugging the browser.
//!
//! Commands and events are the ones in the protocol reference,
//! <https://chromedevtools.github.io/devtools-protocol/>.
//!
//! ```ignore
//! use oxichrome::debugger::{self, Target};
//!
//! let tab = debugger::attach(Target::Tab(tab_id)).await?;
//! let shot: Shot = tab.send("Page.captureScreenshot", json!({ "format": "png" })).await?;
//! tab.detach().await?;
//! ```

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::JsValue;

use crate::call::{self, Only};
use crate::error::{OxichromeError, Result};
pub use crate::runtime::Listener;

const API: &str = "debugger";
const INSTEAD: &str = "Firefox doesn't let extensions use its devtools protocol; use `scripting` or `devtools` there";
/// The protocol version oxichrome asks for; Chrome has answered to 1.3 for years.
const VERSION: &str = "1.3";

/// What to attach to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    Tab(i32),
    /// Another extension's background, by its extension id.
    Extension(String),
    /// Any target by the id [`targets`] gives, such as a service worker.
    Id(String),
}

/// An attached target, or a session inside it (see [`Session::child`]). Cheap to clone; every
/// clone talks to the same target.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Session {
    target: Target,
    session_id: Option<String>,
}

fn debuggee(target: &Target, session_id: Option<&str>) -> Value {
    let mut value = match target {
        Target::Tab(id) => serde_json::json!({ "tabId": id }),
        Target::Extension(id) => serde_json::json!({ "extensionId": id }),
        Target::Id(id) => serde_json::json!({ "targetId": id }),
    };
    if let Some(session_id) = session_id {
        value["sessionId"] = session_id.into();
    }
    value
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    tab_id: Option<i32>,
    extension_id: Option<String>,
    target_id: Option<String>,
    session_id: Option<String>,
}

impl Source {
    fn session(self) -> Option<Session> {
        let target = match (self.tab_id, self.extension_id, self.target_id) {
            (Some(id), _, _) => Target::Tab(id),
            (_, Some(id), _) => Target::Extension(id),
            (_, _, Some(id)) => Target::Id(id),
            _ => return None,
        };
        Some(Session { target, session_id: self.session_id })
    }
}

/// A command the target answered with an error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{method}` failed: {message} (code {code})")]
pub struct CommandError {
    pub method: String,
    /// The protocol's error code, like -32601 for a method that doesn't exist.
    pub code: i64,
    pub message: String,
}

#[derive(Deserialize)]
struct ProtocolError {
    code: i64,
    message: String,
}

async fn run(method: &str, args: &[JsValue]) -> Result<JsValue> {
    call::only(Only::Chrome, API, INSTEAD)?;
    call::call(API, method, args, "Debugger").await
}

/// Attaches to `target`. Fails when something else (DevTools, another extension) already is,
/// or for pages extensions can't touch, like `chrome://` ones and the Web Store.
pub async fn attach(target: Target) -> Result<Session> {
    run("attach", &[crate::json::to_js(&debuggee(&target, None))?, JsValue::from_str(VERSION)]).await?;
    Ok(Session { target, session_id: None })
}

impl Session {
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// The protocol session id, for a child session.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// A session inside this target, such as a worker or a cross-site iframe, by the `sessionId`
    /// of a `Target.attachedToTarget` event. Those come after sending `Target.setAutoAttach`
    /// with `{ "autoAttach": true, "waitForDebuggerOnStart": false, "flatten": true }`; Chrome
    /// doesn't let extensions call `Target.attachToTarget` themselves.
    pub fn child(&self, session_id: impl Into<String>) -> Session {
        Session { target: self.target.clone(), session_id: Some(session_id.into()) }
    }

    /// Sends a protocol command and reads its result as `T` (`serde_json::Value` takes any).
    /// `params` is usually `json!({...})`; give `()` for none.
    pub async fn send<T: DeserializeOwned>(&self, method: &str, params: impl Serialize) -> Result<T> {
        let value = self.command(method, params).await?;
        crate::json::from_js(&value).map_err(|error| {
            OxichromeError::UnexpectedValue(format!("the result of `{method}` doesn't fit `{}`: {error}", std::any::type_name::<T>()))
        })
    }

    /// Sends a protocol command whose result isn't needed, like `Network.enable`.
    pub async fn run(&self, method: &str, params: impl Serialize) -> Result<()> {
        self.command(method, params).await.map(|_| ())
    }

    async fn command(&self, method: &str, params: impl Serialize) -> Result<JsValue> {
        let target = crate::json::to_js(&debuggee(&self.target, self.session_id.as_deref()))?;
        let params = crate::json::to_js(&params)?;
        let mut args = vec![target, JsValue::from_str(method)];
        if !params.is_null() && !params.is_undefined() {
            args.push(params);
        }
        run("sendCommand", &args).await.map_err(|error| match &error {
            OxichromeError::Js(js) => match serde_json::from_str::<ProtocolError>(&js.to_string()) {
                Ok(found) => OxichromeError::Debugger(CommandError { method: method.to_string(), code: found.code, message: found.message }),
                Err(_) => error,
            },
            _ => error,
        })
    }

    /// Detaches from the target. For a child session this detaches the whole target; to leave
    /// only the child, send `Target.detachFromTarget` instead.
    pub async fn detach(self) -> Result<()> {
        run("detach", &[crate::json::to_js(&debuggee(&self.target, None))?]).await.map(|_| ())
    }

    /// Runs `handler` for every event from this session (not its children).
    pub fn on_event(&self, handler: impl Fn(Event) + 'static) -> Result<Listener> {
        let me = self.clone();
        on_event(move |event| {
            if event.session == me {
                handler(event);
            }
        })
    }
}

/// An event a target sent, like `Network.requestWillBeSent`.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Where it came from.
    pub session: Session,
    pub method: String,
    pub params: Value,
}

impl Event {
    /// The event's parameters as `T`.
    pub fn params<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_value(self.params.clone()).map_err(|error| OxichromeError::Serde(error.to_string()))
    }
}

fn listen(event: &str, callback: JsValue) -> Result<Listener> {
    call::only(Only::Chrome, API, INSTEAD)?;
    Listener::add(API, event, &callback)
        .ok_or_else(|| OxichromeError::InvalidArgument("`debugger` isn't available here; add `Debugger` to `permissions` in #[oxichrome::extension]".into()))
}

/// Runs `handler` for every event from every attached target.
pub fn on_event(handler: impl Fn(Event) + 'static) -> Result<Listener> {
    let callback = wasm_bindgen::closure::Closure::<dyn Fn(JsValue, JsValue, JsValue)>::new(move |source: JsValue, method: JsValue, params: JsValue| {
        let Some(session) = crate::json::from_js::<Source>(&source).ok().and_then(Source::session) else { return };
        let params = if params.is_undefined() { Value::Null } else { crate::json::from_js(&params).unwrap_or(Value::Null) };
        handler(Event { session, method: method.as_string().unwrap_or_default(), params });
    })
    .into_js_value();
    listen("onEvent", callback)
}

/// Why Chrome let go of a target.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Detached {
    /// The tab closed or the target went away.
    TargetClosed,
    /// The user closed Chrome's "is debugging this browser" bar, or opened DevTools on it.
    CanceledByUser,
    Other(String),
}

/// Runs `handler` when Chrome detaches from a target by itself (not after [`Session::detach`]).
pub fn on_detach(handler: impl Fn(Session, Detached) + 'static) -> Result<Listener> {
    let callback = wasm_bindgen::closure::Closure::<dyn Fn(JsValue, JsValue)>::new(move |source: JsValue, reason: JsValue| {
        let Some(session) = crate::json::from_js::<Source>(&source).ok().and_then(Source::session) else { return };
        let reason = match reason.as_string().unwrap_or_default().as_str() {
            "target_closed" => Detached::TargetClosed,
            "canceled_by_user" => Detached::CanceledByUser,
            other => Detached::Other(other.to_string()),
        };
        handler(session, reason);
    })
    .into_js_value();
    listen("onDetach", callback)
}

/// What kind of target it is.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Kind {
    Page,
    BackgroundPage,
    Worker,
    #[serde(other)]
    Other,
}

/// A target Chrome could attach to.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    #[serde(rename = "type")]
    pub kind: Kind,
    pub id: String,
    pub tab_id: Option<i32>,
    pub extension_id: Option<String>,
    /// Whether something (this extension, DevTools, another one) is attached.
    pub attached: bool,
    pub title: String,
    pub url: String,
    pub favicon_url: Option<String>,
}

impl TargetInfo {
    /// The [`Target`] to [`attach`] to it: its tab when it is one, otherwise its id.
    pub fn target(&self) -> Target {
        match self.tab_id {
            Some(id) => Target::Tab(id),
            None => Target::Id(self.id.clone()),
        }
    }
}

/// Every target Chrome could attach to.
pub async fn targets() -> Result<Vec<TargetInfo>> {
    crate::json::from_js(&run("getTargets", &[]).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debuggees() {
        assert_eq!(debuggee(&Target::Tab(4), None), serde_json::json!({ "tabId": 4 }));
        assert_eq!(debuggee(&Target::Id("T".into()), Some("S")), serde_json::json!({ "targetId": "T", "sessionId": "S" }));
        assert_eq!(debuggee(&Target::Extension("e".into()), None), serde_json::json!({ "extensionId": "e" }));
    }

    #[test]
    fn sources() {
        let source: Source = serde_json::from_value(serde_json::json!({ "tabId": 3, "sessionId": "S" })).unwrap();
        assert_eq!(source.session(), Some(Session { target: Target::Tab(3), session_id: Some("S".into()) }));
        let source: Source = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(source.session(), None);
    }

    #[test]
    fn target_info() {
        let info: TargetInfo = serde_json::from_value(serde_json::json!({
            "type": "service_worker", "id": "X", "attached": false, "title": "t", "url": "u"
        }))
        .unwrap();
        assert_eq!(info.kind, Kind::Other);
        assert_eq!(info.target(), Target::Id("X".into()));
    }
}
