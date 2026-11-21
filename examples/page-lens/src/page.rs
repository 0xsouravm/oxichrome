use std::cell::Cell;

use oxichrome::content_script;
use oxichrome::runtime::{self, Port};
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlElement, ScrollIntoViewOptions};

use crate::{minutes, Estimate, Reading};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PageInfo {
    pub title: String,
    pub url: String,
    pub words: usize,
    pub links: u32,
    pub headings: Vec<Heading>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Progress {
    pub percent: u8,
    pub minutes_left: usize,
}

pub fn percent(scrolled: f64, viewport: f64, height: f64) -> u8 {
    let room = height - viewport;
    if room <= 0.0 {
        return 100;
    }
    (scrolled / room * 100.0).clamp(0.0, 100.0).round() as u8
}

fn document() -> Document {
    web_sys::window().and_then(|window| window.document()).expect("a page has a document")
}

fn text_of(element: &Element) -> String {
    let text = element.dyn_ref::<HtmlElement>().map(HtmlElement::inner_text).unwrap_or_default();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn root(document: &Document) -> Element {
    document.query_selector("article, main, [role=main]").ok().flatten().or_else(|| document.body().map(Into::into)).expect("a page has a body")
}

fn words(document: &Document) -> usize {
    text_of(&root(document)).split(' ').filter(|word| !word.is_empty()).count()
}

fn headings(document: &Document) -> Vec<Element> {
    let Ok(list) = root(document).query_selector_all("h1, h2, h3") else { return Vec::new() };
    (0..list.length()).filter_map(|index| list.item(index)?.dyn_into::<Element>().ok()).filter(|element| !text_of(element).is_empty()).collect()
}

#[content_script]
pub async fn inspect() -> PageInfo {
    let document = document();
    PageInfo {
        title: document.title(),
        url: document.url().unwrap_or_default(),
        words: words(&document),
        links: document.links().length(),
        headings: headings(&document)
            .iter()
            .map(|element| Heading { level: element.tag_name()[1..].parse().unwrap_or(2), text: text_of(element) })
            .collect(),
    }
}

#[content_script]
pub async fn scroll_to(index: usize) -> bool {
    let Some(element) = headings(&document()).into_iter().nth(index) else { return false };
    let options = ScrollIntoViewOptions::new();
    options.set_behavior(web_sys::ScrollBehavior::Smooth);
    element.scroll_into_view_with_scroll_into_view_options(&options);
    true
}

thread_local! {
    static FOLLOWED: Cell<bool> = const { Cell::new(false) };
}

fn progress(document: &Document, words: usize) -> Progress {
    let window = web_sys::window().expect("a page has a window");
    let viewport = window.inner_height().ok().and_then(|height| height.as_f64()).unwrap_or(0.0);
    let height = document.document_element().map_or(0, |html| html.scroll_height()) as f64;
    let percent = percent(window.scroll_y().unwrap_or(0.0), viewport, height);
    let left = words * (100 - percent as usize) / 100;
    Progress { percent, minutes_left: if percent == 100 { 0 } else { minutes(left) } }
}

async fn stream(port: Port) {
    let document = document();
    let window = web_sys::window().expect("a page has a window");
    let total = words(&document);
    let _ = port.post(&progress(&document, total));
    let sender = port.clone();
    let on_scroll = Closure::<dyn Fn()>::new(move || {
        let _ = sender.post(&progress(&document, total));
    });
    let closer = port.clone();
    let on_hide = Closure::<dyn Fn()>::new(move || closer.disconnect());
    let _ = window.add_event_listener_with_callback("scroll", on_scroll.as_ref().unchecked_ref());
    let _ = window.add_event_listener_with_callback("pagehide", on_hide.as_ref().unchecked_ref());
    let _ = port.closed().await;
    let _ = window.remove_event_listener_with_callback("scroll", on_scroll.as_ref().unchecked_ref());
    let _ = window.remove_event_listener_with_callback("pagehide", on_hide.as_ref().unchecked_ref());
}

#[content_script]
pub async fn follow() {
    if FOLLOWED.replace(true) {
        return;
    }
    let _ = runtime::on_connect(|port: Port| async move {
        if port.name() == "progress" {
            stream(port).await;
        }
    });
}

#[content_script(matches = ["https://*.wikipedia.org/wiki/*", "https://developer.mozilla.org/*"], run_at = DocumentIdle, css = ["pill.css"])]
async fn reading_pill() {
    let document = document();
    let words = words(&document);
    if words < 150 || document.get_element_by_id("page-lens-pill").is_some() {
        return;
    }
    let Ok(Estimate { minutes }) = runtime::request(&Reading { words }).await else { return };
    let Ok(pill) = document.create_element("div") else { return };
    pill.set_id("page-lens-pill");
    pill.set_text_content(Some(&format!("{minutes} min read · {words} words")));
    if let Some(body) = document.body() {
        let _ = body.append_child(&pill);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_of_the_page_scrolled() {
        assert_eq!(percent(0.0, 800.0, 2800.0), 0);
        assert_eq!(percent(1000.0, 800.0, 2800.0), 50);
        assert_eq!(percent(5000.0, 800.0, 2800.0), 100);
        assert_eq!(percent(0.0, 800.0, 600.0), 100);
    }
}
