use std::future::Future;
use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;
use crate::tabs::Tab;

pub fn get_url(path: &str) -> String {
    js_bridge::chrome_runtime_get_url(path)
}

/// This extension's id.
pub fn id() -> String {
    js_bridge::runtime_id()
}

/// The extension's `manifest.json` as the browser loaded it.
pub fn get_manifest() -> serde_json::Value {
    crate::json::from_js(&js_bridge::runtime_manifest()).unwrap_or(serde_json::Value::Null)
}

/// Sends a message to the background and extension pages and returns the raw reply.
pub async fn send_message<T: Serialize>(message: &T) -> Result<JsValue> {
    let js_msg = crate::json::to_js(message)?;
    let promise = js_bridge::chrome_runtime_send_message_checked(&js_msg).map_err(|err| receiver_error(err, NO_RUNTIME_RECEIVER))?;
    JsFuture::from(promise).await.map_err(|err| receiver_error(err, NO_RUNTIME_RECEIVER))
}

/// Sends a message to the background and extension pages and reads the reply as `R`.
///
/// The first receiver to answer wins. `R = Option<T>` reads "nobody answered" as `None`.
pub async fn request<M: Serialize, R: DeserializeOwned>(message: &M) -> Result<R> {
    read_reply(send_message(message).await?)
}

pub(crate) const NO_RUNTIME_RECEIVER: &str = "nothing received the message: no background handler or open extension page is listening (the background doesn't receive messages it sends itself)";
pub(crate) const NO_TAB_RECEIVER: &str = "nothing received the message: that tab or frame has no content script listening, or doesn't exist";

pub(crate) fn receiver_error(err: JsValue, text: &str) -> OxichromeError {
    let message = JsError::new(err).to_string();
    if message.contains("Receiving end does not exist") {
        OxichromeError::Js(JsError::new(JsValue::from_str(text)))
    } else {
        OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
    }
}

pub(crate) fn read_reply<R: DeserializeOwned>(reply: JsValue) -> Result<R> {
    let empty = reply.is_undefined() || reply.is_null();
    crate::json::from_js(&reply).map_err(|err| {
        if empty {
            OxichromeError::UnexpectedValue(
                "every receiver finished without replying; read the reply as an `Option` if that can happen".to_string(),
            )
        } else {
            err
        }
    })
}

/// Why `runtime::on_installed` fired.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InstallReason {
    Install,
    Update,
    /// The browser itself updated (`chrome_update` in Chrome, `browser_update` in Firefox).
    BrowserUpdate,
    /// A shared module this extension imports updated (Chrome only).
    SharedModuleUpdate,
    Other(String),
}

/// The details `runtime::on_installed` passes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InstalledDetails {
    pub reason: InstallReason,
    /// The version before an `Update`.
    pub previous_version: Option<String>,
    /// The shared module's id for `SharedModuleUpdate`.
    pub id: Option<String>,
    /// A temporary add-on in Firefox (`about:debugging`); always `false` in Chrome.
    pub temporary: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawInstalled {
    reason: String,
    previous_version: Option<String>,
    id: Option<String>,
    #[serde(default)]
    temporary: bool,
}

impl FromEventArg for InstalledDetails {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let raw: RawInstalled = crate::json::from_js(&value)?;
        let reason = match raw.reason.as_str() {
            "install" => InstallReason::Install,
            "update" => InstallReason::Update,
            "chrome_update" | "browser_update" => InstallReason::BrowserUpdate,
            "shared_module_update" => InstallReason::SharedModuleUpdate,
            _ => InstallReason::Other(raw.reason),
        };
        Ok(InstalledDetails { reason, previous_version: raw.previous_version, id: raw.id, temporary: raw.temporary })
    }
}

impl EventArg<events::RuntimeOnInstalled, 1> for InstalledDetails {}

/// Who sent a message or opened a port.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MessageSender {
    /// The sending extension's id.
    pub id: Option<String>,
    /// The page or frame that sent it.
    pub url: Option<String>,
    pub origin: Option<String>,
    /// The tab, when a content script or an extension page in a tab sent it.
    pub tab: Option<Tab>,
    /// The frame inside `tab` (`0` is the top frame).
    pub frame_id: Option<i64>,
    pub document_id: Option<String>,
}

impl MessageSender {
    /// `true` when a content script sent it (the url is a web page, not this extension).
    pub fn is_content_script(&self) -> bool {
        let own = get_url("");
        self.tab.is_some() && !self.url.as_deref().is_some_and(|url| url.starts_with(&own))
    }
}

impl FromEventArg for MessageSender {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        crate::json::from_js(&value)
    }
}

