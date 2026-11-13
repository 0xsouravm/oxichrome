use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::{commands, t, windows};

use crate::tidy::{self, Group, TabInfo};

pub const SEARCH: &str = r#"<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>"#;
pub const PIN: &str = r#"<path d="M12 17v5"/><path d="M9 10.8a2 2 0 0 1-1.1 1.8l-1.8.9A2 2 0 0 0 5 15.2V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.8a2 2 0 0 0-1.1-1.8l-1.8-.9a2 2 0 0 1-1.1-1.8V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>"#;
pub const SOUND: &str = r#"<path d="M11 5 6 9H2v6h4l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7"/><path d="M19 5a10 10 0 0 1 0 14"/>"#;
pub const MUTED: &str = r#"<path d="M11 5 6 9H2v6h4l5 4z"/><path d="m22 9-6 6"/><path d="m16 9 6 6"/>"#;
pub const WINDOW: &str = r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18"/><path d="M7 6.5h.01"/>"#;
pub const CLOSE: &str = r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#;
pub const PANEL: &str = r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/>"#;
pub const FOCUS: &str = r#"<path d="M7 17 17 7"/><path d="M8 7h9v9"/>"#;
pub const COPIES: &str = r#"<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>"#;
pub const PLUS: &str = r#"<path d="M12 5v14"/><path d="M5 12h14"/>"#;

pub fn icon(paths: &'static str) -> impl IntoView {
    view! { <svg viewBox="0 0 24 24" aria-hidden="true" inner_html=paths></svg> }
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

    pub fn show(&self, text: String) {
        let this = *self;
        this.text.set(text);
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

fn favicon(tab: &TabInfo) -> impl IntoView {
    match tab.icon() {
        Some(src) => view! { <span class="favicon"><img src=src alt="" /></span> }.into_any(),
        None => {
            let letter = tab.host().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "·".into());
            view! { <span class="favicon">{letter}</span> }.into_any()
        }
    }
}

#[component]
fn TabRow(tab: TabInfo, duplicate: bool, toast: Toast, refresh: Callback<()>, switched: Callback<()>) -> impl IntoView {
    let id = tab.id;
    let window_id = tab.window_id;
    let pinned = tab.pinned;
    let muted = tab.muted();
    let sound = tab.audible || muted;
    let host = tab.host();
    let after = move |done: Option<String>| {
        if let Some(text) = done {
            toast.show(text);
        }
        refresh.run(());
    };
    let open = move |_| {
        spawn_local(async move {
            if tidy::switch_to(id, window_id).await.is_ok() {
                switched.run(());
            }
        });
    };
    let pin = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        spawn_local(async move {
            let _ = tidy::set_pinned(id, !pinned).await;
            after(None);
        });
    };
    let mute = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        spawn_local(async move {
            let _ = tidy::set_muted(id, !muted).await;
            after(None);
        });
    };
    let detach = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        spawn_local(async move {
            let moved = tidy::move_to_new_window(id).await;
            after(Some(match moved {
                Ok(()) => t!(toast_moved),
                Err(err) => err.to_string(),
            }));
        });
    };
    let close = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        spawn_local(async move {
            tidy::close(&[id]).await;
            after(None);
        });
    };
    let pin_label = if pinned { t!(unpin) } else { t!(pin) };
    let mute_label = if muted { t!(unmute) } else { t!(mute) };
    view! {
        <div class="item clickable" class:current=tab.active role="button" tabindex="0" on:click=open title=tab.url.clone()>
            {favicon(&tab)}
            <div class="main">
                <span class="title">{tab.label()}</span>
                <span class="sub">
                    {host}
                    {pinned.then(|| view! { <span class="dot">"·"</span> {t!(pinned)} })}
                    {duplicate.then(|| view! { <span class="dot">"·"</span> <span class="warn">{t!(duplicate)}</span> })}
                </span>
            </div>
            {sound.then(|| view! {
                <button class="icon-btn" class:on=tab.audible && !muted title=mute_label.clone() aria-label=mute_label on:click=mute>
                    {icon(if muted { MUTED } else { SOUND })}
                </button>
            })}
            <div class="actions">
                <button class="icon-btn" class:on=pinned title=pin_label.clone() aria-label=pin_label on:click=pin>{icon(PIN)}</button>
                <button class="icon-btn" title=t!(move_window) aria-label=t!(move_window) on:click=detach>{icon(WINDOW)}</button>
                <button class="icon-btn" title=t!(close_tab) aria-label=t!(close_tab) on:click=close>{icon(CLOSE)}</button>
            </div>
        </div>
    }
}

