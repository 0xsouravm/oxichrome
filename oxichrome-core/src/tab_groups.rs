use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

/// A group's color, the same nine in Chrome and Firefox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    Grey,
    Blue,
    Red,
    Yellow,
    Green,
    Pink,
    Purple,
    Cyan,
    Orange,
}

/// A tab group.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Group {
    pub id: i64,
    #[serde(default)]
    pub title: String,
    pub color: Color,
    #[serde(default)]
    pub collapsed: bool,
    pub window_id: i32,
}

impl FromEventArg for Group {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        crate::json::from_js(&value)
    }
}

impl EventArg<events::TabGroupsOnCreated, 1> for Group {}
impl EventArg<events::TabGroupsOnUpdated, 1> for Group {}
impl EventArg<events::TabGroupsOnRemoved, 1> for Group {}
impl EventArg<events::TabGroupsOnMoved, 1> for Group {}

pub(crate) fn group_error(err: OxichromeError) -> OxichromeError {
    let OxichromeError::Js(js) = &err else { return err };
    let message = js.to_string();
    if message == "oxichrome:no-tab-groups" {
        return OxichromeError::InvalidArgument(
            "the tab groups API isn't available; add `TabGroups` to `permissions` in #[oxichrome::extension] (if it's optional, request it first)".to_string(),
        );
    }
    if let Some(id) = message.strip_prefix("No group with id: ") {
        return OxichromeError::InvalidArgument(format!("no tab group with id {}", id.trim_end_matches('.')));
    }
    err
}

async fn call(method: &str, args: &[JsValue]) -> Result<JsValue> {
    let promise = js_bridge::tab_groups_call(method, &args.iter().cloned().collect::<js_sys::Array>())
        .map_err(|err| group_error(OxichromeError::Js(JsError::new(err))))?;
    JsFuture::from(promise).await.map_err(|err| group_error(OxichromeError::Js(JsError::new(err))))
}

/// One group.
pub async fn get(group_id: i64) -> Result<Group> {
    crate::json::from_js(&call("get", &[JsValue::from_f64(group_id as f64)]).await?)
}

/// Which groups [`query`] returns; every field left out matches all.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_id: Option<i32>,
}

/// The groups that match, in no particular order.
pub async fn query(query: &Query) -> Result<Vec<Group>> {
    crate::json::from_js(&call("query", &[crate::json::to_js(query)?]).await?)
}

/// What [`update`] changes; fields left out stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
}

/// Changes a group's title, color or collapsed state and returns it.
pub async fn update(group_id: i64, changes: &Update) -> Result<Group> {
    crate::json::from_js(&call("update", &[JsValue::from_f64(group_id as f64), crate::json::to_js(changes)?]).await?)
}

/// Moves a whole group, keeping its tabs together, and returns it.
pub async fn move_group(group_id: i64, to: crate::tabs::Position) -> Result<Group> {
    crate::json::from_js(&call("move", &[JsValue::from_f64(group_id as f64), to.to_js()?]).await?)
}
