use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::{popup, scripting, side_panel, storage};

use crate::notes::{self, Settings};
use crate::page::{self, PageInfo, LINKS_CSS, READABLE_CSS};
use crate::ui::{active_tab, host, icon, thousands, Toast, ALERT, CLOCK, COPY, NOTE};

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Readable,
    Links,
}

impl Look {
    fn css(self) -> &'static str {
        match self {
            Look::Readable => READABLE_CSS,
            Look::Links => LINKS_CSS,
        }
    }

    fn key(self, tab_id: i32) -> String {
        match self {
            Look::Readable => format!("readable:{tab_id}"),
            Look::Links => format!("links:{tab_id}"),
        }
    }
}

#[popup(style = ["base.css", "page-lens.css"])]
fn Popup() -> impl IntoView {
    let toast = Toast::new();
    let tab_id = RwSignal::new(None::<i32>);
    let info = RwSignal::new(None::<Result<PageInfo, String>>);
    let readable = RwSignal::new(false);
    let links = RwSignal::new(false);
    let note_count = RwSignal::new(0usize);
    let settings = RwSignal::new(Settings::default());

    spawn_local(async move {
        settings.set(notes::settings().await);
        let Some(tab) = active_tab().await else {
            info.set(Some(Err("There's no tab to look at.".into())));
            return;
        };
        let Some(id) = tab.id else { return };
        tab_id.set(Some(id));
        readable.set(storage::session::get::<bool>(&Look::Readable.key(id)).await.ok().flatten().unwrap_or(false));
        links.set(storage::session::get::<bool>(&Look::Links.key(id)).await.ok().flatten().unwrap_or(false));
        let found = page::inspect::SCRIPT.run(id, ()).await.map_err(|err| err.to_string());
        if let Ok(found) = &found {
            note_count.set(notes::for_page(&found.url).await.notes.len());
        }
        info.set(Some(found));
    });

    let toggle = move |look: Look, signal: RwSignal<bool>| {
        let Some(id) = tab_id.get_untracked() else { return };
        let on = !signal.get_untracked();
        spawn_local(async move {
            let done = if on { scripting::insert_css(id, look.css()).await } else { scripting::remove_css(id, look.css()).await };
            match done {
                Ok(()) => {
                    signal.set(on);
                    let _ = storage::session::set(&look.key(id), &on).await;
                }
                Err(err) => toast.show(err.to_string()),
            }
        });
    };
    let jump = move |index: usize| {
        let Some(id) = tab_id.get_untracked() else { return };
        spawn_local(async move {
            let _ = page::scroll_to::SCRIPT.run(id, index).await;
        });
    };
    let copy = move |_| {
        let Some(Ok(found)) = info.get_untracked() else { return };
        spawn_local(async move {
            match crate::copy_text(notes::markdown_link(&found.title, &found.url)).await {
                Ok(()) => toast.show("Markdown link copied"),
                Err(err) => toast.show(err.to_string()),
            }
        });
    };
    let open_notes = side_panel::open_on_click(move |opened| async move {
        match opened {
            Ok(()) => {
                if let Some(window) = web_sys::window() {
                    let _ = window.close();
                }
            }
            Err(err) => toast.show(err.to_string()),
        }
    });
    let pill = move |_| {
        settings.update(|settings| settings.show_pill = !settings.show_pill);
        let fresh = settings.get_untracked();
        spawn_local(async move {
            let _ = notes::save_settings(&fresh).await;
        });
    };

    view! {
        <div class="popup">
            <header class="head">
                <img class="mark" src="/icons/48.png" alt="" />
                <div class="titles">
                    <h1 class="ellipsis">{move || match info.get() {
                        Some(Ok(found)) if !found.title.is_empty() => found.title,
                        _ => "Page Lens".to_string(),
                    }}</h1>
                    <p>{move || match info.get() {
                        Some(Ok(found)) => host(&found.url),
                        Some(Err(_)) => "Not available on this page".to_string(),
                        None => "Reading the page…".to_string(),
                    }}</p>
                </div>
            </header>
            <div class="body">
                {move || match info.get() {
                    None => view! { <div class="card skeleton"></div> }.into_any(),
                    Some(Err(reason)) => view! {
                        <div class="card empty">
                            {icon(ALERT)}
                            <strong>"Page Lens can't read this page"</strong>
                            <span>"Browser pages and the extension stores are off limits to every extension."</span>
                            <span class="mono faint reason">{reason}</span>
                        </div>
                    }.into_any(),
                    Some(Ok(found)) => {
                        let outline = found.headings.clone();
                        let top = outline.iter().map(|heading| heading.level).min().unwrap_or(1);
                        view! {
                            <div class="stats">
                                <div class="stat accent-stat">
                                    <div class="value">{icon(CLOCK)}{format!("{} min", found.minutes())}</div>
                                    <div class="label">"Reading time"</div>
                                </div>
                                <div class="stat"><div class="value">{thousands(found.words)}</div><div class="label">"Words"</div></div>
                                <div class="stat"><div class="value">{thousands(found.links as usize)}</div><div class="label">"Links"</div></div>
                            </div>
                            {(!found.description.is_empty()).then(|| view! { <p class="description">{found.description.clone()}</p> })}
                            <section class="card">
                                <div class="card-head">
                                    <span class="section-title">"Outline"</span>
                                    <span class="chip">{outline.len()}</span>
                                </div>
                                {if outline.is_empty() {
                                    view! { <div class="empty small">"This page has no headings."</div> }.into_any()
                                } else {
                                    view! {
                                        <div class="list outline">
                                            {outline.into_iter().enumerate().map(|(index, heading)| {
                                                let depth = heading.level.saturating_sub(top).min(2);
                                                view! {
                                                    <button class="item clickable" data-depth=depth.to_string() on:click=move |_| jump(index)>
                                                        <span class="title">{heading.text}</span>
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }}
                            </section>
                            <section class="card">
                                <div class="setting">
                                    <div class="main"><div class="label">"Readable text"</div><div class="hint">"Bigger type and shorter lines on this tab."</div></div>
                                    <button class="switch" role="switch" aria-label="Readable text" aria-checked=move || readable.get().to_string() on:click=move |_| toggle(Look::Readable, readable)></button>
                                </div>
                                <div class="setting">
                                    <div class="main"><div class="label">"Outline links"</div><div class="hint">"Draw a box around every link."</div></div>
                                    <button class="switch" role="switch" aria-label="Outline links" aria-checked=move || links.get().to_string() on:click=move |_| toggle(Look::Links, links)></button>
                                </div>
                            </section>
                        }.into_any()
                    }
                }}
            </div>
            <div class="actions-row">
                    <button class="btn" on:click=copy disabled=move || !matches!(info.get(), Some(Ok(_)))>{icon(COPY)}"Copy Markdown link"</button>
                    <button class="btn primary" on:click=open_notes>
                        {icon(NOTE)}"Notes"
                        <Show when=move || { note_count.get() > 0 }>
                            <span class="count">{move || note_count.get()}</span>
                        </Show>
                    </button>
            </div>
            <footer class="foot">
                <span>"Reading time on Wikipedia and MDN"</span>
                <button class="switch" role="switch" aria-label="Reading time on Wikipedia and MDN" aria-checked=move || settings.with(|settings| settings.show_pill).to_string() on:click=pill></button>
            </footer>
            {toast.view()}
        </div>
    }
}
