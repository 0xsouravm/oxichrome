mod pages;

use oxichrome::bookmarks::{self, Kind, New, Node, Special};
use oxichrome::history::{self, Item};
use oxichrome::omnibox::{self, Description, Disposition, Suggestion};
use oxichrome::prelude::*;
use oxichrome::search::{self, Where};
use oxichrome::{storage, top_sites, Result};

#[extension(
    name = "Launchpad",
    description = "Search, your most visited sites, recent history and saved pages in one popup, plus an address bar keyword: type lp and a space.",
    version = "0.3.0",
    permissions = [Bookmarks, History, TopSites, Search, Storage, Tabs],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = "Launchpad" },
    gecko_id = "launchpad@oxichrome.dev"
)]
pub struct Launchpad;

pub const FOLDER: &str = "Launchpad";
const VISITS: &str = "visits";

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub title: String,
    pub url: String,
}

impl Place {
    pub fn name(&self) -> String {
        match self.title.trim() {
            "" => host(&self.url),
            title => title.to_string(),
        }
    }
}

pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

pub fn is_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

pub fn looks_like_url(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.contains(char::is_whitespace) {
        return None;
    }
    if is_web(text) {
        return Some(text.to_string());
    }
    let host = text.split('/').next().unwrap_or_default();
    let tld = host.rsplit('.').next().unwrap_or_default();
    (host.contains('.') && tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())).then(|| format!("https://{text}"))
}

pub async fn go(text: &str, disposition: Disposition) -> Result<()> {
    match looks_like_url(text) {
        Some(url) => omnibox::open(&url, disposition).await,
        None => {
            let at = match disposition {
                Disposition::NewForegroundTab | Disposition::NewBackgroundTab => Where::NewTab,
                _ => Where::CurrentTab,
            };
            search::query(text, at).await
        }
    }
}

pub async fn top(count: usize) -> Vec<Place> {
    let sites = top_sites::get().await.unwrap_or_default();
    sites.into_iter().filter(|site| is_web(&site.url)).take(count).map(|site| Place { title: site.title, url: site.url }).collect()
}

pub async fn recent(count: usize) -> Vec<Place> {
    let items = history::search(&history::Search::new().limit(30)).await.unwrap_or_default();
    items.into_iter().filter(|item| is_web(&item.url)).take(count).map(|item| Place { title: item.title, url: item.url }).collect()
}

async fn folder() -> Result<Node> {
    let other = bookmarks::folder(Special::Other).await?;
    let found = bookmarks::children(&other.id).await?.into_iter().find(|node| node.title == FOLDER && node.is_folder());
    match found {
        Some(folder) => Ok(folder),
        None => bookmarks::create(&New::folder(&other.id, FOLDER)).await,
    }
}

fn bookmark(node: Node) -> Option<(String, Place)> {
    match node.kind {
        Kind::Bookmark { url } => Some((node.id, Place { title: node.title, url })),
        Kind::Folder | Kind::Separator => None,
    }
}

pub async fn saved() -> Vec<(String, Place)> {
    let Ok(folder) = folder().await else { return Vec::new() };
    bookmarks::children(&folder.id).await.unwrap_or_default().into_iter().filter_map(bookmark).collect()
}

pub async fn save(place: &Place) -> Result<()> {
    let folder = folder().await?;
    bookmarks::create(&New::bookmark(&folder.id, &place.title, &place.url)).await?;
    Ok(())
}

pub async fn visits() -> u32 {
    storage::session::get(VISITS).await.ok().flatten().unwrap_or(0)
}

#[omnibox(keyword = "lp", default = "Search the web for {input}")]
async fn suggest(input: String) -> Vec<Suggestion> {
    let mut list = Vec::new();
    if let Some(url) = looks_like_url(&input) {
        list.push(Suggestion::new(url.clone(), Description::new().text("Open ").url(url)));
    }
    if input.trim().is_empty() {
        return list;
    }
    let marks = bookmarks::search(&bookmarks::Search::new().text(&input)).await.unwrap_or_default();
    let pages = history::search(&history::Search::new().text(&input).limit(20)).await.unwrap_or_default();
    let found = marks.into_iter().filter_map(bookmark).map(|(_, place)| place);
    let found = found.chain(pages.into_iter().map(|item| Place { title: item.title, url: item.url }));
    let mut seen = std::collections::HashSet::new();
    for place in found.filter(|place| is_web(&place.url) && seen.insert(place.url.clone())).take(6) {
        list.push(Suggestion::new(place.url.clone(), Description::new().matched(place.name()).dim(" · ").url(host(&place.url))));
    }
    list
}

#[on(omnibox::on_input_entered)]
async fn entered(text: String, disposition: Disposition) {
    let _ = go(&text, disposition).await;
}

#[on(history::on_visited)]
async fn visited(item: Item) {
    if is_web(&item.url) {
        let _ = storage::session::set(VISITS, &(visits().await + 1)).await;
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

    #[test]
    fn names_fall_back_to_the_host() {
        assert_eq!(host("https://www.example.com/a?b"), "example.com");
        assert_eq!(Place { title: "  ".into(), url: "http://127.0.0.1:8080/x".into() }.name(), "127.0.0.1:8080");
        assert_eq!(Place { title: " Docs ".into(), url: "https://docs.rs".into() }.name(), "Docs");
    }
}
