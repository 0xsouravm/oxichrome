mod devtools;
mod page;
mod panel;
mod popup;

use oxichrome::context_menus::Click;
use oxichrome::prelude::*;
use oxichrome::runtime::MessageSender;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;

#[extension(
    name = "Page Lens",
    description = "See how long a page takes to read, jump through its outline, follow your progress and copy Markdown links.",
    version = "0.3.0",
    permissions = [ActiveTab, Scripting, Downloads, ClipboardWrite, SidePanel],
    optional_host_permissions = ["https://*/*", "http://*/*"],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = "Page Lens", shortcut = "Alt+Shift+L" },
    gecko_id = "page-lens@oxichrome.dev"
)]
pub struct PageLens;

pub const WORDS_PER_MINUTE: usize = 230;

pub fn minutes(words: usize) -> usize {
    words.div_ceil(WORDS_PER_MINUTE).max(1)
}

pub fn markdown_link(title: &str, url: &str) -> String {
    let title = if title.trim().is_empty() { url } else { title.trim() };
    format!("[{}]({})", title.replace('[', "\\[").replace(']', "\\]"), url.replace(' ', "%20").replace(')', "%29"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reading {
    pub words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Estimate {
    pub minutes: usize,
}

#[on(runtime::on_message)]
async fn reading(reading: Reading, sender: MessageSender) -> Estimate {
    let minutes = minutes(reading.words);
    if let Some(tab_id) = sender.tab.and_then(|tab| tab.id) {
        let _ = action::set_badge_background_color("#18a06e", Some(tab_id)).await;
        let _ = action::set_badge_text(&format!("{minutes}m"), Some(tab_id)).await;
    }
    Estimate { minutes }
}

#[offscreen(reasons = [Clipboard], justification = "Copy Markdown links to the clipboard")]
async fn copy(text: String) -> bool {
    let document: web_sys::HtmlDocument = web_sys::window().unwrap().document().unwrap().unchecked_into();
    let area: web_sys::HtmlTextAreaElement = document.create_element("textarea").unwrap().unchecked_into();
    area.set_value(&text);
    document.body().unwrap().append_child(&area).unwrap();
    area.select();
    let copied = document.exec_command("copy").unwrap_or(false);
    area.remove();
    copied
}

#[menu(title = "Copy link as Markdown", contexts = [page, link])]
async fn copy_link(click: Click) {
    let markdown = match (click.link_url, click.page_url) {
        (Some(link), _) => markdown_link(&click.selection_text.unwrap_or_default(), &link),
        (None, Some(page)) => {
            let title = match click.tab_id {
                Some(id) => tabs::get::<tabs::Tab>(id).await.ok().and_then(|tab| tab.title).unwrap_or_default(),
                None => String::new(),
            };
            markdown_link(&title, &page)
        }
        (None, None) => return,
    };
    if let Err(err) = copy::OFFSCREEN.run(markdown).await {
        oxichrome::log!("couldn't copy: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_round_up() {
        assert_eq!(minutes(0), 1);
        assert_eq!(minutes(230), 1);
        assert_eq!(minutes(400), 2);
    }

    #[test]
    fn markdown_links_escape() {
        assert_eq!(markdown_link("Rust [lang]", "https://x.dev/a b)"), "[Rust \\[lang\\]](https://x.dev/a%20b%29)");
        assert_eq!(markdown_link(" ", "https://x.dev"), "[https://x.dev](https://x.dev)");
    }
}
