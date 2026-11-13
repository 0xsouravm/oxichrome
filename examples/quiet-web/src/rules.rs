use oxichrome::declarative_net_request::{self as dnr, Redirect, ResourceType, Rule, Transform};
use oxichrome::permissions::{self, PermissionSet};
use oxichrome::{storage, Result};
use serde::{Deserialize, Serialize};

pub const TRACKERS: &str = "trackers";
pub const ANNOYANCES: &str = "annoyances";
const STATE: &str = "state";
const STATS: &str = "stats";
const STRIP_RULE: u32 = 1;
const PAUSED_FROM: u32 = 1000;
const BLOCKED_FROM: u32 = 3000;
const TRACKING_PARAMS: [&str; 9] = ["utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content", "fbclid", "gclid", "msclkid", "mc_eid"];

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub blocked: Vec<String>,
    pub paused: Vec<String>,
    pub strip_params: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub day: f64,
    pub stopped: u32,
    pub loaded: u32,
}

pub struct List {
    pub id: &'static str,
    pub name: &'static str,
    pub about: &'static str,
}

pub const LISTS: [List; 2] = [
    List { id: TRACKERS, name: "Trackers", about: "Analytics, ad networks and session recorders that follow you between sites." },
    List { id: ANNOYANCES, name: "Annoyances", about: "Cookie banners and chat bubbles. Off by default, since a few sites need them to work." },
];

pub fn all_sites() -> PermissionSet {
    PermissionSet::new().origin("https://*/*").origin("http://*/*")
}

pub fn site_access(host: &str) -> PermissionSet {
    PermissionSet::new().origin(format!("https://{host}/*")).origin(format!("http://{host}/*"))
}

pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#', ':']).next().unwrap_or_default().to_lowercase();
    host.strip_prefix("www.").map(str::to_string).unwrap_or(host)
}

pub fn is_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

pub fn clean_site(input: &str) -> std::result::Result<String, String> {
    let site = host(input.trim());
    if site.is_empty() || !site.contains('.') {
        return Err("Type a site like example.com".into());
    }
    if !site.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return Err(format!("“{site}” isn't a site name"));
    }
    Ok(site)
}

pub async fn state() -> State {
    storage::get(STATE).await.ok().flatten().unwrap_or_default()
}

pub async fn stats() -> Stats {
    let stats: Stats = storage::get(STATS).await.ok().flatten().unwrap_or_default();
    if stats.day == today() {
        stats
    } else {
        Stats { day: today(), ..Stats::default() }
    }
}

fn today() -> f64 {
    let day = js_sys::Date::new_0();
    day.set_hours(0);
    day.set_minutes(0);
    day.set_seconds(0);
    day.set_milliseconds(0);
    day.get_time()
}

pub async fn count(stopped: bool) {
    let mut stats = stats().await;
    if stopped {
        stats.stopped += 1;
    } else {
        stats.loaded += 1;
    }
    let _ = storage::set(STATS, &stats).await;
}

pub async fn has_all_sites() -> bool {
    permissions::contains(all_sites()).await.unwrap_or(false)
}

fn rules_for(state: &State, can_redirect: bool) -> Vec<Rule> {
    let mut rules = Vec::new();
    if state.strip_params && can_redirect {
        let mut transform = Transform::new();
        for param in TRACKING_PARAMS {
            transform = transform.remove_param(param);
        }
        rules.push(Rule::redirect(STRIP_RULE, Redirect::transform(transform)).regex("[?&](utm_[a-z]+|fbclid|gclid|msclkid|mc_eid)=").resource_types([ResourceType::MainFrame]));
    }
    for (n, site) in state.paused.iter().enumerate() {
        rules.push(Rule::allow_all_requests(PAUSED_FROM + n as u32).request_domains([site.as_str()]).resource_types([ResourceType::MainFrame, ResourceType::SubFrame]).priority(100));
    }
    for (n, site) in state.blocked.iter().enumerate() {
        let id = BLOCKED_FROM + n as u32;
        let rule = if can_redirect { Rule::redirect(id, Redirect::extension_path("/blocked.html")) } else { Rule::block(id) };
        rules.push(rule.request_domains([site.as_str()]).resource_types([ResourceType::MainFrame, ResourceType::SubFrame]).priority(50));
    }
    rules
}

pub async fn apply(state: &State) -> Result<()> {
    let can_redirect = has_all_sites().await;
    let old: Vec<u32> = dnr::dynamic_rules().await?.iter().map(|rule| rule.id).collect();
    dnr::update_dynamic_rules(&old, &rules_for(state, can_redirect)).await
}

pub async fn change(edit: impl FnOnce(&mut State)) -> Result<State> {
    let mut next = state().await;
    edit(&mut next);
    next.blocked.sort();
    next.blocked.dedup();
    next.paused.sort();
    next.paused.dedup();
    apply(&next).await?;
    storage::set(STATE, &next).await?;
    Ok(next)
}

pub async fn enabled_lists() -> Vec<String> {
    dnr::enabled_rule_sets().await.unwrap_or_default()
}

pub async fn set_list(id: &str, on: bool) -> Result<()> {
    if on {
        dnr::update_rule_sets(&[id], &[]).await
    } else {
        dnr::update_rule_sets(&[], &[id]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_are_cleaned_and_checked() {
        assert_eq!(clean_site(" https://WWW.YouTube.com/watch?v=1 ").unwrap(), "youtube.com");
        assert_eq!(clean_site("news.ycombinator.com:443").unwrap(), "news.ycombinator.com");
        assert!(clean_site("localhost").is_err());
        assert!(clean_site("exa mple.com").is_err());
    }

    #[test]
    fn rules_follow_the_state() {
        let state = State { blocked: vec!["a.com".into()], paused: vec!["b.com".into(), "c.com".into()], strip_params: true };
        let without = rules_for(&state, false);
        assert_eq!(without.iter().map(|rule| rule.id).collect::<Vec<_>>(), [1000, 1001, 3000]);
        let with = rules_for(&state, true);
        assert_eq!(with.iter().map(|rule| rule.id).collect::<Vec<_>>(), [1, 1000, 1001, 3000]);
    }
}
