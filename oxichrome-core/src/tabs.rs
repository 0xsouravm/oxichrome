use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::Result;
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Tab {
    pub id: Option<i32>,
    #[serde(default)]
    pub index: i32,
    #[serde(default = "no_window")]
    pub window_id: i32,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub highlighted: bool,
    #[serde(default)]
    pub incognito: bool,
    #[serde(default)]
    pub discarded: bool,
    #[serde(default)]
    pub audible: bool,
    pub url: Option<String>,
    pub title: Option<String>,
    pub fav_icon_url: Option<String>,
    pub status: Option<String>,
    pub opener_tab_id: Option<i32>,
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

pub async fn query<Q: Serialize, T: DeserializeOwned>(query_info: &Q) -> Result<Vec<T>> {
    let js_query = crate::json::to_js(query_info)?;
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
    let promise = js_bridge::chrome_tabs_send_message(tab_id, &js_msg)?;
    let result = JsFuture::from(promise).await?;
    Ok(result)
}
