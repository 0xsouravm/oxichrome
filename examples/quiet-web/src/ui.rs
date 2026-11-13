use std::time::Duration;

use leptos::prelude::*;

pub const SHIELD: &str = r#"<path d="M12 3 4.5 6v5.5c0 4.6 3.1 8.2 7.5 9.5 4.4-1.3 7.5-4.9 7.5-9.5V6z"/><path d="m9 12 2 2 4-4"/>"#;
pub const PAUSED: &str = r#"<path d="M12 3 4.5 6v5.5c0 4.6 3.1 8.2 7.5 9.5 4.4-1.3 7.5-4.9 7.5-9.5V6z"/><path d="M10 9.5v5"/><path d="M14 9.5v5"/>"#;
pub const BAN: &str = r#"<circle cx="12" cy="12" r="9"/><path d="m5.6 5.6 12.8 12.8"/>"#;
pub const COOKIE: &str = r#"<path d="M12 3a9 9 0 1 0 9 9 3 3 0 0 1-3-3 3 3 0 0 1-3-3 3 3 0 0 1-3-3z"/><path d="M8.5 11.5h.01"/><path d="M15 15h.01"/><path d="M10.5 16h.01"/>"#;
pub const GEAR: &str = r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>"#;
pub const CLOSE: &str = r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#;
pub const LOCK: &str = r#"<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>"#;
pub const PLAY: &str = r#"<path d="M7 5v14l11-7z"/>"#;

pub fn icon(paths: &'static str) -> impl IntoView {
    view! { <svg viewBox="0 0 24 24" aria-hidden="true" inner_html=paths></svg> }
}

#[component]
pub fn Switch(label: String, on: Signal<bool>, #[prop(optional)] disabled: Signal<bool>, flip: Callback<bool>) -> impl IntoView {
    view! {
        <button
            class="switch"
            role="switch"
            aria-label=label
            aria-checked=move || on.get().to_string()
            disabled=move || disabled.get()
            on:click=move |_| flip.run(!on.get_untracked())
        ></button>
    }
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
            Duration::from_millis(2200),
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
