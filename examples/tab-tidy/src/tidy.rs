use std::collections::HashSet;

use oxichrome::sessions::{self, Closed};
use oxichrome::tab_groups::{self, Color, Update};
use oxichrome::tabs::{self, Tab};
use oxichrome::windows::{self, NewWindow, WindowUpdate};
use oxichrome::{action, t, Result};
use serde_json::json;

const COLORS: [Color; 9] = [Color::Blue, Color::Red, Color::Yellow, Color::Green, Color::Pink, Color::Purple, Color::Cyan, Color::Orange, Color::Grey];

pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

fn url(tab: &Tab) -> &str {
    tab.url.as_deref().unwrap_or_default()
}

pub fn label(tab: &Tab) -> String {
    tab.title.clone().filter(|title| !title.trim().is_empty()).unwrap_or_else(|| url(tab).to_string())
}

pub fn matches(tab: &Tab, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty() || label(tab).to_lowercase().contains(&query) || url(tab).to_lowercase().contains(&query)
}

pub fn duplicates(tabs: &[Tab]) -> Vec<i32> {
    let mut order: Vec<&Tab> = tabs.iter().filter(|tab| tab.id.is_some() && !url(tab).is_empty()).collect();
    order.sort_by_key(|tab| (!tab.pinned, !tab.active, tab.window_id, tab.index));
    let mut seen = HashSet::new();
    order
        .into_iter()
        .filter(|tab| !seen.insert(url(tab).split('#').next().unwrap_or_default()) && !tab.pinned && !tab.active)
        .filter_map(|tab| tab.id)
        .collect()
}

pub fn site_groups(tabs: &[Tab]) -> Vec<(String, Vec<i32>)> {
    let mut sites: Vec<((i32, String), Vec<i32>)> = Vec::new();
    for tab in tabs.iter().filter(|tab| !tab.pinned && tab.group_id.is_none() && url(tab).starts_with("http")) {
        let Some(id) = tab.id else { continue };
        let key = (tab.window_id, host(url(tab)));
        match sites.iter_mut().find(|(site, _)| *site == key) {
            Some((_, ids)) => ids.push(id),
            None => sites.push((key, vec![id])),
        }
    }
    sites.into_iter().filter(|(_, ids)| ids.len() > 1).map(|((_, site), ids)| (site, ids)).collect()
}

pub fn color(site: &str) -> Color {
    COLORS[site.bytes().map(usize::from).sum::<usize>() % COLORS.len()]
}

pub async fn all() -> Vec<Tab> {
    tabs::query(&tabs::Query::new()).await.unwrap_or_default()
}

pub async fn switch_to(tab_id: i32, window_id: i32) -> Result<()> {
    let _: Tab = tabs::update(tab_id, &json!({ "active": true })).await?;
    windows::update(window_id, WindowUpdate::new().focused(true)).await?;
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

pub async fn group_by_site() -> Result<usize> {
    let groups = site_groups(&all().await);
    for (site, ids) in &groups {
        let group_id = tabs::group(ids).await?;
        tab_groups::update(group_id, &Update { title: Some(site.clone()), color: Some(color(site)), ..Default::default() }).await?;
    }
    Ok(groups.len())
}

pub async fn close_duplicates() -> usize {
    let mut closed = 0;
    for id in duplicates(&all().await) {
        closed += usize::from(tabs::remove(id).await.is_ok());
    }
    refresh_badge().await;
    closed
}

pub async fn recently_closed() -> Vec<Closed> {
    sessions::recently_closed(5).await.unwrap_or_default()
}

pub fn closed_label(closed: &Closed) -> String {
    match closed {
        Closed::Tab(tab) => tab.title.clone().or_else(|| tab.url.clone()).unwrap_or_else(|| t!(untitled)),
        Closed::Window(window) => t!(closed_window, window.tabs.len()),
    }
}

pub async fn refresh_badge() {
    let extra = duplicates(&all().await).len();
    let text = if extra == 0 { String::new() } else { extra.to_string() };
    let _ = action::set_badge_background_color("#c25a14", None).await;
    let _ = action::set_badge_text(&text, None).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(id: i32, url: &str, active: bool, pinned: bool) -> Tab {
        serde_json::from_value(json!({ "id": id, "index": id, "windowId": 1, "url": url, "active": active, "pinned": pinned })).unwrap()
    }

    #[test]
    fn keeps_the_active_and_pinned_copy() {
        let tabs = [tab(1, "https://a.dev/x", false, false), tab(2, "https://a.dev/x#part", true, false), tab(3, "https://a.dev/x", false, true), tab(4, "https://b.dev/", false, false)];
        assert_eq!(duplicates(&tabs), [1]);
    }

    #[test]
    fn groups_two_or_more_tabs_of_a_site() {
        let tabs = [tab(1, "https://www.a.dev/x", false, false), tab(2, "https://a.dev/y", false, false), tab(3, "https://a.dev/z", false, true), tab(4, "https://b.dev/", false, false)];
        assert_eq!(site_groups(&tabs), [("a.dev".to_string(), vec![1, 2])]);
    }

    #[test]
    fn hosts_drop_www_and_paths() {
        assert_eq!(host("https://www.rust-lang.org/learn?x=1"), "rust-lang.org");
        assert_eq!(color("a.dev"), color("a.dev"));
    }
}
