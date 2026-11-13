mod options;
mod popup;
mod rules;
mod ui;

use oxichrome::permissions::PermissionSet;
use oxichrome::prelude::*;
use oxichrome::web_navigation::{Failure, NavError, Navigation};

#[extension(
    name = "Quiet Web",
    description = "Block trackers and annoyances, keep a list of sites you don't want to open, strip tracking parameters, and clear a site's cookies.",
    version = "0.3.0",
    permissions = [DeclarativeNetRequest, Storage, Cookies, WebNavigation, Tabs],
    optional_host_permissions = ["https://*/*", "http://*/*"],
    rule_sets = { trackers = "rules/trackers.json", annoyances = "rules/annoyances.json" },
    disabled_rule_sets = [annoyances],
    web_accessible = ["blocked.html"],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = "Quiet Web" },
    gecko_id = "quiet-web@oxichrome.dev"
)]
pub struct QuietWeb;

#[background]
async fn start() {
    let _ = rules::apply(&rules::state().await).await;
}

#[on(permissions::on_added)]
async fn granted(_set: PermissionSet) {
    let _ = rules::apply(&rules::state().await).await;
}

#[on(permissions::on_removed)]
async fn revoked(_set: PermissionSet) {
    let _ = rules::apply(&rules::state().await).await;
}

#[on(web_navigation::on_completed)]
async fn loaded(nav: Navigation) {
    if nav.is_top_level() && rules::is_web(&nav.url) {
        rules::count(false).await;
    }
}

#[on(web_navigation::on_error_occurred)]
async fn failed(failure: Failure) {
    if failure.is_top_level() && failure.error == NavError::Blocked {
        rules::count(true).await;
    }
}
