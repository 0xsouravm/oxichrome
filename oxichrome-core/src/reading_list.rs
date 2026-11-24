//! Chrome's reading list. Chrome only, with `ReadingList` in `permissions`.
//!
//! Firefox has no reading list API; [`add`] and the rest fail there saying so.

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

use crate::call::{self, Only};
use crate::error::{OxichromeError, Result};
pub use crate::runtime::Listener;

const API: &str = "readingList";
const INSTEAD: &str = "keep the list in `storage` there, or use `bookmarks`";

/// A page on the reading list.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub url: String,
    pub title: String,
    pub has_been_read: bool,
    /// When it was added, in milliseconds since 1970.
    pub creation_time: f64,
    pub last_update_time: f64,
}

/// Which entries [`query`] finds; every part given has to match.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_been_read: Option<bool>,
}

impl Query {
    /// Every entry.
    pub fn all() -> Self {
        Self::default()
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn read(mut self, read: bool) -> Self {
        self.has_been_read = Some(read);
        self
    }
}

async fn run(method: &str, details: &impl Serialize) -> Result<JsValue> {
    call::only(Only::Chrome, API, INSTEAD)?;
    call::call(API, method, &[crate::json::to_js(details)?], "ReadingList").await
}

fn check_url(url: &str) -> Result<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(OxichromeError::InvalidArgument(format!("the reading list takes http and https pages, not `{url}`")));
    }
    Ok(())
}

/// Adds a page; fails when it is already on the list.
pub async fn add(url: &str, title: &str, read: bool) -> Result<()> {
    check_url(url)?;
    run("addEntry", &serde_json::json!({ "url": url, "title": title, "hasBeenRead": read })).await.map(|_| ())
}

/// Changes an entry's title, or marks it read or unread.
pub async fn update(url: &str, title: Option<&str>, read: Option<bool>) -> Result<()> {
    check_url(url)?;
    if title.is_none() && read.is_none() {
        return Err(OxichromeError::InvalidArgument("there is nothing to change; give a title or whether it is read".into()));
    }
    let mut details = serde_json::json!({ "url": url });
    if let Some(title) = title {
        details["title"] = title.into();
    }
    if let Some(read) = read {
        details["hasBeenRead"] = read.into();
    }
    run("updateEntry", &details).await.map(|_| ())
}

pub async fn remove(url: &str) -> Result<()> {
    check_url(url)?;
    run("removeEntry", &serde_json::json!({ "url": url })).await.map(|_| ())
}

pub async fn query(query: Query) -> Result<Vec<Entry>> {
    crate::json::from_js(&run("query", &query).await?)
}

/// What happened to the list.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Change {
    Added(Entry),
    Updated(Entry),
    Removed(Entry),
}

/// Runs whenever an entry is added, changed or removed.
pub fn on_changed(handler: impl Fn(Change) + 'static) -> Result<Vec<Listener>> {
    call::only(Only::Chrome, API, INSTEAD)?;
    let handler = std::rc::Rc::new(handler);
    let mut listeners = Vec::new();
    for (event, change) in [("onEntryAdded", Change::Added as fn(Entry) -> Change), ("onEntryUpdated", Change::Updated), ("onEntryRemoved", Change::Removed)] {
        let handler = handler.clone();
        let callback = wasm_bindgen::closure::Closure::<dyn Fn(JsValue)>::new(move |entry: JsValue| {
            if let Ok(entry) = crate::json::from_js::<Entry>(&entry) {
                handler(change(entry));
            }
        })
        .into_js_value();
        listeners.push(Listener::add(API, event, &callback).ok_or_else(|| {
            OxichromeError::InvalidArgument("`readingList` isn't available here; add `ReadingList` to `permissions` in #[oxichrome::extension]".into())
        })?);
    }
    Ok(listeners)
}
