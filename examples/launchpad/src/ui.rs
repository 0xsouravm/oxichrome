use std::time::Duration;

use leptos::prelude::*;

use crate::places::{hue, Place};

pub const SEARCH: &str = r#"<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>"#;
pub const STAR: &str = r#"<path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1 6.2L12 17.3 6.5 20.2l1-6.2L3 9.6l6.2-.9z"/>"#;
pub const CLOCK: &str = r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#;
pub const CLOSE: &str = r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#;
pub const GEAR: &str = r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>"#;
pub const TRASH: &str = r#"<path d="M4 7h16"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M6 7l1 13h10l1-13"/><path d="M9 7V4h6v3"/>"#;

pub fn icon(paths: &'static str) -> impl IntoView {
    view! { <svg viewBox="0 0 24 24" aria-hidden="true" inner_html=paths></svg> }
}

pub fn letter(place: &Place) -> impl IntoView {
    let host = place.host();
    let first = host.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "·".into());
    view! { <span class="letter" style=format!("--hue: {}", hue(&host))>{first}</span> }
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
            Duration::from_millis(2000),
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
