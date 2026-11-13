use std::time::Duration;

use leptos::prelude::*;
use oxichrome::tabs;
use serde_json::json;

pub const CLOCK: &str = r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#;
pub const NOTE: &str = r#"<path d="M4 4h16v12l-4 4H4z"/><path d="M16 20v-4h4"/><path d="M8 9h8"/><path d="M8 13h5"/>"#;
pub const COPY: &str = r#"<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>"#;
pub const DOWNLOAD: &str = r#"<path d="M12 4v11"/><path d="m7 10 5 5 5-5"/><path d="M5 20h14"/>"#;
pub const QUOTE: &str = r#"<path d="M7 7h4v4c0 3-1.5 5-4 6"/><path d="M14 7h4v4c0 3-1.5 5-4 6"/>"#;
pub const TRASH: &str = r#"<path d="M4 7h16"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M6 7l1 13h10l1-13"/><path d="M9 7V4h6v3"/>"#;
pub const LOCK: &str = r#"<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>"#;
pub const ALERT: &str = r#"<circle cx="12" cy="12" r="9"/><path d="M12 8v5"/><path d="M12 16h.01"/>"#;

pub fn icon(paths: &'static str) -> impl IntoView {
    view! { <svg viewBox="0 0 24 24" aria-hidden="true" inner_html=paths></svg> }
}

pub fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

pub async fn active_tab() -> Option<tabs::Tab> {
    let found: Vec<tabs::Tab> = tabs::query(&json!({ "active": true, "currentWindow": true })).await.ok()?;
    found.into_iter().next()
}

#[derive(Clone, Copy)]
pub struct Toast {
    text: RwSignal<String>,
    shown: RwSignal<bool>,
    generation: RwSignal<u32>,
}

impl Toast {
    pub fn new() -> Self {
        Self { text: RwSignal::new(String::new()), shown: RwSignal::new(false), generation: RwSignal::new(0) }
    }

    pub fn show(&self, text: impl Into<String>) {
        let this = *self;
        this.text.set(text.into());
        this.shown.set(true);
        this.generation.update(|generation| *generation += 1);
        let mine = this.generation.get_untracked();
        set_timeout(
            move || {
                if this.generation.get_untracked() == mine {
                    this.shown.set(false);
                }
            },
            Duration::from_millis(1800),
        );
    }

    pub fn view(&self) -> impl IntoView {
        let this = *self;
        view! {
            <div class="toast" role="status" data-show=move || this.shown.get().to_string()>
                {move || this.text.get()}
            </div>
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1240), "1,240");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
