use std::cell::RefCell;

use oxichrome::declarative_net_request::{self as dnr, Redirect, ResourceType, Rule, Transform};
use oxichrome::permissions::{self, PermissionSet};
use oxichrome::{storage, Result};
use serde::{Deserialize, Serialize};

pub const LISTS: [(&str, &str); 2] = [("trackers", "Trackers"), ("annoyances", "Annoyances")];
const STATE: &str = "state";
const PAGES: &str = "pages_stopped";
const REQUESTS: &str = "requests_stopped";
const STRIP_RULE: u32 = 1;
const BLOCKED_FROM: u32 = 1000;
const TRACKING_PARAMS: [&str; 5] = ["utm_source", "utm_medium", "utm_campaign", "fbclid", "gclid"];

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub blocked: Vec<String>,
    pub strip_params: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stopped {
    pub host: String,
    pub kind: String,
}

pub fn all_sites() -> PermissionSet {
    PermissionSet::new().origin("https://*/*").origin("http://*/*")
}

pub async fn has_all_sites() -> bool {
    permissions::contains(all_sites()).await.unwrap_or(false)
}

pub fn is_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#', ':']).next().unwrap_or_default().to_lowercase();
    host.strip_prefix("www.").map(str::to_string).unwrap_or(host)
}

pub fn clean_site(input: &str) -> std::result::Result<String, String> {
    let site = host(input.trim());
    let valid = site.contains('.') && site.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if valid { Ok(site) } else { Err("Type a site like example.com".into()) }
}

pub async fn state() -> State {
    storage::get(STATE).await.ok().flatten().unwrap_or_default()
}

fn rules_for(state: &State, can_redirect: bool) -> Vec<Rule> {
    let mut rules: Vec<Rule> = state
        .blocked
        .iter()
        .enumerate()
        .map(|(n, site)| {
            let id = BLOCKED_FROM + n as u32;
            let rule = if can_redirect { Rule::redirect(id, Redirect::extension_path("/blocked.html")) } else { Rule::block(id) };
            rule.request_domains([site.as_str()]).resource_types([ResourceType::MainFrame, ResourceType::SubFrame])
        })
        .collect();
    if state.strip_params && can_redirect {
        let transform = TRACKING_PARAMS.iter().fold(Transform::new(), |transform, param| transform.remove_param(*param));
        rules.push(Rule::redirect(STRIP_RULE, Redirect::transform(transform)).regex("[?&](utm_[a-z]+|fbclid|gclid)=").resource_types([ResourceType::MainFrame]));
    }
    rules
}

pub async fn apply(state: &State) -> Result<()> {
    let old: Vec<u32> = dnr::dynamic_rules().await?.iter().map(|rule| rule.id).collect();
    dnr::update_dynamic_rules(&old, &rules_for(state, has_all_sites().await)).await
}

pub async fn change(edit: impl FnOnce(&mut State)) -> Result<State> {
    let mut next = state().await;
    edit(&mut next);
    next.blocked.sort();
    next.blocked.dedup();
    apply(&next).await?;
    storage::set(STATE, &next).await?;
    Ok(next)
}

pub async fn enabled_lists() -> Vec<String> {
    dnr::enabled_rule_sets().await.unwrap_or_default()
}

pub async fn set_list(id: &str, on: bool) -> Result<()> {
    if on { dnr::update_rule_sets(&[id], &[]).await } else { dnr::update_rule_sets(&[], &[id]).await }
}

pub async fn pages_stopped() -> u32 {
    storage::get(PAGES).await.ok().flatten().unwrap_or(0)
}

pub async fn count_page() {
    let _ = storage::set(PAGES, &(pages_stopped().await + 1)).await;
}

pub async fn requests_stopped() -> Vec<Stopped> {
    storage::get(REQUESTS).await.ok().flatten().unwrap_or_default()
}

thread_local! {
    static WAITING: RefCell<(Vec<Stopped>, bool)> = const { RefCell::new((Vec::new(), false)) };
}

pub async fn log_request(entry: Stopped) {
    let first = WAITING.with_borrow_mut(|(queue, busy)| {
        queue.push(entry);
        !std::mem::replace(busy, true)
    });
    if !first {
        return;
    }
    loop {
        let batch = WAITING.with_borrow_mut(|(queue, busy)| {
            *busy = !queue.is_empty();
            std::mem::take(queue)
        });
        if batch.is_empty() {
            return;
        }
        let mut log = requests_stopped().await;
        log.splice(0..0, batch.into_iter().rev());
        log.truncate(20);
        let _ = storage::set(REQUESTS, &log).await;
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
        let state = State { blocked: vec!["a.com".into(), "b.com".into()], strip_params: true };
        assert_eq!(rules_for(&state, false).iter().map(|rule| rule.id).collect::<Vec<_>>(), [1000, 1001]);
        assert_eq!(rules_for(&state, true).iter().map(|rule| rule.id).collect::<Vec<_>>(), [1000, 1001, 1]);
    }
}
