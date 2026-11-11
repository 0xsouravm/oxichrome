use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;
use crate::tabs::Tab;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WindowType {
    Normal,
    Popup,
    Panel,
    App,
    Devtools,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
    LockedFullscreen,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Window {
    pub id: i32,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub incognito: bool,
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(rename = "type")]
    pub kind: WindowType,
    pub state: WindowState,
    pub left: Option<i32>,
    pub top: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub title: Option<String>,
    pub tabs: Option<Vec<Tab>>,
}

impl FromEventArg for Window {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        crate::json::from_js(&value)
    }
}

impl EventArg<events::WindowsOnCreated, 1> for Window {}
impl EventArg<events::WindowsOnBoundsChanged, 1> for Window {}
impl EventArg<events::WindowsOnRemoved, 1> for i32 {}
impl EventArg<events::WindowsOnFocusChanged, 1> for Option<i32> {}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Bounds {
    #[serde(skip_serializing_if = "Option::is_none")]
    left: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<i32>,
}

impl Bounds {
    fn is_set(&self) -> bool {
        self.left.is_some() || self.top.is_some() || self.width.is_some() || self.height.is_some()
    }

    fn check(&self) -> Result<()> {
        for (name, value) in [("width", self.width), ("height", self.height)] {
            if let Some(value) = value {
                if value <= 0 {
                    return Err(OxichromeError::InvalidArgument(format!("the {name} must be more than 0, got {value}")));
                }
            }
        }
        Ok(())
    }
}

fn check_state(state: Option<WindowState>, focused: Option<bool>, bounds: &Bounds) -> Result<()> {
    let Some(state) = state else {
        return Ok(());
    };
    let name = match state {
        WindowState::Normal => return Ok(()),
        WindowState::Minimized => "minimized",
        WindowState::Maximized => "maximized",
        WindowState::Fullscreen => "fullscreen",
        WindowState::LockedFullscreen | WindowState::Other => {
            return Err(OxichromeError::InvalidArgument(
                "only `Normal`, `Minimized`, `Maximized` and `Fullscreen` can be set; locked fullscreen is for ChromeOS kiosk apps".to_string(),
            ))
        }
    };
    if bounds.is_set() {
        return Err(OxichromeError::InvalidArgument(format!(
            "a {name} window can't also get a size or position (browsers refuse it); set the size with `WindowState::Normal`, or leave it out"
        )));
    }
    match (state, focused) {
        (WindowState::Minimized, Some(true)) => Err(OxichromeError::InvalidArgument(
            "a minimized window can't be focused (Chrome refuses it); leave out `focused(true)`".to_string(),
        )),
        (WindowState::Maximized | WindowState::Fullscreen, Some(false)) => Err(OxichromeError::InvalidArgument(format!(
            "a {name} window is always focused (Chrome refuses `focused(false)` here); leave it out"
        ))),
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, Default)]
pub struct NewWindow {
    urls: Vec<String>,
    tab_id: Option<i32>,
    kind: Option<WindowType>,
    state: Option<WindowState>,
    focused: Option<bool>,
    incognito: Option<bool>,
    title_preface: Option<String>,
    bounds: Bounds,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateData<'a> {
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    url: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    tab_id: Option<i32>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<WindowType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<WindowState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    focused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    incognito: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title_preface: Option<&'a str>,
    #[serde(flatten)]
    bounds: &'a Bounds,
}

impl NewWindow {
    pub fn new() -> Self {
        NewWindow::default()
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.urls.push(url.into());
        self
    }

    pub fn tab(mut self, tab_id: i32) -> Self {
        self.tab_id = Some(tab_id);
        self
    }

    pub fn popup(mut self) -> Self {
        self.kind = Some(WindowType::Popup);
        self
    }

    pub fn state(mut self, state: WindowState) -> Self {
        self.state = Some(state);
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = Some(focused);
        self
    }

    pub fn incognito(mut self) -> Self {
        self.incognito = Some(true);
        self
    }

