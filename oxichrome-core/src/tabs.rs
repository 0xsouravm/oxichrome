use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(from = "RawTab")]
#[non_exhaustive]
pub struct Tab {
    pub id: Option<i32>,
    pub index: i32,
    pub window_id: i32,
    pub active: bool,
    pub pinned: bool,
    pub highlighted: bool,
    pub incognito: bool,
    pub discarded: bool,
    pub audible: bool,
    pub url: Option<String>,
    pub title: Option<String>,
    pub fav_icon_url: Option<String>,
    /// `loading`, `complete`, or `unloaded` for a discarded tab (in both browsers).
    pub status: Option<String>,
    pub opener_tab_id: Option<i32>,
    /// The tab group it's in, if any.
    pub group_id: Option<i64>,
    /// Hidden from the tab strip by an extension with [`hide`]. Always `false` in Chrome.
    pub hidden: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTab {
    id: Option<i32>,
    #[serde(default)]
    index: i32,
    #[serde(default = "no_window")]
    window_id: i32,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    highlighted: bool,
    #[serde(default)]
    incognito: bool,
    #[serde(default)]
    discarded: bool,
    #[serde(default)]
    audible: bool,
    url: Option<String>,
    title: Option<String>,
    fav_icon_url: Option<String>,
    status: Option<String>,
    opener_tab_id: Option<i32>,
    group_id: Option<f64>,
    #[serde(default)]
    hidden: bool,
}

impl From<RawTab> for Tab {
    fn from(raw: RawTab) -> Self {
        Tab {
            id: raw.id,
            index: raw.index,
            window_id: raw.window_id,
            active: raw.active,
            pinned: raw.pinned,
            highlighted: raw.highlighted,
            incognito: raw.incognito,
            discarded: raw.discarded,
            audible: raw.audible,
            url: raw.url,
            title: raw.title,
            fav_icon_url: raw.fav_icon_url,
            status: if raw.discarded { Some("unloaded".to_string()) } else { raw.status },
            opener_tab_id: raw.opener_tab_id,
            group_id: raw.group_id.filter(|id| *id >= 0.0).map(|id| id as i64),
            hidden: raw.hidden,
        }
    }
}

fn no_window() -> i32 {
    -1
}

impl FromEventArg for Tab {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        crate::json::from_js(&value)
    }
}

impl EventArg<events::TabsOnUpdated, 1> for i32 {}
impl EventArg<events::TabsOnUpdated, 3> for Tab {}
impl EventArg<events::TabsOnRemoved, 1> for i32 {}
impl EventArg<events::TabsOnRemoved, 2> for Removed {}
impl EventArg<events::TabsOnActivated, 1> for Activated {}
impl EventArg<events::TabsOnCreated, 1> for Tab {}
impl EventArg<events::TabsOnMoved, 1> for i32 {}
impl EventArg<events::TabsOnMoved, 2> for Moved {}
impl EventArg<events::TabsOnAttached, 1> for i32 {}
impl EventArg<events::TabsOnAttached, 2> for Attached {}
impl EventArg<events::TabsOnDetached, 1> for i32 {}
impl EventArg<events::TabsOnDetached, 2> for Detached {}
impl EventArg<events::TabsOnHighlighted, 1> for Highlighted {}

pub use crate::runtime::Listener;

/// Runs `handler` each time the user switches tabs, in a popup, options page, side panel or
/// offscreen document. The background uses `#[oxichrome::on(tabs::on_activated)]` instead.
pub fn on_activated(handler: impl Fn(Activated) + 'static) -> Result<Listener> {
    let callback = wasm_bindgen::closure::Closure::<dyn Fn(JsValue)>::new(move |info: JsValue| {
        if let Ok(info) = crate::json::from_js::<Activated>(&info) {
            handler(info);
        }
    })
    .into_js_value();
    Listener::add("tabs", "onActivated", &callback)
        .ok_or_else(|| OxichromeError::InvalidArgument("`tabs` isn't available here (content scripts don't have it)".into()))
}

