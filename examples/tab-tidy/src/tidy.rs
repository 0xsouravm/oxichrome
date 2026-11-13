use std::collections::HashSet;

use oxichrome::windows::{self, NewWindow, WindowUpdate};
use oxichrome::{action, storage, t, tabs, Result};
use serde::Deserialize;
use serde_json::{json, Value};

const CLOSED_TOTAL: &str = "closed_total";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
    #[serde(default = "no_tab")]
    pub id: i32,
    pub window_id: i32,
    pub index: i32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    pub fav_icon_url: Option<String>,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub audible: bool,
    #[serde(default)]
    pub muted_info: MutedInfo,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct MutedInfo {
    #[serde(default)]
    pub muted: bool,
}

fn no_tab() -> i32 {
    -1
}

impl TabInfo {
    pub fn muted(&self) -> bool {
        self.muted_info.muted
    }

    pub fn host(&self) -> String {
        host(&self.url)
    }

    pub fn label(&self) -> String {
        if self.title.trim().is_empty() {
            self.url.clone()
        } else {
            self.title.clone()
        }
    }

    pub fn icon(&self) -> Option<String> {
        self.fav_icon_url
            .clone()
            .filter(|url| url.starts_with("https://") || url.starts_with("http://") || url.starts_with("data:image/"))
    }
}

pub struct Group {
    pub window_id: i32,
    pub current: bool,
    pub number: usize,
    pub tabs: Vec<TabInfo>,
}

pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

fn page(url: &str) -> &str {
    url.split('#').next().unwrap_or(url)
}

pub fn duplicates(tabs: &[TabInfo]) -> Vec<i32> {
    let mut order: Vec<&TabInfo> = tabs.iter().filter(|tab| !tab.url.is_empty()).collect();
    order.sort_by_key(|tab| (!tab.pinned, !tab.active, tab.window_id, tab.index));
    let mut seen = HashSet::new();
    let mut extra = Vec::new();
    for tab in order {
        let first = seen.insert(page(&tab.url));
        if !first && !tab.pinned && !tab.active {
            extra.push(tab.id);
        }
    }
    extra
}

pub fn matches(tab: &TabInfo, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty() || tab.title.to_lowercase().contains(&query) || tab.url.to_lowercase().contains(&query)
}

pub fn group(tabs: Vec<TabInfo>, current_window: Option<i32>) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for tab in tabs {
        match groups.iter_mut().find(|group| group.window_id == tab.window_id) {
            Some(group) => group.tabs.push(tab),
            None => groups.push(Group { window_id: tab.window_id, current: Some(tab.window_id) == current_window, number: 0, tabs: vec![tab] }),
        }
    }
    groups.sort_by_key(|group| (!group.current, group.window_id));
    for (number, group) in groups.iter_mut().enumerate() {
        group.number = number + 1;
        group.tabs.sort_by_key(|tab| tab.index);
    }
    groups
}

pub async fn all() -> Vec<TabInfo> {
    let tabs: Vec<TabInfo> = tabs::query(&json!({})).await.unwrap_or_default();
    tabs.into_iter().filter(|tab| tab.id >= 0).collect()
}

pub async fn switch_to(tab_id: i32, window_id: i32) -> Result<()> {
    let _: Value = tabs::update(tab_id, &json!({ "active": true })).await?;
    windows::update(window_id, WindowUpdate::new().focused(true)).await?;
    Ok(())
}

pub async fn focus_window(window_id: i32) -> Result<()> {
    windows::update(window_id, WindowUpdate::new().focused(true)).await?;
    Ok(())
}

pub async fn set_pinned(tab_id: i32, pinned: bool) -> Result<()> {
    let _: Value = tabs::update(tab_id, &json!({ "pinned": pinned })).await?;
    Ok(())
}

pub async fn toggle_pinned(tab_id: i32) -> Result<()> {
    let tab: TabInfo = tabs::get(tab_id).await?;
    set_pinned(tab_id, !tab.pinned).await
}

pub async fn set_muted(tab_id: i32, muted: bool) -> Result<()> {
    let _: Value = tabs::update(tab_id, &json!({ "muted": muted })).await?;
    Ok(())
}

pub async fn move_to_new_window(tab_id: i32) -> Result<()> {
    windows::create(NewWindow::new().tab(tab_id)).await?;
    Ok(())
}

pub async fn new_window() -> Result<()> {
    windows::create(NewWindow::new()).await?;
    Ok(())
}

pub async fn close(ids: &[i32]) -> usize {
    let mut closed = 0;
    for id in ids {
        if tabs::remove(*id).await.is_ok() {
            closed += 1;
        }
    }
    closed
}

pub async fn close_duplicates() -> usize {
    let closed = close(&duplicates(&all().await)).await;
    count_closed(closed).await;
    refresh_badge().await;
    closed
}

pub async fn close_site(url: &str, keep: Option<i32>) -> usize {
    let site = host(url);
    if site.is_empty() {
        return 0;
    }
    let ids: Vec<i32> = all().await.into_iter().filter(|tab| tab.host() == site && Some(tab.id) != keep && !tab.pinned).map(|tab| tab.id).collect();
    let closed = close(&ids).await;
    count_closed(closed).await;
    closed
}

pub async fn closed_total() -> u32 {
    storage::get::<u32>(CLOSED_TOTAL).await.ok().flatten().unwrap_or(0)
}

async fn count_closed(closed: usize) {
    if closed > 0 {
        let total = closed_total().await + closed as u32;
        let _ = storage::set(CLOSED_TOTAL, &total).await;
    }
}

pub async fn refresh_badge() {
    let extra = duplicates(&all().await).len();
    let text = if extra == 0 { String::new() } else { extra.to_string() };
    let _ = action::set_badge_background_color("#e0702f", None).await;
    let _ = action::set_badge_text(&text, None).await;
    let title = if extra == 0 { t!(action_title) } else { format!("{} · {}", t!(action_title), duplicates_label(extra)) };
    let _ = action::set_title(&title, None).await;
}

pub fn tabs_label(count: usize) -> String {
    if count == 1 {
        t!(tab_one)
    } else {
        t!(tab_many, count)
    }
}

pub fn windows_label(count: usize) -> String {
    if count == 1 {
        t!(window_one)
    } else {
        t!(window_many, count)
    }
}

pub fn duplicates_label(count: usize) -> String {
    if count == 1 {
        t!(dupes_one)
    } else {
        t!(dupes_many, count)
    }
}

pub fn closed_label(count: usize) -> String {
    if count == 1 {
        t!(toast_closed_one)
    } else {
        t!(toast_closed_many, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(id: i32, url: &str, active: bool, pinned: bool) -> TabInfo {
        TabInfo {
            id,
            window_id: 1,
            index: id,
            title: String::new(),
            url: url.into(),
            fav_icon_url: None,
            active,
            pinned,
            audible: false,
            muted_info: MutedInfo::default(),
        }
    }

    #[test]
    fn keeps_the_active_and_pinned_copy() {
        let tabs = [tab(1, "https://a.dev/x", false, false), tab(2, "https://a.dev/x#part", true, false), tab(3, "https://a.dev/x", false, true), tab(4, "https://b.dev/", false, false)];
        assert_eq!(duplicates(&tabs), [1]);
    }

    #[test]
    fn hosts_drop_www_and_paths() {
        assert_eq!(host("https://www.rust-lang.org/learn?x=1"), "rust-lang.org");
        assert_eq!(host("about:blank"), "about:blank");
    }
}