    pub fn title_preface(mut self, preface: impl Into<String>) -> Self {
        self.title_preface = Some(preface.into());
        self
    }

    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.bounds.width = Some(width);
        self.bounds.height = Some(height);
        self
    }

    pub fn position(mut self, left: i32, top: i32) -> Self {
        self.bounds.left = Some(left);
        self.bounds.top = Some(top);
        self
    }

    fn check(&self) -> Result<()> {
        if self.tab_id.is_some() && !self.urls.is_empty() {
            return Err(OxichromeError::InvalidArgument(
                "a new window takes either an existing tab or URLs, not both".to_string(),
            ));
        }
        for url in &self.urls {
            if url.trim_start().to_ascii_lowercase().starts_with("javascript:") {
                return Err(OxichromeError::InvalidArgument(format!(
                    "browsers don't open `javascript:` URLs in new windows (`{url}`); run code with `#[oxichrome::content_script]` instead"
                )));
            }
        }
        self.bounds.check()?;
        check_state(self.state, self.focused, &self.bounds)
    }
}

#[derive(Debug, Clone, Default)]
pub struct WindowUpdate {
    state: Option<WindowState>,
    focused: Option<bool>,
    draw_attention: Option<bool>,
    title_preface: Option<String>,
    bounds: Bounds,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<WindowState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    focused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    draw_attention: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title_preface: Option<&'a str>,
    #[serde(flatten)]
    bounds: &'a Bounds,
}

impl WindowUpdate {
    pub fn new() -> Self {
        WindowUpdate::default()
    }

    pub fn state(mut self, state: WindowState) -> Self {
        self.state = Some(state);
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = Some(focused);
        self
    }

    pub fn draw_attention(mut self, on: bool) -> Self {
        self.draw_attention = Some(on);
        self
    }

