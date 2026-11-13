mod options;
mod places;
mod popup;
mod ui;

use oxichrome::history::Item;
use oxichrome::omnibox::{self, Description, Disposition, Suggestion};
use oxichrome::prelude::*;
use oxichrome::search::Where;

#[extension(
    name = "Launchpad",
    description = "Search, your most visited sites, recent history and saved pages in one popup, plus an address bar keyword: type lp and a space.",
    version = "0.3.0",
    permissions = [Bookmarks, History, TopSites, Search, Storage, Tabs],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = "Launchpad", shortcut = "Alt+Shift+K" },
    gecko_id = "launchpad@oxichrome.dev"
)]
pub struct Launchpad;

#[omnibox(keyword = "lp", default = "Search the web for {input}")]
async fn suggest(input: String) -> Vec<Suggestion> {
    let mut list = Vec::new();
    if let Some(url) = places::looks_like_url(&input) {
        list.push(Suggestion::new(url.clone(), Description::new().text("Open ").url(url)));
    }
    for place in places::matching(&input, 6).await {
        list.push(Suggestion::new(place.url.clone(), Description::new().matched(place.name()).dim(" · ").url(place.host())));
    }
    list
}

#[on(omnibox::on_input_entered)]
async fn entered(text: String, disposition: Disposition) {
    if text.starts_with("https://") || text.starts_with("http://") {
        let _ = omnibox::open(&text, disposition).await;
        return;
    }
    let place = match disposition {
        Disposition::NewForegroundTab | Disposition::NewBackgroundTab => Where::NewTab,
        _ => Where::CurrentTab,
    };
    let _ = places::go(&text, place).await;
}

#[on(history::on_visited)]
async fn visited(_item: Item) {
    places::count_visit().await;
}