impl EventArg<events::RuntimeOnMessage, 2> for MessageSender {}
impl EventArg<events::RuntimeOnUserScriptMessage, 2> for MessageSender {}

/// The reply channel `runtime::on_message` passes as its third argument.
///
/// Only the first reply to a message is delivered, from any handler or page.
#[derive(Clone)]
pub struct Reply(js_sys::Function);

impl Reply {
    /// Sends `value` as the reply. `Ok(false)` means another reply was already sent.
    pub fn send<T: Serialize + ?Sized>(&self, value: &T) -> Result<bool> {
        let value = crate::json::to_js(value)?;
        Ok(self.0.call1(&JsValue::NULL, &value)?.as_bool().unwrap_or(true))
    }
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reply")
    }
}

impl FromEventArg for Reply {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        value
            .dyn_into::<js_sys::Function>()
            .map(Reply)
            .map_err(|value| OxichromeError::UnexpectedValue(format!("expected a reply function, got {value:?}")))
    }
}

impl EventArg<events::RuntimeOnMessage, 3> for Reply {}
impl EventArg<events::RuntimeOnUserScriptMessage, 3> for Reply {}

#[doc(hidden)]
pub fn __reply<R: Serialize>(reply: &JsValue, value: &R) -> std::result::Result<(), JsValue> {
    let reply = Reply::from_event_arg(reply.clone()).map_err(|err| JsValue::from_str(&err.to_string()))?;
    reply.send(value).map(|_| ()).map_err(|err| {
        JsValue::from(js_sys::Error::new(&format!("[oxichrome] the reply could not be sent: {err}")))
    })
}

#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be the message of a message handler",
    label = "the message must implement `Deserialize` and own its data",
    note = "derive `serde::Deserialize` on your message type (use `String`, not `&str`), or take `JsValue` to see every message"
)]
pub trait __Message {}

impl<T: DeserializeOwned> __Message for T {}

#[doc(hidden)]
pub fn __assert_message<T: __Message>() {}

#[doc(hidden)]
pub struct __MessageProbe<T>(pub PhantomData<T>);

#[doc(hidden)]
pub trait __MessageAsJs {
    fn __read(&self, value: JsValue) -> Option<JsValue>;
}

impl __MessageAsJs for __MessageProbe<JsValue> {
    fn __read(&self, value: JsValue) -> Option<JsValue> {
        Some(value)
    }
}

#[doc(hidden)]
pub trait __MessageAsSerde<T> {
    fn __read(&self, value: JsValue) -> Option<T>;
}

impl<T: DeserializeOwned> __MessageAsSerde<T> for &__MessageProbe<T> {
    fn __read(&self, value: JsValue) -> Option<T> {
        crate::json::from_js(&value).ok()
    }
}

fn is_internal(message: &JsValue) -> bool {
    message.is_object()
        && js_sys::Object::keys(message.unchecked_ref::<js_sys::Object>())
            .iter()
            .any(|key: JsValue| key.as_string().is_some_and(|key| key.starts_with("__oxichrome_")))
}

/// A listener added with [`listen`], [`on_connect`], `storage::on_changed` or `tabs::on_activated`.
/// It keeps listening until [`remove`](Listener::remove) is called; dropping it doesn't stop it.
pub struct Listener {
    remove: Option<js_sys::Function>,
}

impl Listener {
    /// Adds `callback` to `chrome[api][event]`, or `None` when this context doesn't have it.
    pub(crate) fn add(api: &str, event: &str, callback: &JsValue) -> Option<Listener> {
        Listener::from_remover(js_bridge::event_add(api, event, callback))
    }

    /// Wraps a function that removes the listener, or `None` when `remove` isn't one.
    pub(crate) fn from_remover(remove: JsValue) -> Option<Listener> {
        remove.is_function().then(|| Listener { remove: Some(remove.unchecked_into()) })
    }

    /// Stops listening.
    pub fn remove(self) {
        if let Some(remove) = self.remove {
            let _ = remove.call0(&JsValue::NULL);
        }
    }
}

