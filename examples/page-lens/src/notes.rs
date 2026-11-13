use std::collections::BTreeMap;

use oxichrome::downloads::{self, Conflict, Save};
use oxichrome::{storage, Result};
use serde::{Deserialize, Serialize};

const NOTES: &str = "notes";
const SETTINGS: &str = "settings";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Quote,
    Note,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub kind: Kind,
    pub text: String,
    pub added_at: f64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PageNotes {
    pub title: String,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub show_pill: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { show_pill: true }
    }
}

pub fn page_key(url: &str) -> String {
    url.split('#').next().unwrap_or(url).to_string()
}

pub async fn all() -> BTreeMap<String, PageNotes> {
    storage::get(NOTES).await.ok().flatten().unwrap_or_default()
}

pub async fn for_page(url: &str) -> PageNotes {
    all().await.remove(&page_key(url)).unwrap_or_default()
}

pub async fn add(url: &str, title: &str, kind: Kind, text: &str) -> Result<()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    let mut all = all().await;
    let page = all.entry(page_key(url)).or_default();
    if !title.trim().is_empty() {
        page.title = title.trim().to_string();
    }
    let now = js_sys::Date::now();
    let id = page.notes.iter().map(|note| note.id + 1).max().unwrap_or(1).max(now as u64);
    page.notes.push(Note { id, kind, text: text.to_string(), added_at: now });
    storage::set(NOTES, &all).await
}

pub async fn remove(url: &str, id: u64) -> Result<()> {
    let mut all = all().await;
    let key = page_key(url);
    if let Some(page) = all.get_mut(&key) {
        page.notes.retain(|note| note.id != id);
        if page.notes.is_empty() {
            all.remove(&key);
        }
    }
    storage::set(NOTES, &all).await
}

pub fn markdown(all: &BTreeMap<String, PageNotes>) -> String {
    let mut out = String::from("# Page Lens notes\n");
    for (url, page) in all {
        let title = if page.title.is_empty() { url.as_str() } else { page.title.as_str() };
        out.push_str(&format!("\n## {}\n\n", markdown_link(title, url)));
        for note in &page.notes {
            match note.kind {
                Kind::Quote => out.push_str(&format!("> {}\n\n", note.text.replace('\n', "\n> "))),
                Kind::Note => out.push_str(&format!("- {}\n\n", note.text.replace('\n', "\n  "))),
            }
        }
    }
    out
}

pub fn markdown_link(title: &str, url: &str) -> String {
    let title = if title.trim().is_empty() { url } else { title.trim() };
    format!("[{}]({})", title.replace('[', "\\[").replace(']', "\\]"), url.replace(' ', "%20").replace(')', "%29"))
}

pub async fn export() -> Result<usize> {
    let all = all().await;
    let count = all.values().map(|page| page.notes.len()).sum();
    downloads::save(Save::text("page-lens-notes.md", markdown(&all)).on_conflict(Conflict::Uniquify)).await?;
    Ok(count)
}

pub async fn settings() -> Settings {
    storage::sync::get(SETTINGS).await.ok().flatten().unwrap_or_default()
}

pub async fn save_settings(settings: &Settings) -> Result<()> {
    storage::sync::set(SETTINGS, settings).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_keeps_quotes_and_escapes_links() {
        let mut all = BTreeMap::new();
        all.insert(
            "https://a.dev/x".to_string(),
            PageNotes {
                title: "A [draft]".into(),
                notes: vec![
                    Note { id: 1, kind: Kind::Quote, text: "line one\nline two".into(), added_at: 0.0 },
                    Note { id: 2, kind: Kind::Note, text: "check this".into(), added_at: 0.0 },
                ],
            },
        );
        let text = markdown(&all);
        assert!(text.contains("## [A \\[draft\\]](https://a.dev/x)"), "{text}");
        assert!(text.contains("> line one\n> line two"), "{text}");
        assert!(text.contains("- check this"), "{text}");
        assert_eq!(markdown_link("Rust (lang)", "https://x.dev/a b)"), "[Rust (lang)](https://x.dev/a%20b%29)");
        assert_eq!(page_key("https://a.dev/x#top"), "https://a.dev/x");
    }
}