    pub fn title_preface(mut self, preface: impl Into<String>) -> Self {
        self.title_preface = Some(preface.into());
        self
    }

    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.bounds.width = Some(width);
        self.bounds.height = Some(height);
        self
    }

    pub fn position(mut self, left: i32, top: i32) -> Self {
        self.bounds.left = Some(left);
        self.bounds.top = Some(top);
        self
    }

    fn check(&self) -> Result<()> {
        if self.state.is_none() && self.focused.is_none() && self.draw_attention.is_none() && self.title_preface.is_none() && !self.bounds.is_set() {
            return Err(OxichromeError::InvalidArgument("the update is empty; set something to change".to_string()));
        }
        self.bounds.check()?;
        check_state(self.state, self.focused, &self.bounds)
    }
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    let lower = message.to_ascii_lowercase();
    if lower.contains("no window with id") || lower.contains("invalid window id") {
        return OxichromeError::InvalidArgument(format!("{message} (the window was closed, or the id is from before a restart)"));
    }
    if lower.contains("incognito") {
        return OxichromeError::InvalidArgument(incognito_message());
    }
    if lower.contains("illegal url") {
        return OxichromeError::InvalidArgument(format!(
            "{message} (Firefox doesn't let extensions open `about:`, `chrome:` or `file:` pages)"
        ));
    }
    if lower.contains("bounds must be") {
        return OxichromeError::InvalidArgument(format!("{message} (Chrome needs at least half the window on screen)"));
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

fn incognito_message() -> String {
    "the user hasn't allowed this extension in private windows; they can turn it on in the browser's extension settings".to_string()
}

async fn settle<T: serde::de::DeserializeOwned>(promise: js_sys::Promise) -> Result<T> {
    let value = JsFuture::from(promise).await.map_err(browser_error)?;
    crate::json::from_js(&value)
}

pub async fn get(window_id: i32) -> Result<Window> {
    settle(js_bridge::windows_get(window_id, false).map_err(browser_error)?).await
}

pub async fn get_with_tabs(window_id: i32) -> Result<Window> {
    settle(js_bridge::windows_get(window_id, true).map_err(browser_error)?).await
}

pub async fn current() -> Result<Window> {
    settle(js_bridge::windows_current(false).map_err(browser_error)?).await
}

pub async fn current_with_tabs() -> Result<Window> {
    settle(js_bridge::windows_current(true).map_err(browser_error)?).await
}

pub async fn last_focused() -> Result<Window> {
    settle(js_bridge::windows_last_focused(false).map_err(browser_error)?).await
}

pub async fn last_focused_with_tabs() -> Result<Window> {
    settle(js_bridge::windows_last_focused(true).map_err(browser_error)?).await
}

pub async fn get_all() -> Result<Vec<Window>> {
    settle(js_bridge::windows_all(false).map_err(browser_error)?).await
}

pub async fn get_all_with_tabs() -> Result<Vec<Window>> {
    settle(js_bridge::windows_all(true).map_err(browser_error)?).await
}

pub async fn create(window: NewWindow) -> Result<Window> {
    window.check()?;
    let data = crate::json::to_js(&CreateData {
        url: &window.urls,
        tab_id: window.tab_id,
        kind: window.kind,
        state: window.state,
        focused: window.focused,
        incognito: window.incognito,
        title_preface: window.title_preface.as_deref(),
        bounds: &window.bounds,
    })?;
    let created: Option<Window> = settle(js_bridge::windows_create(&data).map_err(browser_error)?).await?;
    created.ok_or_else(|| OxichromeError::InvalidArgument(incognito_message()))
}

pub async fn update(window_id: i32, update: WindowUpdate) -> Result<Window> {
    update.check()?;
    let info = crate::json::to_js(&UpdateInfo {
        state: update.state,
        focused: update.focused,
        draw_attention: update.draw_attention,
        title_preface: update.title_preface.as_deref(),
        bounds: &update.bounds,
    })?;
    settle(js_bridge::windows_update(window_id, &info).map_err(browser_error)?).await
}

pub async fn remove(window_id: i32) -> Result<()> {
    JsFuture::from(js_bridge::windows_remove(window_id).map_err(browser_error)?)
        .await
        .map_err(browser_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(result: Result<()>) -> String {
        match result.unwrap_err() {
            OxichromeError::InvalidArgument(message) => message,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rejects_what_browsers_refuse() {
        assert!(err(NewWindow::new().state(WindowState::Maximized).size(300, 200).check()).contains("can't also get a size"));
        assert!(err(NewWindow::new().state(WindowState::Minimized).focused(true).check()).contains("can't be focused"));
        assert!(err(NewWindow::new().state(WindowState::Fullscreen).focused(false).check()).contains("always focused"));
        assert!(err(NewWindow::new().state(WindowState::LockedFullscreen).check()).contains("ChromeOS"));
        assert!(err(NewWindow::new().url("javascript:alert(1)").check()).contains("javascript:"));
        assert!(err(NewWindow::new().url("a.html").tab(3).check()).contains("not both"));
        assert!(err(NewWindow::new().size(0, 100).check()).contains("more than 0"));
        assert!(err(WindowUpdate::new().check()).contains("empty"));
        assert!(err(WindowUpdate::new().state(WindowState::Minimized).position(1, 2).check()).contains("size or position"));
        assert!(NewWindow::new().state(WindowState::Minimized).focused(false).check().is_ok());
        assert!(NewWindow::new().popup().size(400, 300).position(10, 10).focused(false).check().is_ok());
        assert!(WindowUpdate::new().state(WindowState::Normal).size(500, 400).check().is_ok());
    }

    #[test]
    fn reads_both_browsers() {
        let chrome: Window = serde_json::from_str(r#"{"alwaysOnTop":false,"focused":true,"height":556,"id":7,"incognito":false,"left":22,"state":"normal","top":22,"type":"normal","width":756}"#).unwrap();
        assert_eq!((chrome.id, chrome.kind, chrome.state, chrome.title), (7, WindowType::Normal, WindowState::Normal, None));
        let firefox: Window = serde_json::from_str(r#"{"alwaysOnTop":false,"focused":false,"height":300,"id":45,"incognito":false,"left":50,"state":"maximized","title":"Mozilla Firefox","top":60,"type":"popup","width":400,"tabs":[{"id":3,"index":0,"windowId":45,"active":true,"pinned":false,"highlighted":true,"incognito":false,"url":"about:blank"}]}"#).unwrap();
        assert_eq!((firefox.kind, firefox.state, firefox.title.as_deref()), (WindowType::Popup, WindowState::Maximized, Some("Mozilla Firefox")));
        assert_eq!(firefox.tabs.unwrap()[0].window_id, 45);
        let odd: Window = serde_json::from_str(r#"{"id":1,"type":"something_new","state":"docked"}"#).unwrap();
        assert_eq!((odd.kind, odd.state), (WindowType::Other, WindowState::Other));
        let data = serde_json::to_value(CreateData { url: &["a.html".to_string()], tab_id: None, kind: Some(WindowType::Popup), state: None, focused: Some(false), incognito: None, title_preface: None, bounds: &Bounds { width: Some(400), height: Some(300), ..Bounds::default() } }).unwrap();
        assert_eq!(data, serde_json::json!({"url":["a.html"],"type":"popup","focused":false,"width":400,"height":300}));
        assert_eq!(serde_json::to_value(WindowState::Fullscreen).unwrap(), "fullscreen");
    }
}