#[component]
fn WindowCard(group: Group, duplicates: Vec<i32>, toast: Toast, refresh: Callback<()>, switched: Callback<()>) -> impl IntoView {
    let window_id = group.window_id;
    let name = if group.current { t!(this_window) } else { t!(other_window, group.number) };
    let count = tidy::tabs_label(group.tabs.len());
    let focus = move |_| {
        spawn_local(async move {
            if tidy::focus_window(window_id).await.is_ok() {
                switched.run(());
            }
        });
    };
    view! {
        <section class="card">
            <div class="card-head">
                <span class="section-title">{name}</span>
                <span class="row-flex">
                    <span class="chip">{count}</span>
                    {(!group.current).then(|| view! {
                        <button class="icon-btn" title=t!(focus_window) aria-label=t!(focus_window) on:click=focus>{icon(FOCUS)}</button>
                    })}
                </span>
            </div>
            <div class="list">
                {group.tabs.into_iter().map(|tab| {
                    let duplicate = duplicates.contains(&tab.id);
                    view! { <TabRow tab duplicate toast refresh switched /> }
                }).collect_view()}
            </div>
        </section>
    }
}

#[component]
pub fn TabBrowser(switched: Callback<()>, toast: Toast) -> impl IntoView {
    let tabs = RwSignal::new(Vec::<TabInfo>::new());
    let current = RwSignal::new(None::<i32>);
    let loaded = RwSignal::new(false);
    let query = RwSignal::new(String::new());
    let refresh = Callback::new(move |_| {
        spawn_local(async move {
            if current.get_untracked().is_none() {
                current.set(windows::current().await.ok().map(|window| window.id));
            }
            let fresh = tidy::all().await;
            if fresh != tabs.get_untracked() {
                tabs.set(fresh);
            }
            loaded.set(true);
        });
    });
    refresh.run(());
    let ticker = set_interval_with_handle(move || refresh.run(()), Duration::from_millis(1500)).ok();
    on_cleanup(move || {
        if let Some(ticker) = ticker {
            ticker.clear();
        }
    });

    let duplicates = Memo::new(move |_| tabs.with(|tabs| tidy::duplicates(tabs)));
    let visible = Memo::new(move |_| {
        let query = query.get();
        tabs.with(|tabs| tabs.iter().filter(|tab| tidy::matches(tab, &query)).cloned().collect::<Vec<_>>())
    });
    let close_duplicates = move |_| {
        spawn_local(async move {
            let closed = tidy::close_duplicates().await;
            toast.show(if closed == 0 { t!(toast_none) } else { tidy::closed_label(closed) });
            refresh.run(());
        });
    };
    let first_match = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() == "Enter" {
            if let Some(tab) = visible.get_untracked().into_iter().next() {
                spawn_local(async move {
                    if tidy::switch_to(tab.id, tab.window_id).await.is_ok() {
                        switched.run(());
                    }
                });
            }
        }
    };

    view! {
        <label class="field">
            {icon(SEARCH)}
            <input
                type="text"
                autofocus
                placeholder=t!(search_placeholder)
                aria-label=t!(search_placeholder)
                prop:value=move || query.get()
                on:input=move |ev| query.set(event_target_value(&ev))
                on:keydown=first_match
            />
        </label>
        <Show when=move || !duplicates.with(|ids| ids.is_empty())>
            <div class="dupes">
                <span class="dupes-icon">{icon(COPIES)}</span>
                <div class="grow">
                    <div class="dupes-title">{move || tidy::duplicates_label(duplicates.with(|ids| ids.len()))}</div>
                    <div class="dupes-hint">{t!(dupes_hint)}</div>
                </div>
                <button class="btn primary" on:click=close_duplicates>{t!(close_dupes)}</button>
            </div>
        </Show>
        {move || {
            if !loaded.get() {
                return view! { <div class="skeleton card"></div> }.into_any();
            }
            let shown = visible.get();
            if shown.is_empty() {
                return view! {
                    <div class="card empty">
                        {icon(SEARCH)}
                        <strong>{t!(no_match_title)}</strong>
                        <span>{t!(no_match_body)}</span>
                    </div>
                }
                .into_any();
            }
            let ids = duplicates.get();
            tidy::group(shown, current.get())
                .into_iter()
                .map(|group| view! { <WindowCard group duplicates=ids.clone() toast refresh switched /> })
                .collect_view()
                .into_any()
        }}
    }
}

#[component]
pub fn Footer() -> impl IntoView {
    let total = RwSignal::new(0u32);
    let shortcut = RwSignal::new(None::<String>);
    spawn_local(async move {
        total.set(tidy::closed_total().await);
        shortcut.set(crate::close_duplicates::COMMAND.shortcut().await.ok().flatten());
    });
    let settings = move |_| {
        spawn_local(async move {
            let _ = commands::open_shortcut_settings().await;
        });
    };
    view! {
        <footer class="foot">
            <span>{move || t!(closed_so_far, total.get())}</span>
            <span class="row-flex">
                {move || match shortcut.get() {
                    Some(keys) => view! { <span class="kbd">{keys}</span> <span>{t!(shortcut_label)}</span> }.into_any(),
                    None => view! { <span>{t!(shortcut_none)}</span> }.into_any(),
                }}
                <button class="link" on:click=settings>{t!(change_shortcut)}</button>
            </span>
        </footer>
    }
}
