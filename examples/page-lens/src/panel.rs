use std::collections::BTreeMap;
use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::permissions::{self, PermissionSet};
use oxichrome::{side_panel, tabs};
use serde_json::json;

use crate::notes::{self, Kind, Note, PageNotes};
use crate::page;
use crate::ui::{active_tab, host, icon, Toast, DOWNLOAD, LOCK, NOTE, QUOTE, TRASH};

fn all_sites() -> PermissionSet {
    PermissionSet::new().origin("https://*/*").origin("http://*/*")
}

fn ago(added_at: f64) -> String {
    let minutes = ((js_sys::Date::now() - added_at) / 60_000.0).floor() as i64;
    match minutes {
        ..=0 => "just now".to_string(),
        1..=59 => format!("{minutes} min ago"),
        60..=1439 => format!("{} h ago", minutes / 60),
        _ => format!("{} d ago", minutes / 1440),
    }
}

#[side_panel(title = "Page Lens", style = ["base.css", "page-lens.css"])]
fn Notes() -> impl IntoView {
    let toast = Toast::new();
    let url = RwSignal::new(String::new());
    let title = RwSignal::new(String::new());
    let tab_id = RwSignal::new(None::<i32>);
    let all = RwSignal::new(BTreeMap::<String, PageNotes>::new());
    let draft = RwSignal::new(String::new());
    let can_read = RwSignal::new(false);

    let refresh = move || {
        spawn_local(async move {
            if let Some(tab) = active_tab().await {
                let fresh_url = tab.url.unwrap_or_default();
                if fresh_url != url.get_untracked() {
                    url.set(fresh_url);
                }
                title.set(tab.title.unwrap_or_default());
                tab_id.set(tab.id);
            }
            let fresh = notes::all().await;
            if fresh != all.get_untracked() {
                all.set(fresh);
            }
            can_read.set(permissions::contains(all_sites()).await.unwrap_or(false));
        });
    };
    refresh();
    let ticker = set_interval_with_handle(refresh, Duration::from_millis(1500)).ok();
    on_cleanup(move || {
        if let Some(ticker) = ticker {
            ticker.clear();
        }
    });

    let here = Memo::new(move |_| all.with(|all| all.get(&notes::page_key(&url.get())).cloned().unwrap_or_default()));
    let others = Memo::new(move |_| {
        let key = notes::page_key(&url.get());
        all.with(|all| all.iter().filter(|(page, _)| **page != key).map(|(page, notes)| (page.clone(), notes.clone())).collect::<Vec<_>>())
    });

    let add_note = move |_| {
        let text = draft.get_untracked();
        spawn_local(async move {
            match notes::add(&url.get_untracked(), &title.get_untracked(), Kind::Note, &text).await {
                Ok(()) => draft.set(String::new()),
                Err(err) => toast.show(err.to_string()),
            }
            refresh();
        });
    };
    let add_selection = move |_| {
        let Some(id) = tab_id.get_untracked() else { return };
        spawn_local(async move {
            match page::selection::SCRIPT.run(id, ()).await {
                Ok(text) if text.trim().is_empty() => toast.show("Select some text on the page first"),
                Ok(text) => {
                    let _ = notes::add(&url.get_untracked(), &title.get_untracked(), Kind::Quote, &text).await;
                    toast.show("Quote saved");
                }
                Err(err) => toast.show(err.to_string()),
            }
            refresh();
        });
    };
    let allow = move || {
        permissions::request_on_click(all_sites(), move |granted| async move {
            match granted {
                Ok(true) => {
                    can_read.set(true);
                    toast.show("Page Lens can now read selections");
                }
                Ok(false) => toast.show("Not allowed. Use the right-click menu instead."),
                Err(err) => toast.show(err.to_string()),
            }
        })
    };
    let export = move |_| {
        spawn_local(async move {
            match notes::export().await {
                Ok(0) => toast.show("Nothing to export yet"),
                Ok(count) => toast.show(format!("Exported {count} notes")),
                Err(err) => toast.show(err.to_string()),
            }
        });
    };
    let remove = move |id: u64| {
        spawn_local(async move {
            let _ = notes::remove(&url.get_untracked(), id).await;
            refresh();
        });
    };
    let open = move |page: String| {
        spawn_local(async move {
            let _: Result<tabs::Tab, _> = tabs::create(&json!({ "url": page })).await;
        });
    };
    let close = side_panel::close_on_click(|_| async {});

    view! {
        <div class="panel">
            <header class="head">
                <img class="mark" src="/icons/48.png" alt="" />
                <div class="titles">
                    <h1>"Notes"</h1>
                    <p>{move || { let host = host(&url.get()); if host.is_empty() { "Page Lens".to_string() } else { host } }}</p>
                </div>
                <button class="btn" on:click=export title="Download every note as Markdown">{icon(DOWNLOAD)}"Export"</button>
                <button class="icon-btn" on:click=close title="Close" aria-label="Close">
                    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M18 6 6 18" /><path d="m6 6 12 12" /></svg>
                </button>
            </header>
            <div class="body">
                <section class="card">
                    <div class="pad stack">
                        <div class="page-title ellipsis">{move || { let t = title.get(); if t.is_empty() { url.get() } else { t } }}</div>
                        <textarea
                            class="area"
                            placeholder="Write a note about this page"
                            aria-label="Write a note about this page"
                            prop:value=move || draft.get()
                            on:input=move |ev| draft.set(event_target_value(&ev))
                        ></textarea>
                        <div class="row-flex">
                            <Show
                                when=move || can_read.get()
                                fallback=move || view! {
                                    <button class="btn" on:click=allow() title="Lets Page Lens read the text you select on any site">{icon(LOCK)}"Allow quotes from pages"</button>
                                }
                            >
                                <button class="btn" on:click=add_selection>{icon(QUOTE)}"Add selection"</button>
                            </Show>
                            <span class="grow"></span>
                            <button class="btn primary" on:click=add_note disabled=move || draft.with(|text| text.trim().is_empty())>{icon(NOTE)}"Add note"</button>
                        </div>
                    </div>
                </section>
                {move || {
                    let page = here.get();
                    if page.notes.is_empty() {
                        return view! {
                            <div class="card empty">
                                {icon(QUOTE)}
                                <strong>"Nothing saved for this page"</strong>
                                <span>"Select text and right-click → Save to Page Lens, or press Alt+Shift+Q."</span>
                            </div>
                        }.into_any();
                    }
                    view! {
                        <section class="card">
                            <div class="card-head"><span class="section-title">"This page"</span><span class="chip">{page.notes.len()}</span></div>
                            <div class="list notes">
                                {page.notes.into_iter().rev().map(|note: Note| {
                                    let id = note.id;
                                    view! {
                                        <div class="note" class:quote={note.kind == Kind::Quote}>
                                            <p>{note.text}</p>
                                            <div class="note-meta">
                                                <span>{ago(note.added_at)}</span>
                                                <button class="icon-btn" title="Delete" aria-label="Delete" on:click=move |_| remove(id)>{icon(TRASH)}</button>
                                            </div>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        </section>
                    }.into_any()
                }}
                <Show when=move || others.with(|others| !others.is_empty())>
                    <section class="card">
                        <div class="card-head"><span class="section-title">"Other pages"</span></div>
                        <div class="list">
                            {move || others.get().into_iter().map(|(page, saved)| {
                                let target = page.clone();
                                let name = if saved.title.is_empty() { page.clone() } else { saved.title.clone() };
                                view! {
                                    <button class="item clickable" on:click=move |_| open(target.clone())>
                                        <div class="main">
                                            <span class="title">{name}</span>
                                            <span class="sub">{host(&page)}</span>
                                        </div>
                                        <span class="chip">{saved.notes.len()}</span>
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    </section>
                </Show>
            </div>
            {toast.view()}
        </div>
    }
}
