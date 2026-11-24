//! Firefox containers (contextual identities): separate sets of cookies and storage, like
//! "Work" and "Personal", each shown with its own color and icon.
//!
//! Firefox only, with `ContextualIdentities` in `permissions` (add `Cookies` to read their
//! cookies). Open a tab in one with `tabs::create` and its `cookie_store_id`.

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

use crate::call::{self, Only};
use crate::error::{OxichromeError, Result};
pub use crate::runtime::Listener;

const API: &str = "contextualIdentities";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Color {
    Blue,
    Turquoise,
    Green,
    Yellow,
    Orange,
    Red,
    Pink,
    Purple,
    /// The toolbar's own color.
    Toolbar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Icon {
    Fingerprint,
    Briefcase,
    Dollar,
    Cart,
    Circle,
    Gift,
    Vacation,
    Food,
    Fruit,
    Pet,
    Tree,
    Chill,
    Fence,
}

/// A container.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    /// Its cookie store, like `firefox-container-1`; give it to `tabs::create` and `cookies`.
    pub cookie_store_id: String,
    pub name: String,
    pub color: Color,
    /// The color as CSS, like `#37adff`.
    pub color_code: String,
    pub icon: Icon,
    pub icon_url: String,
}

async fn run(method: &str, args: &[JsValue]) -> Result<JsValue> {
    call::only(Only::Firefox, API, "Chrome keeps one set of cookies per profile")?;
    call::call(API, method, args, "ContextualIdentities").await
}

fn check_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(OxichromeError::InvalidArgument("a container needs a name".into()));
    }
    Ok(())
}

/// Every container, in the order Firefox shows them.
pub async fn all() -> Result<Vec<Container>> {
    crate::json::from_js(&run("query", &[js_sys::Object::new().into()]).await?)
}

/// The containers with this name.
pub async fn named(name: &str) -> Result<Vec<Container>> {
    crate::json::from_js(&run("query", &[crate::json::to_js(&serde_json::json!({ "name": name }))?]).await?)
}

pub async fn get(cookie_store_id: &str) -> Result<Container> {
    crate::json::from_js(&run("get", &[cookie_store_id.into()]).await?)
}

pub async fn create(name: &str, color: Color, icon: Icon) -> Result<Container> {
    check_name(name)?;
    let details = crate::json::to_js(&serde_json::json!({ "name": name, "color": color, "icon": icon }))?;
    crate::json::from_js(&run("create", &[details]).await?)
}

/// What to change in [`update`]; parts left `None` stay.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Changes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
}

pub async fn update(cookie_store_id: &str, changes: Changes) -> Result<Container> {
    if let Some(name) = &changes.name {
        check_name(name)?;
    }
    let changes = crate::json::to_js(&changes)?;
    crate::json::from_js(&run("update", &[cookie_store_id.into(), changes]).await?)
}

/// Removes the container. Firefox leaves tabs that are open in it open; close them first
/// (`tabs::query` with its `cookie_store_id`) if they should go too.
pub async fn remove(cookie_store_id: &str) -> Result<Container> {
    crate::json::from_js(&run("remove", &[cookie_store_id.into()]).await?)
}

/// Moves containers to `position` in Firefox's list (0 is first; `-1` the end).
pub async fn move_to(cookie_store_ids: &[&str], position: i32) -> Result<()> {
    let ids = js_sys::Array::from_iter(cookie_store_ids.iter().map(|id| JsValue::from_str(id)));
    run("move", &[ids.into(), position.into()]).await.map(|_| ())
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Change {
    Created(Container),
    Updated(Container),
    Removed(Container),
}

/// Runs whenever a container is created, changed or removed.
pub fn on_changed(handler: impl Fn(Change) + 'static) -> Result<Vec<Listener>> {
    call::only(Only::Firefox, API, "")?;
    let handler = std::rc::Rc::new(handler);
    let mut listeners = Vec::new();
    for (event, change) in [("onCreated", Change::Created as fn(Container) -> Change), ("onUpdated", Change::Updated), ("onRemoved", Change::Removed)] {
        let handler = handler.clone();
        let callback = wasm_bindgen::closure::Closure::<dyn Fn(JsValue)>::new(move |info: JsValue| {
            let container = js_sys::Reflect::get(&info, &"contextualIdentity".into()).unwrap_or(JsValue::UNDEFINED);
            if let Ok(container) = crate::json::from_js::<Container>(&container) {
                handler(change(container));
            }
        })
        .into_js_value();
        listeners.push(Listener::add(API, event, &callback).ok_or_else(|| {
            OxichromeError::InvalidArgument("`contextualIdentities` isn't available; add `ContextualIdentities` to `permissions` in #[oxichrome::extension]".into())
        })?);
    }
    Ok(listeners)
}