macro_rules! event_info {
    ($($(#[$doc:meta])* $name:ident { $($(#[$field_doc:meta])* $field:ident: $ty:ty),* $(,)? })*) => {
        $(
            $(#[$doc])*
            #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            #[non_exhaustive]
            pub struct $name {
                $($(#[$field_doc])* pub $field: $ty,)*
            }

            impl FromEventArg for $name {
                fn from_event_arg(value: JsValue) -> Result<Self> {
                    crate::json::from_js(&value)
                }
            }
        )*
    };
}

event_info! {
    /// The second argument of `tabs::on_removed`.
    Removed { window_id: i32, is_window_closing: bool }
    /// The argument of `tabs::on_activated`.
    Activated { tab_id: i32, window_id: i32 }
    /// The second argument of `tabs::on_moved` (within one window).
    Moved { window_id: i32, from_index: i32, to_index: i32 }
    /// The second argument of `tabs::on_attached` (the tab arrived in another window).
    Attached { new_window_id: i32, new_position: i32 }
    /// The second argument of `tabs::on_detached` (the tab is leaving its window).
    Detached { old_window_id: i32, old_position: i32 }
    /// The argument of `tabs::on_highlighted`: every highlighted tab in the window, in tab order.
    Highlighted { window_id: i32, tab_ids: Vec<i32> }
}

/// What [`query`] looks for. Every condition set must hold; an empty query matches every tab.
///
/// ```ignore
/// let pinned: Vec<Tab> = tabs::query(&Query::new().pinned(true).in_current_window()).await?;
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    #[serde(skip_serializing_if = "Option::is_none")]
    active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_window: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_focused_window: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    muted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    highlighted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discarded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    group_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hidden: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    url: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

impl Query {
    /// Matches every tab.
    pub fn new() -> Self {
        Self::default()
    }

    /// Only the tab shown in its window (`true`) or only the ones behind it (`false`).
    pub fn active(mut self, active: bool) -> Self {
        self.active = Some(active);
        self
    }

    /// Only tabs in the window this code runs in (for the background, the last focused one).
    pub fn in_current_window(mut self) -> Self {
        self.current_window = Some(true);
        self
    }

    /// Only tabs in the window the user last focused.
    pub fn in_last_focused_window(mut self) -> Self {
        self.last_focused_window = Some(true);
        self
    }

    pub fn in_window(mut self, window_id: i32) -> Self {
        self.window_id = Some(window_id);
        self
    }

    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = Some(pinned);
        self
    }

    pub fn audible(mut self, audible: bool) -> Self {
        self.audible = Some(audible);
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = Some(muted);
        self
    }

    pub fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = Some(highlighted);
        self
    }

    pub fn discarded(mut self, discarded: bool) -> Self {
        self.discarded = Some(discarded);
        self
    }

    /// Only tabs [`hide`] has hidden (`true`) or only the ones in the tab strip (`false`).
    /// Chrome hides no tabs, so there `true` matches nothing and `false` matches every tab.
    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = Some(hidden);
        self
    }

    /// Only tabs in this tab group.
    pub fn in_group(mut self, group_id: i64) -> Self {
        self.group_id = Some(group_id);
        self
    }

    /// Only tabs whose URL matches this match pattern, like `https://*.example.com/*`.
    /// Call it more than once to match any of several patterns. Needs the `Tabs`
    /// permission or host permission for the URLs.
    pub fn url(mut self, pattern: impl Into<String>) -> Self {
        self.url.push(pattern.into());
        self
    }

    /// Only tabs whose title matches this pattern (`*` matches anything).
    pub fn title(mut self, pattern: impl Into<String>) -> Self {
        self.title = Some(pattern.into());
        self
    }
}

/// The tab the user is looking at in the current window, if there is one.
pub async fn active_tab() -> Result<Option<Tab>> {
    let found: Vec<Tab> = query(&Query::new().active(true).in_current_window()).await?;
    Ok(found.into_iter().next())
}

/// Finds tabs. Pass a [`Query`] and read them as [`Tab`]s, or pass and read your own types.
pub async fn query<Q: Serialize, T: DeserializeOwned>(query_info: &Q) -> Result<Vec<T>> {
    let mut query_info = serde_json::to_value(query_info).map_err(|error| OxichromeError::Serde(error.to_string()))?;
    if !js_bridge::is_firefox() {
        if let Some(hidden) = query_info.as_object_mut().and_then(|query| query.remove("hidden")) {
            if hidden == serde_json::Value::Bool(true) {
                return Ok(Vec::new());
            }
        }
    }
    let js_query = crate::json::to_js(&query_info)?;
    let promise = js_bridge::chrome_tabs_query(&js_query)?;
    let result = JsFuture::from(promise).await?;
    let tabs: Vec<T> = crate::json::from_js(&result)?;
    Ok(tabs)
}

pub async fn create<P: Serialize, T: DeserializeOwned>(props: &P) -> Result<T> {
    let js_props = crate::json::to_js(props)?;
    let promise = js_bridge::chrome_tabs_create(&js_props)?;
    let result = JsFuture::from(promise).await?;
    let tab: T = crate::json::from_js(&result)?;
    Ok(tab)
}

pub async fn get<T: DeserializeOwned>(tab_id: i32) -> Result<T> {
    let promise = js_bridge::chrome_tabs_get(tab_id)?;
    let result = JsFuture::from(promise).await?;
    let tab: T = crate::json::from_js(&result)?;
    Ok(tab)
}

pub async fn update<P: Serialize, T: DeserializeOwned>(tab_id: i32, props: &P) -> Result<T> {
    let js_props = crate::json::to_js(props)?;
    let promise = js_bridge::chrome_tabs_update(tab_id, &js_props)?;
    let result = JsFuture::from(promise).await?;
    let tab: T = crate::json::from_js(&result)?;
    Ok(tab)
}

pub async fn remove(tab_id: i32) -> Result<()> {
    let promise = js_bridge::chrome_tabs_remove(tab_id)?;
    JsFuture::from(promise).await?;
    Ok(())
}

pub async fn send_message<M: Serialize>(tab_id: i32, message: &M) -> Result<JsValue> {
    let js_msg = crate::json::to_js(message)?;
    let no_receiver = |err| crate::runtime::receiver_error(err, crate::runtime::NO_TAB_RECEIVER);
    let promise = js_bridge::chrome_tabs_send_message(tab_id, &js_msg).map_err(no_receiver)?;
    JsFuture::from(promise).await.map_err(no_receiver)
}

/// Sends a message to the content scripts in a tab and reads the reply as `R`.
///
/// Every frame's content scripts receive it; the first to answer wins. Use
/// [`request_frame`] to reach one frame (`0` is the top frame).
pub async fn request<M: Serialize, R: DeserializeOwned>(tab_id: i32, message: &M) -> Result<R> {
    send_to(tab_id, message, None).await
}

/// Sends a message to the content scripts in one frame of a tab and reads the reply as `R`.
pub async fn request_frame<M: Serialize, R: DeserializeOwned>(tab_id: i32, frame_id: i64, message: &M) -> Result<R> {
    send_to(tab_id, message, Some(frame_id)).await
}

async fn send_to<M: Serialize, R: DeserializeOwned>(tab_id: i32, message: &M, frame_id: Option<i64>) -> Result<R> {
    let message = crate::json::to_js(message)?;
    let no_receiver = |err| crate::runtime::receiver_error(err, crate::runtime::NO_TAB_RECEIVER);
    let promise = js_bridge::tabs_send_message_to(tab_id, &message, frame_id.map(|id| id as f64)).map_err(no_receiver)?;
    crate::runtime::read_reply(JsFuture::from(promise).await.map_err(no_receiver)?)
}

/// Opens a port to the content scripts in a tab (every frame's, or one frame's with `frame_id`).
pub fn connect(tab_id: i32, name: &str, frame_id: Option<i64>) -> Result<crate::runtime::Port> {
    let raw = js_bridge::tabs_connect(tab_id, name, frame_id.map(|id| id as f64))?;
    Ok(crate::runtime::Port::wrap(&raw))
}

fn tab_error(err: JsValue) -> OxichromeError {
    let message = JsError::new(err).to_string();
    let missing = message
        .strip_prefix("No tab with id: ")
        .map(|rest| rest.trim_end_matches('.'))
        .or_else(|| message.strip_prefix("Invalid tab ID: "));
    match missing {
        Some(id) => OxichromeError::InvalidArgument(format!("no tab with id {id}")),
        None => OxichromeError::Js(JsError::new(JsValue::from_str(&message))),
    }
}

async fn call(promise: std::result::Result<js_sys::Promise, JsValue>) -> Result<JsValue> {
    JsFuture::from(promise.map_err(tab_error)?).await.map_err(tab_error)
}

/// Reloads a tab. `bypass_cache` reloads every resource from the network.
pub async fn reload(tab_id: i32, bypass_cache: bool) -> Result<()> {
    call(js_bridge::tabs_call("reload", &[tab_id.into(), crate::json::to_js(&serde_json::json!({ "bypassCache": bypass_cache }))?])).await?;
    Ok(())
}

/// Opens a copy of a tab right after it, and makes the copy active.
pub async fn duplicate(tab_id: i32) -> Result<Tab> {
    crate::json::from_js(&call(js_bridge::tabs_call("duplicate", &[tab_id.into()])).await?)
}

/// Where [`move_tab`] and [`move_tabs`] put tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    window_id: Option<i32>,
    index: i32,
}

impl Position {
    /// At `index` in the tab's own window (`0` is the first tab).
    pub fn index(index: u32) -> Self {
        Position { window_id: None, index: index.min(i32::MAX as u32) as i32 }
    }

    /// After the last tab.
    pub fn end() -> Self {
        Position { window_id: None, index: -1 }
    }

    /// In another window (the same index rules apply).
    pub fn in_window(mut self, window_id: i32) -> Self {
        self.window_id = Some(window_id);
        self
    }

    pub(crate) fn to_js(self) -> Result<JsValue> {
        let mut props = serde_json::json!({ "index": self.index });
        if let Some(window_id) = self.window_id {
            props["windowId"] = window_id.into();
        }
        crate::json::to_js(&props)
    }
}

/// Moves a tab and returns it at its new place. An index past the end puts it last.
pub async fn move_tab(tab_id: i32, to: Position) -> Result<Tab> {
    let mut moved = move_tabs(&[tab_id], to).await?;
    moved.pop().ok_or_else(|| OxichromeError::UnexpectedValue("the browser moved no tab".into()))
}

/// Moves several tabs, keeping their order, and returns them at their new places.
pub async fn move_tabs(tab_ids: &[i32], to: Position) -> Result<Vec<Tab>> {
    if tab_ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids = crate::json::to_js(tab_ids)?;
    let moved = call(js_bridge::tabs_call("move", &[ids, to.to_js()?])).await?;
    if js_sys::Array::is_array(&moved) {
        crate::json::from_js(&moved)
    } else {
        Ok(vec![crate::json::from_js(&moved)?])
    }
}

/// Unloads a tab to free memory; it stays in the tab strip and loads again when selected.
///
/// The active tab can't be discarded (Firefox ignores it, so oxichrome refuses it in both).
/// Discarding a tab that is already discarded returns it unchanged.
pub async fn discard(tab_id: i32) -> Result<Tab> {
    let tab: Tab = get(tab_id).await.map_err(|err| match err {
        OxichromeError::Js(js) => tab_error(JsValue::from_str(&js.to_string())),
        other => other,
    })?;
    if tab.active {
        return Err(OxichromeError::InvalidArgument(format!("tab {tab_id} is the active tab in its window, which can't be discarded")));
    }
    if tab.discarded {
        return Ok(tab);
    }
    call(js_bridge::tabs_call("discard", &[tab_id.into()])).await?;
    let mut tab: Tab = crate::json::from_js(&call(js_bridge::tabs_call("get", &[tab_id.into()])).await?)?;
    tab.discarded = true;
    Ok(tab)
}

/// Selects (highlights) tabs by index in a window, `None` for the current window.
/// The first index becomes the active tab.
pub async fn highlight(window_id: Option<i32>, indexes: &[u32]) -> Result<crate::windows::Window> {
    if indexes.is_empty() {
        return Err(OxichromeError::InvalidArgument("highlight needs at least one tab index".into()));
    }
    let mut info = serde_json::json!({ "tabs": indexes });
    if let Some(window_id) = window_id {
        info["windowId"] = window_id.into();
    }
    let window = call(js_bridge::tabs_call("highlight", &[crate::json::to_js(&info)?])).await.map_err(|err| match err {
        OxichromeError::Js(js) if js.to_string().starts_with("No tab at index") => {
            OxichromeError::InvalidArgument(format!("{} in that window", js.to_string().trim_end_matches('.').replace("No tab at index:", "no tab at index")))
        }
        other => other,
    })?;
    crate::json::from_js(&window)
}

/// The image format for [`capture_visible_tab`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    /// JPEG with a quality from 0 to 100.
    Jpeg(u8),
}

