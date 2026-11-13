use oxichrome::content_script;
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlElement, ScrollBehavior, ScrollIntoViewOptions, ScrollLogicalPosition};

pub const WORDS_PER_MINUTE: usize = 230;
pub const READABLE_CSS: &str = "p, li, dd, blockquote { font-size: 19px !important; line-height: 1.7 !important; max-width: 70ch !important; } body { letter-spacing: 0.005em !important; }";
pub const LINKS_CSS: &str = "a[href] { outline: 2px solid #18a06e !important; outline-offset: 2px !important; border-radius: 3px !important; }";

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PageInfo {
    pub title: String,
    pub url: String,
    pub description: String,
    pub language: String,
    pub words: usize,
    pub links: u32,
    pub images: u32,
    pub headings: Vec<Heading>,
}

impl PageInfo {
    pub fn minutes(&self) -> usize {
        self.words.div_ceil(WORDS_PER_MINUTE).max(1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reading {
    pub minutes: usize,
    pub words: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadingReply {
    pub show_pill: bool,
}

fn document() -> Document {
    web_sys::window().and_then(|window| window.document()).expect("a page has a document")
}

fn root(document: &Document) -> Element {
    document
        .query_selector("article, main, [role=main]")
        .ok()
        .flatten()
        .or_else(|| document.body().map(Into::into))
        .expect("a page has a body")
}

fn text_of(element: &Element) -> String {
    element.dyn_ref::<HtmlElement>().map(HtmlElement::inner_text).unwrap_or_else(|| element.text_content().unwrap_or_default())
}

fn headings(document: &Document) -> Vec<Element> {
    let mut found = Vec::new();
    if let Ok(list) = root(document).query_selector_all("h1, h2, h3") {
        for index in 0..list.length() {
            if let Some(element) = list.item(index).and_then(|node| node.dyn_into::<Element>().ok()) {
                if !text_of(&element).trim().is_empty() {
                    found.push(element);
                }
            }
        }
    }
    found
}

fn word_count(document: &Document) -> usize {
    text_of(&root(document)).split_whitespace().count()
}

#[content_script]
pub async fn inspect() -> PageInfo {
    let document = document();
    let description = document
        .query_selector("meta[name=description]")
        .ok()
        .flatten()
        .and_then(|meta| meta.get_attribute("content"))
        .unwrap_or_default();
    let language = document.document_element().map(|html| html.get_attribute("lang").unwrap_or_default()).unwrap_or_default();
    PageInfo {
        title: document.title(),
        url: document.url().unwrap_or_default(),
        description: description.trim().to_string(),
        language,
        words: word_count(&document),
        links: document.links().length(),
        images: document.images().length(),
        headings: headings(&document)
            .iter()
            .take(60)
            .map(|element| Heading {
                level: element.tag_name()[1..].parse().unwrap_or(2),
                text: text_of(element).split_whitespace().collect::<Vec<_>>().join(" "),
            })
            .collect(),
    }
}

#[content_script]
pub async fn selection() -> String {
    web_sys::window()
        .and_then(|window| window.get_selection().ok().flatten())
        .and_then(|selection| selection.to_string().as_string())
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

#[content_script]
pub async fn scroll_to(index: usize) -> bool {
    let Some(element) = headings(&document()).into_iter().nth(index) else { return false };
    let options = ScrollIntoViewOptions::new();
    options.set_behavior(ScrollBehavior::Smooth);
    options.set_block(ScrollLogicalPosition::Start);
    element.scroll_into_view_with_scroll_into_view_options(&options);
    let Some(element) = element.dyn_ref::<HtmlElement>().cloned() else { return true };
    let style = element.style();
    let _ = style.set_property("outline", "3px solid #18a06e");
    let _ = style.set_property("outline-offset", "4px");
    let _ = style.set_property("border-radius", "4px");
    let clear = Closure::once_into_js(move || {
        let style = element.style();
        for property in ["outline", "outline-offset", "border-radius"] {
            let _ = style.remove_property(property);
        }
    });
    if let Some(window) = web_sys::window() {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(clear.unchecked_ref(), 1600);
    }
    true
}

#[content_script(matches = ["https://*.wikipedia.org/wiki/*", "https://developer.mozilla.org/*"], run_at = DocumentIdle, css = ["pill.css"])]
async fn reading_pill() {
    let document = document();
    let words = word_count(&document);
    if words < 150 {
        return;
    }
    let reading = Reading { minutes: words.div_ceil(WORDS_PER_MINUTE), words };
    let reply = oxichrome::runtime::send_message(&reading).await.ok();
    let show = reply.and_then(|reply| serde_wasm_bindgen::from_value::<ReadingReply>(reply).ok()).is_some_and(|reply| reply.show_pill);
    if !show || document.get_element_by_id("page-lens-pill").is_some() {
        return;
    }
    let Ok(pill) = document.create_element("div") else { return };
    pill.set_id("page-lens-pill");
    pill.set_text_content(Some(&format!("{} min read · {} words", reading.minutes, words)));
    if let Some(body) = document.body() {
        let _ = body.append_child(&pill);
    }
}
