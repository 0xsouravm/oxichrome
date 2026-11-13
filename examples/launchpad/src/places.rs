use oxichrome::bookmarks::{self, Kind, New, Node, Special};
use oxichrome::history::{self, Item};
use oxichrome::search::{self, Where};
use oxichrome::top_sites::{self, Site};
use oxichrome::{storage, tabs, Result};
use serde_json::json;

pub const FOLDER: &str = "Launchpad";
const VISITS: &str = "visits";

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub title: String,
    pub url: String,
}

impl From<Site> for Place {
    fn from(site: Site) -> Self {
        Place { title: site.title, url: site.url }
    }
}

impl From<Item> for Place {
    fn from(item: Item) -> Self {
        Place { title: item.title, url: item.url }
    }
}

impl Place {
    pub fn from_node(node: Node) -> Option<Self> {
        match node.kind {
            Kind::Bookmark { url } => Some(Place { title: node.title, url }),
            Kind::Folder | Kind::Separator => None,
        }
    }

    pub fn host(&self) -> String {
        host(&self.url)
    }

    pub fn name(&self) -> String {
        if self.title.trim().is_empty() {
            self.host()
        } else {
            self.title.trim().to_string()
        }
    }
}

pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

pub fn hue(text: &str) -> u32 {
    text.bytes().fold(7u32, |hash, byte| hash.wrapping_mul(31).wrapping_add(u32::from(byte))) % 360
}

pub fn looks_like_url(text: &str) -> Option<String> {
    let text = text.trim();
    if text.contains(char::is_whitespace) || text.is_empty() {
        return None;
    }
    if text.starts_with("https://") || text.starts_with("http://") {
        return Some(text.to_string());
    }
    let host = text.split('/').next().unwrap_or_default();
    let tld = host.rsplit('.').next().unwrap_or_default();
    (host.contains('.') && tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())).then(|| format!("https://{text}"))
}

pub async fn go(text: &str, place: Where) -> Result<()> {
    match looks_like_url(text) {
        Some(url) => open(&url, matches!(place, Where::CurrentTab)).await,
        None => search::query(text, place).await,
    }
}

pub async fn open(url: &str, here: bool) -> Result<()> {
    if here {
        let found: Vec<tabs::Tab> = tabs::query(&json!({ "active": true, "currentWindow": true })).await?;
        if let Some(id) = found.into_iter().next().and_then(|tab| tab.id) {
            let _: serde_json::Value = tabs::update(id, &json!({ "url": url })).await?;
            return Ok(());
        }
    }
    let _: serde_json::Value = tabs::create(&json!({ "url": url })).await?;
    Ok(())
}

fn is_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

pub async fn top(count: usize) -> Vec<Place> {
    top_sites::get().await.unwrap_or_default().into_iter().filter(|site| is_web(&site.url)).take(count).map(Place::from).collect()
}

pub async fn recent_history(count: u32) -> Vec<(Place, f64)> {
    history::search(&history::Search::new().limit(count * 3))
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|item| is_web(&item.url))
        .take(count as usize)
        .map(|item| {
            let when = item.last_visit;
            (Place::from(item), when)
        })
        .collect()
}

pub async fn folder() -> Result<Node> {
    let other = bookmarks::folder(Special::Other).await?;
    let existing = bookmarks::children(&other.id).await?;
    if let Some(found) = existing.into_iter().find(|node| node.title == FOLDER && matches!(node.kind, Kind::Folder)) {
        return Ok(found);
    }
    bookmarks::create(&New::folder(&other.id, FOLDER)).await
}

pub async fn saved() -> Vec<(String, Place)> {
    let Ok(folder) = folder().await else { return Vec::new() };
    bookmarks::children(&folder.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|node| {
            let id = node.id.clone();
            Place::from_node(node).map(|place| (id, place))
        })
        .collect()
}

pub async fn save(title: &str, url: &str) -> Result<()> {
    let folder = folder().await?;
    bookmarks::create(&New::bookmark(&folder.id, title, url).index(0)).await?;
    Ok(())
}

pub async fn unsave(id: &str) -> Result<()> {
    bookmarks::remove(id).await
}

pub async fn matching(text: &str, count: usize) -> Vec<Place> {
    let mut found: Vec<Place> = Vec::new();
    if text.trim().is_empty() {
        return found;
    }
    for node in bookmarks::search(&bookmarks::Search::new().text(text)).await.unwrap_or_default() {
        if let Some(place) = Place::from_node(node) {
            found.push(place);
        }
    }
    for item in history::search(&history::Search::new().text(text).limit(20)).await.unwrap_or_default() {
        found.push(item.into());
    }
    let mut seen = std::collections::HashSet::new();
    found.retain(|place| is_web(&place.url) && seen.insert(place.url.clone()));
    found.truncate(count);
    found
}

pub async fn count_visit() {
    let today = start_of_today();
    let (day, count): (f64, u32) = storage::session::get(VISITS).await.ok().flatten().unwrap_or((today, 0));
    let count = if day == today { count + 1 } else { 1 };
    let _ = storage::session::set(VISITS, &(today, count)).await;
}

pub async fn visits_today() -> u32 {
    let (day, count): (f64, u32) = storage::session::get(VISITS).await.ok().flatten().unwrap_or((0.0, 0));
    if day == start_of_today() {
        count
    } else {
        0
    }
}

pub fn start_of_today() -> f64 {
    let day = js_sys::Date::new_0();
    day.set_hours(0);
    day.set_minutes(0);
    day.set_seconds(0);
    day.set_milliseconds(0);
    day.get_time()
}

pub fn ago(time: f64) -> String {
    let minutes = ((js_sys::Date::now() - time) / 60_000.0).floor() as i64;
    match minutes {
        ..=0 => "now".to_string(),
        1..=59 => format!("{minutes}m"),
        60..=1439 => format!("{}h", minutes / 60),
        _ => format!("{}d", minutes / 1440),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_addresses_from_searches() {
        assert_eq!(looks_like_url("docs.rs/serde").as_deref(), Some("https://docs.rs/serde"));
        assert_eq!(looks_like_url("http://localhost:3000").as_deref(), Some("http://localhost:3000"));
        assert_eq!(looks_like_url("rust web extensions"), None);
        assert_eq!(looks_like_url("v0.3.0"), None);
        assert_eq!(looks_like_url("serde"), None);
    }
}
