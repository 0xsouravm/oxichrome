use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

const CHROME_ROOT: &str = "0";
const FIREFOX_ROOT: &str = "root________";
const FIREFOX_TAGS: &str = "tags________";
const MAX_TITLE: usize = 4096;
const MAX_URL: usize = 65536;

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub parent_id: Option<String>,
    pub index: Option<u32>,
    pub title: String,
    pub kind: Kind,
    pub date_added: Option<f64>,
    pub special: Option<Special>,
    pub managed: bool,
    pub children: Option<Vec<Node>>,
}

impl Node {
    pub fn url(&self) -> Option<&str> {
        match &self.kind {
            Kind::Bookmark { url } => Some(url),
            _ => None,
        }
    }

    pub fn is_folder(&self) -> bool {
        self.kind == Kind::Folder
    }

    pub fn is_top_level(&self) -> bool {
        self.parent_id.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Bookmark { url: String },
    Folder,
    Separator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Special {
    Toolbar,
    Other,
    Mobile,
    Menu,
    Managed,
}

impl Special {
    fn describe(self) -> &'static str {
        match self {
            Special::Toolbar => "bookmarks toolbar",
            Special::Other => "other bookmarks",
            Special::Mobile => "mobile bookmarks",
            Special::Menu => "bookmarks menu",
            Special::Managed => "managed bookmarks",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    pub id: String,
    pub parent_id: String,
    pub index: u32,
    pub title: String,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub id: String,
    pub parent_id: String,
    pub index: u32,
    pub old_parent_id: String,
    pub old_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct New {
    parent_id: String,
    title: String,
    url: Option<String>,
    index: Option<u32>,
}

impl New {
    pub fn bookmark(parent_id: impl Into<String>, title: impl Into<String>, url: impl Into<String>) -> Self {
        New { parent_id: parent_id.into(), title: title.into(), url: Some(url.into()), index: None }
    }

    pub fn folder(parent_id: impl Into<String>, title: impl Into<String>) -> Self {
        New { parent_id: parent_id.into(), title: title.into(), url: None, index: None }
    }

    pub fn index(mut self, index: u32) -> Self {
        self.index = Some(index);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Change {
    title: Option<String>,
    url: Option<String>,
}

impl Change {
    pub fn new() -> Self {
        Change::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Search {
    text: Option<String>,
    url: Option<String>,
    title: Option<String>,
}

impl Search {
    pub fn new() -> Self {
        Search::default()
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNode {
    id: String,
    parent_id: Option<String>,
    index: Option<f64>,
    title: Option<String>,
    url: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    date_added: Option<f64>,
    folder_type: Option<String>,
    unmodifiable: Option<String>,
    children: Option<Vec<RawNode>>,
}

fn is_root(id: &str) -> bool {
    id == CHROME_ROOT || id == FIREFOX_ROOT
}

fn whole(value: f64, what: &str) -> Result<u32> {
    if value.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&value) {
        Ok(value as u32)
    } else {
        Err(OxichromeError::UnexpectedValue(format!("expected a {what}, got {value}")))
    }
}

impl RawNode {
    fn special(&self) -> Option<Special> {
        match self.parent_id.as_deref() {
            Some(FIREFOX_ROOT) => match self.id.as_str() {
                "toolbar_____" => Some(Special::Toolbar),
                "unfiled_____" => Some(Special::Other),
                "mobile______" => Some(Special::Mobile),
                "menu________" => Some(Special::Menu),
                _ => None,
            },
            Some(CHROME_ROOT) => match self.folder_type.as_deref() {
                Some("bookmarks-bar") => Some(Special::Toolbar),
                Some("other") => Some(Special::Other),
                Some("mobile") => Some(Special::Mobile),
                Some("managed") => Some(Special::Managed),
                Some(_) => None,
                None if self.unmodifiable.as_deref() == Some("managed") => Some(Special::Managed),
                None => match self.id.as_str() {
                    "1" => Some(Special::Toolbar),
                    "2" => Some(Special::Other),
                    "3" => Some(Special::Mobile),
                    _ => None,
                },
            },
            _ => None,
        }
    }

    fn kind(&mut self) -> Kind {
        match (self.kind.as_deref(), self.url.take()) {
            (Some("separator"), _) => Kind::Separator,
            (Some("folder"), _) | (_, None) => Kind::Folder,
            (_, Some(url)) => Kind::Bookmark { url },
        }
    }

    fn node(mut self, deep: bool) -> Result<Node> {
        let special = self.special();
        let kind = self.kind();
        let children = match (deep, &kind) {
            (true, Kind::Folder) => Some(
                self.children
                    .take()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|child| child.id != FIREFOX_TAGS)
                    .map(|child| child.node(true))
                    .collect::<Result<Vec<_>>>()?,
            ),
            _ => None,
        };
        Ok(Node {
            parent_id: self.parent_id.filter(|parent| !is_root(parent)),
            index: self.index.map(|index| whole(index, "bookmark index")).transpose()?,
            title: self.title.unwrap_or_default(),
            date_added: self.date_added,
            managed: self.unmodifiable.as_deref() == Some("managed"),
            special,
            kind,
            children,
            id: self.id,
        })
    }
}

fn parse<T: DeserializeOwned>(value: &JsValue) -> Result<T> {
    crate::json::from_js(value)
}

impl FromEventArg for Node {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        parse::<RawNode>(&value)?.node(false)
    }
}

impl FromEventArg for Removed {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let mut raw: RawNode = parse(&value)?;
        let kind = raw.kind();
        Ok(Removed {
            parent_id: raw.parent_id.ok_or_else(|| OxichromeError::UnexpectedValue("the removed bookmark has no parent id".to_string()))?,
            index: whole(raw.index.ok_or_else(|| OxichromeError::UnexpectedValue("the removed bookmark has no index".to_string()))?, "bookmark index")?,
            title: raw.title.unwrap_or_default(),
            kind,
            id: raw.id,
        })
    }
}

impl FromEventArg for Changed {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let node = Node::from_event_arg(value)?;
        Ok(Changed { url: node.url().map(str::to_string), title: node.title, id: node.id })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMoved {
    id: String,
    parent_id: String,
    index: f64,
    old_parent_id: String,
    old_index: f64,
}

impl FromEventArg for Moved {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let raw: RawMoved = parse(&value)?;
        Ok(Moved {
            id: raw.id,
            parent_id: raw.parent_id,
            index: whole(raw.index, "bookmark index")?,
            old_parent_id: raw.old_parent_id,
            old_index: whole(raw.old_index, "bookmark index")?,
        })
    }
}

impl EventArg<events::BookmarksOnCreated, 1> for Node {}
impl EventArg<events::BookmarksOnRemoved, 1> for Removed {}
impl EventArg<events::BookmarksOnChanged, 1> for Changed {}
impl EventArg<events::BookmarksOnMoved, 1> for Moved {}

fn invalid(message: String) -> OxichromeError {
    OxichromeError::InvalidArgument(message)
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    let Some(code) = message.strip_prefix("oxichrome:") else {
        return OxichromeError::Js(JsError::new(JsValue::from_str(&message)));
    };
    let (code, detail) = code.split_once(':').unwrap_or((code, ""));
    match code {
        "no-permission" => invalid(
            "the bookmarks API isn't available; add `Bookmarks` to `permissions` in #[oxichrome::extension] (if it's optional, request it first)".to_string(),
        ),
        "not-found" => invalid(format!("there's no bookmark with id `{detail}`")),
        "root" => invalid(
            "bookmarks can't go directly in the root; put them in a top-level folder, like `bookmarks::folder(Special::Other)`".to_string(),
        ),
        "not-folder" => invalid(format!("`{detail}` isn't a folder, so nothing can go inside it")),
        "top-level" => invalid(format!("`{detail}` is one of the browser's top-level folders, which can't be changed, moved or removed")),
        "managed" => invalid(format!("`{detail}` is managed by an administrator and can't be changed")),
        "separator" => invalid(format!("`{detail}` is a separator, which has no title or url to change")),
        "folder-url" => invalid(format!("`{detail}` is a folder, which can't have a url")),
        "into-self" => invalid(format!("`{detail}` can't be moved into itself or a folder inside it")),
        "not-empty" => invalid(format!("the folder `{detail}` isn't empty; use `bookmarks::remove_tree` to remove it with everything inside")),
        "index" => {
            let (index, last) = detail.split_once(':').unwrap_or((detail, "?"));
            invalid(format!("index {index} is past the end of the folder; the highest it can be here is {last}"))
        }
        _ => OxichromeError::Js(JsError::new(JsValue::from_str(&message))),
    }
}

async fn call(op: &str, args: serde_json::Value) -> Result<JsValue> {
    let promise = js_bridge::bookmarks_call(op, &args.to_string()).map_err(browser_error)?;
    let text = JsFuture::from(promise).await.map_err(browser_error)?;
    let text = text.as_string().unwrap_or_else(|| "null".to_string());
    Ok(js_sys::JSON::parse(&text)?)
}

async fn one(op: &str, args: serde_json::Value, deep: bool) -> Result<Node> {
    parse::<RawNode>(&call(op, args).await?)?.node(deep)
}

async fn many(op: &str, args: serde_json::Value, deep: bool) -> Result<Vec<Node>> {
    parse::<Vec<RawNode>>(&call(op, args).await?)?.into_iter().map(|raw| raw.node(deep)).collect()
}

fn check_id(id: &str, what: &str) -> Result<()> {
    if id.trim().is_empty() {
        return Err(invalid(format!("the {what} can't be empty")));
    }
    Ok(())
}

fn clean_title(title: &str) -> Result<String> {
    let title: String = title.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if title.chars().count() > MAX_TITLE {
        return Err(invalid(format!("the title is {} characters long; bookmarks keep at most {MAX_TITLE}", title.chars().count())));
    }
    Ok(title)
}

fn check_url(url: &str) -> Result<()> {
    if url.is_empty() {
        return Err(invalid("the url can't be empty".to_string()));
    }
    if let Some(bad) = url.chars().find(|c| c.is_whitespace() || c.is_control()) {
        return Err(invalid(format!("the url `{url}` contains {bad:?}; encode spaces and other special characters, like `%20` for a space")));
    }
    if url.len() > MAX_URL {
        return Err(invalid(format!("the url is {} bytes long; bookmarks keep at most {MAX_URL}", url.len())));
    }
    let scheme = url.split_once(':').map(|(scheme, _)| scheme).unwrap_or("");
    let mut chars = scheme.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
    if !valid {
        return Err(invalid(format!("`{url}` isn't a full url; start it with a scheme, like `https://`")));
    }
    Ok(())
}

pub async fn tree() -> Result<Vec<Node>> {
    many("top", json!({}), true).await
}

pub async fn folder(which: Special) -> Result<Node> {
    many("top", json!({}), false)
        .await?
        .into_iter()
        .find(|node| node.special == Some(which))
        .ok_or_else(|| invalid(format!("this browser has no {} folder", which.describe())))
}

pub async fn get(id: &str) -> Result<Option<Node>> {
    check_id(id, "bookmark id")?;
    match one("get", json!({ "id": id }), false).await {
        Ok(node) => Ok(Some(node)),
        Err(OxichromeError::InvalidArgument(message)) if message.starts_with("there's no bookmark") => Ok(None),
        Err(other) => Err(other),
    }
}

pub async fn children(folder_id: &str) -> Result<Vec<Node>> {
    check_id(folder_id, "folder id")?;
    many("children", json!({ "id": folder_id }), false).await
}

pub async fn subtree(id: &str) -> Result<Node> {
    check_id(id, "bookmark id")?;
    one("subtree", json!({ "id": id }), true).await
}

pub async fn recent(count: u32) -> Result<Vec<Node>> {
    if count == 0 {
        return Err(invalid("ask for at least 1 recent bookmark".to_string()));
    }
    many("recent", json!({ "count": count.min(i32::MAX as u32) }), false).await
}

pub async fn search(query: &Search) -> Result<Vec<Node>> {
    if query.text.is_none() && query.url.is_none() && query.title.is_none() {
        return Err(invalid("the search is empty; give it text, a url or a title".to_string()));
    }
    if let Some(text) = &query.text {
        if text.trim().is_empty() {
            return Err(invalid("the search text can't be blank; leave it out to search by url or title alone".to_string()));
        }
    }
    if let Some(url) = &query.url {
        check_url(url)?;
    }
    let title = query.title.as_deref().map(clean_title).transpose()?;
    many("search", json!({ "query": query.text, "url": query.url, "title": title }), false).await
}

pub async fn create(new: &New) -> Result<Node> {
    check_id(&new.parent_id, "parent folder id")?;
    if let Some(url) = &new.url {
        check_url(url)?;
    }
    let title = clean_title(&new.title)?;
    one("create", json!({ "parentId": new.parent_id, "title": title, "url": new.url, "index": new.index }), false).await
}

pub async fn update(id: &str, change: &Change) -> Result<Node> {
    check_id(id, "bookmark id")?;
    if let Some(url) = &change.url {
        check_url(url)?;
    }
    let title = change.title.as_deref().map(clean_title).transpose()?;
    one("update", json!({ "id": id, "change": { "title": title, "url": change.url } }), false).await
}

pub async fn move_to(id: &str, parent_id: &str, index: Option<u32>) -> Result<Node> {
    check_id(id, "bookmark id")?;
    check_id(parent_id, "parent folder id")?;
    one("move", json!({ "id": id, "parentId": parent_id, "index": index }), false).await
}

pub async fn remove(id: &str) -> Result<()> {
    check_id(id, "bookmark id")?;
    call("remove", json!({ "id": id })).await.map(|_| ())
}

pub async fn remove_tree(id: &str) -> Result<()> {
    check_id(id, "bookmark id")?;
    call("remove_tree", json!({ "id": id })).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> RawNode {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn chrome_and_firefox_nodes_read_the_same() {
        let chrome = raw(r#"{"id":"1","parentId":"0","index":0,"title":"Bookmarks bar","dateAdded":1,"folderType":"bookmarks-bar","children":[{"id":"5","parentId":"1","index":0,"title":"a","url":"https://a.com/","dateAdded":2}]}"#).node(true).unwrap();
        let firefox = raw(r#"{"id":"toolbar_____","parentId":"root________","index":1,"title":"Bookmarks Toolbar","type":"folder","dateAdded":1,"children":[{"id":"abc","parentId":"toolbar_____","index":0,"title":"a","url":"https://a.com/","type":"bookmark","dateAdded":2}]}"#).node(true).unwrap();
        for node in [&chrome, &firefox] {
            assert_eq!(node.special, Some(Special::Toolbar));
            assert!(node.is_top_level());
            assert!(node.is_folder());
            let kids = node.children.as_ref().unwrap();
            assert_eq!(kids[0].url(), Some("https://a.com/"));
            assert_eq!(kids[0].special, None);
            assert_eq!(kids[0].children, None);
        }
        assert_eq!(chrome.children.unwrap()[0].parent_id.as_deref(), Some("1"));
    }

    #[test]
    fn chrome_without_folder_types_uses_ids() {
        assert_eq!(raw(r#"{"id":"2","parentId":"0"}"#).node(false).unwrap().special, Some(Special::Other));
        assert_eq!(raw(r#"{"id":"3","parentId":"0"}"#).node(false).unwrap().special, Some(Special::Mobile));
        assert_eq!(raw(r#"{"id":"9","parentId":"0","unmodifiable":"managed"}"#).node(false).unwrap().special, Some(Special::Managed));
        assert_eq!(raw(r#"{"id":"2","parentId":"1"}"#).node(false).unwrap().special, None);
        assert_eq!(raw(r#"{"id":"menu________","parentId":"root________","type":"folder"}"#).node(false).unwrap().special, Some(Special::Menu));
    }

    #[test]
    fn separators_and_empty_folders() {
        let separator = raw(r#"{"id":"s","parentId":"p","index":3,"title":"","url":"data:","type":"separator"}"#).node(true).unwrap();
        assert_eq!(separator.kind, Kind::Separator);
        assert_eq!(separator.url(), None);
        assert_eq!(separator.children, None);
        let folder = raw(r#"{"id":"f","parentId":"p","title":"x"}"#).node(true).unwrap();
        assert_eq!(folder.children, Some(vec![]));
        assert_eq!(raw(r#"{"id":"f","parentId":"p","title":"x"}"#).node(false).unwrap().children, None);
        let tags = raw(r#"{"id":"root________","type":"folder","children":[{"id":"tags________","parentId":"root________","type":"folder"},{"id":"menu________","parentId":"root________","type":"folder"}]}"#).node(true).unwrap();
        assert_eq!(tags.children.unwrap().len(), 1);
        assert!(raw(r#"{"id":"x","index":1.5}"#).node(false).is_err());
    }

    #[test]
    fn titles_and_urls_are_checked() {
        assert_eq!(clean_title("a\r\nb\tc\u{0}").unwrap(), "a  b c ");
        assert!(clean_title(&"é".repeat(MAX_TITLE)).is_ok());
        assert!(clean_title(&"é".repeat(MAX_TITLE + 1)).is_err());
        for good in ["https://a.com/", "javascript:void(0)", "about:blank", "x-y.z+w:stuff", "file:///tmp/a"] {
            assert!(check_url(good).is_ok(), "{good}");
        }
        for bad in ["", "a.com", "https://a.com/a b", "https://a.com/\n", "1http://a", ":x", "ht tp://a", "https://a.com/\u{a0}"] {
            assert!(check_url(bad).is_err(), "{bad:?}");
        }
        assert!(check_url(&format!("https://a.com/{}", "a".repeat(MAX_URL))).is_err());
    }
}