/// Answers messages in a popup, options page, side panel, offscreen document or content script.
///
/// `handler` runs for every message that reads as `M`, and its return value is the reply.
/// Messages that don't read as `M` are left for other listeners. The background uses
/// `#[oxichrome::on(runtime::on_message)]` instead. A content script receives what
/// `tabs::request` sends; pages receive what `runtime::request` sends.
pub fn listen<M, R, F, Fut>(handler: F) -> Listener
where
    M: DeserializeOwned + 'static,
    R: Serialize + 'static,
    F: Fn(M, MessageSender) -> Fut + 'static,
    Fut: Future<Output = R> + 'static,
{
    let callback = Closure::<dyn FnMut(JsValue, JsValue, JsValue) -> JsValue>::new(
        move |message: JsValue, sender: JsValue, respond: JsValue| -> JsValue {
            if is_internal(&message) {
                return JsValue::UNDEFINED;
            }
            let Ok(message) = crate::json::from_js::<M>(&message) else {
                return JsValue::UNDEFINED;
            };
            let sender: MessageSender = crate::json::from_js(&sender).unwrap_or(MessageSender {
                id: None,
                url: None,
                origin: None,
                tab: None,
                frame_id: None,
                document_id: None,
            });
            let future = handler(message, sender);
            wasm_bindgen_futures::spawn_local(async move {
                let value = future.await;
                let respond: js_sys::Function = respond.unchecked_into();
                match crate::json::to_js(&value) {
                    Ok(value) => {
                        let _ = respond.call1(&JsValue::NULL, &value);
                    }
                    Err(err) => {
                        web_sys::console::error_1(&JsValue::from_str(&format!("[oxichrome] the reply could not be sent: {err}")));
                        let _ = respond.call0(&JsValue::NULL);
                    }
                }
            });
            JsValue::TRUE
        },
    )
    .into_js_value();
    Listener::add("runtime", "onMessage", &callback).unwrap_or(Listener { remove: None })
}

/// A long-lived connection between two parts of the extension.
///
/// Open one with [`connect`] or `tabs::connect`; the other side gets it from
/// `#[oxichrome::on(runtime::on_connect)]` (background) or [`on_connect`] (pages and content scripts).
#[derive(Clone)]
pub struct Port(JsValue);

impl std::fmt::Debug for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Port").field("name", &self.name()).field("open", &self.is_open()).finish()
    }
}

impl Port {
    pub(crate) fn wrap(raw: &JsValue) -> Port {
        Port(js_bridge::port_wrap(raw))
    }

    /// The name given to `connect`.
    pub fn name(&self) -> String {
        js_bridge::port_name(&self.0)
    }

    /// Who opened the port; `None` on the side that called `connect`.
    pub fn sender(&self) -> Option<MessageSender> {
        let sender = js_bridge::port_sender(&self.0);
        if sender.is_undefined() || sender.is_null() {
            return None;
        }
        crate::json::from_js(&sender).ok()
    }

    /// Sends a message to the other side.
    pub fn post<T: Serialize + ?Sized>(&self, message: &T) -> Result<()> {
        let message = crate::json::to_js(message)?;
        js_bridge::port_post(&self.0, &message)?;
        Ok(())
    }

    /// The next message, or `None` once the port is closed and every message has been read.
    pub async fn recv<T: DeserializeOwned>(&self) -> Option<Result<T>> {
        let next = JsFuture::from(js_bridge::port_next(&self.0)).await.ok()?;
        if next.is_undefined() {
            return None;
        }
        let value = js_sys::Reflect::get(&next, &JsValue::from_str("value")).unwrap_or(JsValue::NULL);
        Some(crate::json::from_js(&value))
    }

    /// Closes the port. The other side's `recv` returns `None` and `closed` returns `Ok(())`.
    pub fn disconnect(&self) {
        js_bridge::port_disconnect(&self.0);
    }

    /// `false` once either side closed the port.
    pub fn is_open(&self) -> bool {
        js_bridge::port_is_open(&self.0)
    }

    /// Waits until the port closes. `Err` says why when it never connected
    /// (nothing was listening) or the browser closed it with an error.
    pub async fn closed(&self) -> Result<()> {
        let reason = JsFuture::from(js_bridge::port_closed(&self.0)).await.unwrap_or(JsValue::NULL);
        match reason.as_string() {
            Some(reason) if reason.contains("Receiving end does not exist") => Err(OxichromeError::Js(JsError::new(
                JsValue::from_str("the port closed because nothing was listening for it"),
            ))),
            Some(reason) => Err(OxichromeError::Js(JsError::new(JsValue::from_str(&reason)))),
            None => Ok(()),
        }
    }
}

impl FromEventArg for Port {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        Ok(Port::wrap(&value))
    }
}

impl EventArg<events::RuntimeOnConnect, 1> for Port {}

/// Opens a port to the background and extension pages.
pub fn connect(name: &str) -> Result<Port> {
    let raw = js_bridge::runtime_connect(name)?;
    Ok(Port::wrap(&raw))
}

