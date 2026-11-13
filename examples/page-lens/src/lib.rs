mod notes;
mod page;
mod panel;
mod popup;
mod ui;

use oxichrome::commands::Press;
use oxichrome::context_menus::Click;
use oxichrome::prelude::*;
use wasm_bindgen::{JsCast, JsValue};

use crate::notes::Kind;
use crate::page::{Reading, ReadingReply};

#[extension(
    name = "Page Lens",
    description = "See how long a page takes to read, jump through its outline, keep quotes and notes per page, and copy Markdown links.",
    version = "0.3.0",
    permissions = [ActiveTab, Scripting, Storage, Downloads, ClipboardWrite, SidePanel],
    optional_host_permissions = ["https://*/*", "http://*/*"],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = "Page Lens", shortcut = "Alt+Shift+L" },
    gecko_id = "page-lens@oxichrome.dev"
)]
pub struct PageLens;

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

pub async fn copy_text(text: String) -> Result<()> {
    match copy::OFFSCREEN.run(text).await? {
        true => Ok(()),
        false => Err(OxichromeError::InvalidArgument("the browser didn't allow copying".into())),
    }
}

fn sender_tab(sender: &JsValue) -> Option<i32> {
    let tab = js_sys::Reflect::get(sender, &"tab".into()).ok()?;
    js_sys::Reflect::get(&tab, &"id".into()).ok()?.as_f64().map(|id| id as i32)
}

#[on(runtime::on_message)]
async fn message(message: JsValue, sender: JsValue, reply: JsValue) {
    let Ok(reading) = serde_wasm_bindgen::from_value::<Reading>(message) else { return };
    let settings = notes::settings().await;
    if let Some(tab_id) = sender_tab(&sender) {
        let _ = action::set_badge_background_color("#18a06e", Some(tab_id)).await;
        let _ = action::set_badge_text(&format!("{}m", reading.minutes), Some(tab_id)).await;
        let _ = action::set_title(&format!("Page Lens · {} min read", reading.minutes), Some(tab_id)).await;
    }
    let answer = serde_wasm_bindgen::to_value(&ReadingReply { show_pill: settings.show_pill }).unwrap_or(JsValue::NULL);
    if let Some(reply) = reply.dyn_ref::<js_sys::Function>() {
        let _ = reply.call1(&JsValue::NULL, &answer);
    }
}

async fn tab_title(tab_id: Option<i32>) -> String {
    match tab_id {
        Some(id) => tabs::get::<tabs::Tab>(id).await.ok().and_then(|tab| tab.title).unwrap_or_default(),
        None => String::new(),
    }
}

#[menu(title = "Save “%s” to Page Lens", contexts = [selection])]
async fn save_quote(click: Click) {
    let (Some(url), Some(text)) = (click.page_url, click.selection_text) else { return };
    let title = tab_title(click.tab_id).await;
    let _ = notes::add(&url, &title, Kind::Quote, &text).await;
}

#[menu(title = "Copy link as Markdown", contexts = [page, link])]
async fn copy_link(click: Click) {
    let markdown = match (click.link_url, click.page_url) {
        (Some(link), _) => notes::markdown_link(&click.selection_text.unwrap_or_default(), &link),
        (None, Some(page)) => notes::markdown_link(&tab_title(click.tab_id).await, &page),
        (None, None) => return,
    };
    if let Err(err) = copy_text(markdown).await {
        oxichrome::log!("couldn't copy: {err}");
    }
}

#[command(description = "Save the selected text as a quote", shortcut = "Alt+Shift+Q")]
async fn quote_selection(press: Press) {
    let Some(tab_id) = press.tab_id else { return };
    let Ok(text) = page::selection::SCRIPT.run(tab_id, ()).await else { return };
    let Ok(tab) = tabs::get::<tabs::Tab>(tab_id).await else { return };
    let (Some(url), title) = (tab.url, tab.title.unwrap_or_default()) else { return };
    if notes::add(&url, &title, Kind::Quote, &text).await.is_ok() && !text.trim().is_empty() {
        let _ = action::set_badge_background_color("#18a06e", Some(tab_id)).await;
        let _ = action::set_badge_text("✓", Some(tab_id)).await;
    }
}