/// Takes a screenshot of the active tab in a window (`None` for the current window)
/// and returns it as a `data:` URL.
///
/// Needs `ActiveTab` (after the user invokes the extension) or host permission for the page.
/// Chrome allows two captures a second; oxichrome waits and retries instead of failing.
pub async fn capture_visible_tab(window_id: Option<i32>, format: ImageFormat) -> Result<String> {
    let options = match format {
        ImageFormat::Png => serde_json::json!({ "format": "png" }),
        ImageFormat::Jpeg(quality) if quality <= 100 => serde_json::json!({ "format": "jpeg", "quality": quality }),
        ImageFormat::Jpeg(quality) => {
            return Err(OxichromeError::InvalidArgument(format!("JPEG quality is 0 to 100, got {quality}")));
        }
    };
    let window = window_id.map(JsValue::from).unwrap_or(JsValue::UNDEFINED);
    let promise = js_bridge::tabs_capture(&window, &crate::json::to_js(&options)?).map_err(capture_error)?;
    let url = JsFuture::from(promise).await.map_err(capture_error)?;
    url.as_string().ok_or_else(|| OxichromeError::UnexpectedValue(format!("expected a data URL, got {url:?}")))
}

fn capture_error(err: JsValue) -> OxichromeError {
    let message = JsError::new(err).to_string();
    if message.contains("'<all_urls>' or 'activeTab'") || message.contains("captureVisibleTab is not a function") || message.contains("Missing activeTab permission") || message.contains("Missing host permission") {
        return OxichromeError::InvalidArgument(
            "capturing a tab needs `ActiveTab` in `permissions` (after the user clicks the toolbar button, a menu item or a shortcut) or host permission for the page".to_string(),
        );
    }
    if let Some(id) = message.strip_prefix("Invalid window ID: ").or_else(|| message.strip_prefix("No window with id: ")) {
        return OxichromeError::InvalidArgument(format!("no window with id {}", id.trim_end_matches('.')));
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

/// Hides tabs from the tab strip and returns the ids it hid. Firefox only, with `TabHide` in
/// `permissions`.
///
/// The active tab, pinned tabs and tabs already hidden are skipped, not refused. One unknown id
/// fails the whole call and hides nothing. A hidden tab still runs and stays in [`query`] (see
/// [`Query::hidden`]); it comes back when it's made active, when [`show`] is called, or when the
/// extension is turned off. Firefox tells the user the first time an extension hides a tab.
pub async fn hide(tab_ids: &[i32]) -> Result<Vec<i32>> {
    crate::call::only(crate::call::Only::Firefox, "tabs.hide", HIDE_INSTEAD)?;
    if tab_ids.is_empty() {
        return Ok(Vec::new());
    }
    crate::json::from_js(&visibility("hide", tab_ids).await?)
}

/// Puts hidden tabs back in the tab strip. Tabs that aren't hidden are left alone; one unknown
/// id fails the whole call. Firefox only, with `TabHide` in `permissions`.
pub async fn show(tab_ids: &[i32]) -> Result<()> {
    crate::call::only(crate::call::Only::Firefox, "tabs.show", HIDE_INSTEAD)?;
    if tab_ids.is_empty() {
        return Ok(());
    }
    visibility("show", tab_ids).await.map(|_| ())
}

const HIDE_INSTEAD: &str = "Chrome can't hide tabs; put them in a tab group and collapse it instead";

async fn visibility(method: &str, tab_ids: &[i32]) -> Result<JsValue> {
    let ids = crate::json::to_js(tab_ids)?;
    let promise = js_bridge::api_call("tabs", method, &js_sys::Array::of1(&ids)).map_err(|error| {
        if JsError::new(error.clone()).to_string() == "oxichrome:no-permission" {
            OxichromeError::InvalidArgument(format!("`tabs.{method}` needs `TabHide` in `permissions` in #[oxichrome::extension]"))
        } else {
            tab_error(error)
        }
    })?;
    JsFuture::from(promise).await.map_err(tab_error)
}

/// Puts tabs in a new group in their window and returns the group id.
pub async fn group(tab_ids: &[i32]) -> Result<i64> {
    group_with(tab_ids, serde_json::json!({})).await
}

/// Adds tabs to an existing group and returns its id.
pub async fn add_to_group(group_id: i64, tab_ids: &[i32]) -> Result<i64> {
    group_with(tab_ids, serde_json::json!({ "groupId": group_id })).await
}

/// Puts tabs in a new group in another window (they move there) and returns the group id.
pub async fn group_in_window(window_id: i32, tab_ids: &[i32]) -> Result<i64> {
    group_with(tab_ids, serde_json::json!({ "createProperties": { "windowId": window_id } })).await
}

async fn group_with(tab_ids: &[i32], mut options: serde_json::Value) -> Result<i64> {
    if tab_ids.is_empty() {
        return Err(OxichromeError::InvalidArgument("a group needs at least one tab".into()));
    }
    options["tabIds"] = serde_json::json!(tab_ids);
    let id = call(js_bridge::tabs_group_call("group", &crate::json::to_js(&options)?)).await.map_err(crate::tab_groups::group_error)?;
    id.as_f64().map(|id| id as i64).ok_or_else(|| OxichromeError::UnexpectedValue(format!("expected a group id, got {id:?}")))
}

/// Takes tabs out of their groups. Tabs that aren't in a group are left alone; a group
/// whose last tab leaves is removed.
pub async fn ungroup(tab_ids: &[i32]) -> Result<()> {
    if tab_ids.is_empty() {
        return Ok(());
    }
    call(js_bridge::tabs_group_call("ungroup", &crate::json::to_js(tab_ids)?)).await.map_err(crate::tab_groups::group_error)?;
    Ok(())
}

#[cfg(test)]
mod query_tests {
    use super::Query;

    #[test]
    fn query_sends_only_what_was_set() {
        assert_eq!(serde_json::to_value(Query::new()).unwrap(), serde_json::json!({}));
        let query = Query::new().active(true).in_current_window().url("https://*.example.com/*").url("*://rust-lang.org/*").in_group(4).hidden(false);
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            serde_json::json!({
                "active": true,
                "currentWindow": true,
                "groupId": 4,
                "hidden": false,
                "url": ["https://*.example.com/*", "*://rust-lang.org/*"],
            })
        );
    }
}