/// Accepts ports in a popup, options page, side panel or content script.
/// The background uses `#[oxichrome::on(runtime::on_connect)]` instead.
pub fn on_connect<F, Fut>(handler: F) -> Listener
where
    F: Fn(Port) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    let callback = Closure::<dyn FnMut(JsValue)>::new(move |raw: JsValue| {
        let port = Port::wrap(&raw);
        wasm_bindgen_futures::spawn_local(handler(port));
    })
    .into_js_value();
    Listener::add("runtime", "onConnect", &callback).unwrap_or(Listener { remove: None })
}

/// The operating system the browser runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Os {
    Mac,
    Windows,
    Linux,
    ChromeOs,
    Android,
    OpenBsd,
    Other(String),
}

/// The processor architecture the browser runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Arch {
    Arm,
    /// `arm64` in Chrome, `aarch64` in Firefox.
    Arm64,
    X86_32,
    X86_64,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PlatformInfo {
    pub os: Os,
    pub arch: Arch,
}

#[derive(Deserialize)]
struct RawPlatform {
    os: String,
    arch: String,
}

/// The browser's operating system and architecture, the same names in both browsers.
pub async fn get_platform_info() -> Result<PlatformInfo> {
    let raw: RawPlatform = crate::json::from_js(&JsFuture::from(js_bridge::runtime_platform_info()?).await?)?;
    let os = match raw.os.as_str() {
        "mac" => Os::Mac,
        "win" => Os::Windows,
        "linux" => Os::Linux,
        "cros" => Os::ChromeOs,
        "android" => Os::Android,
        "openbsd" => Os::OpenBsd,
        _ => Os::Other(raw.os),
    };
    let arch = match raw.arch.as_str() {
        "arm" => Arch::Arm,
        "arm64" | "aarch64" => Arch::Arm64,
        "x86-32" => Arch::X86_32,
        "x86-64" => Arch::X86_64,
        _ => Arch::Other(raw.arch),
    };
    Ok(PlatformInfo { os, arch })
}

/// Opens the extension's options page.
pub async fn open_options_page() -> Result<()> {
    let manifest = get_manifest();
    let declared = |key: &str| manifest.get(key).is_some_and(|value| !value.is_null());
    if !declared("options_ui") && !declared("options_page") {
        return Err(OxichromeError::InvalidArgument(
            "the extension has no options page: add `#[oxichrome::options_page]` to a component".to_string(),
        ));
    }
    JsFuture::from(js_bridge::runtime_open_options_page()?).await?;
    Ok(())
}

/// The longest uninstall URL both browsers accept.
pub const MAX_UNINSTALL_URL: usize = 1023;

/// Sets the page the browser opens after the extension is uninstalled.
/// An `http`/`https` URL of at most 1023 characters, or `""` to open nothing.
pub async fn set_uninstall_url(url: &str) -> Result<()> {
    check_uninstall_url(url)?;
    JsFuture::from(js_bridge::runtime_set_uninstall_url(url)?).await?;
    Ok(())
}

fn check_uninstall_url(url: &str) -> Result<()> {
    if url.is_empty() {
        return Ok(());
    }
    let length = url.chars().count();
    if length > MAX_UNINSTALL_URL {
        return Err(OxichromeError::LimitExceeded(format!(
            "the uninstall URL is {length} characters; browsers allow at most {MAX_UNINSTALL_URL}"
        )));
    }
    let lower = url.to_ascii_lowercase();
    let rest = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://"));
    match rest {
        Some(rest) if !rest.is_empty() && !rest.starts_with('/') => Ok(()),
        _ => Err(OxichromeError::InvalidArgument(format!(
            "the uninstall URL must be a full http or https URL, got {url:?}"
        ))),
    }
}

/// Reloads the extension. Pages of the extension close, and the background starts again.
pub fn reload() {
    js_bridge::runtime_reload();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uninstall_urls_follow_both_browsers() {
        assert!(check_uninstall_url("").is_ok());
        assert!(check_uninstall_url("https://example.org/bye").is_ok());
        assert!(check_uninstall_url("HTTP://example.org").is_ok());
        for bad in ["ftp://example.org/", "javascript:alert(1)", "bye.html", "https://", "http:///x", "example.org"] {
            assert!(matches!(check_uninstall_url(bad), Err(OxichromeError::InvalidArgument(_))), "{bad}");
        }
        let at_limit = format!("https://example.org/{}", "a".repeat(MAX_UNINSTALL_URL - 20));
        assert_eq!(at_limit.len(), MAX_UNINSTALL_URL);
        assert!(check_uninstall_url(&at_limit).is_ok());
        assert!(matches!(check_uninstall_url(&format!("{at_limit}a")), Err(OxichromeError::LimitExceeded(_))));
    }
}
