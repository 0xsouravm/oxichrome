use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::search::Where;
use oxichrome::{popup, tabs};
use serde_json::json;

use crate::places::{self, Place};
use crate::ui::{icon, letter, Toast, CLOCK, GEAR, SEARCH, STAR};

fn close() {
    if let Some(window) = web_sys::window() {
        let _ = window.close();
    }
}

#[popup(style = ["base.css", "launchpad.css"])]
fn Popup() -> impl IntoView {
    let toast = Toast::new();
    let query = RwSignal::new(String::new());
    let top = RwSignal::new(Vec::<Place>::new());
    let recent = RwSignal::new(Vec::<(Place, f64)>::new());
    let saved = RwSignal::new(Vec::<(String, Place)>::new());
    let current = RwSignal::new(None::<Place>);
    let visits = RwSignal::new(0u32);
    let loaded = RwSignal::new(false);

    let load_saved = move || {
        spawn_local(async move {
            saved.set(places::saved().await);
        });
    };
    spawn_local(async move {
        let found: Vec<tabs::Tab> = tabs::query(&json!({ "active": true, "currentWindow": true })).await.unwrap_or_default();
        current.set(found.into_iter().next().and_then(|tab| {
            let url = tab.url.filter(|url| url.starts_with("http"))?;
            Some(Place { title: tab.title.unwrap_or_default(), url })
        }));
        top.set(places::top(8).await);
        recent.set(places::recent_history(6).await);
        saved.set(places::saved().await);
        visits.set(places::visits_today().await);
        loaded.set(true);
    });

    let submit = move |new_tab: bool| {
        let text = query.get_untracked();
        if text.trim().is_empty() {
            return;
        }
        spawn_local(async move {
            match places::go(&text, if new_tab { Where::NewTab } else { Where::CurrentTab }).await {
                Ok(()) => close(),
                Err(err) => toast.show(err.to_string()),
            }
        });
    };
    let open = move |url: String| {
        spawn_local(async move {
            match places::open(&url, false).await {
                Ok(()) => close(),
                Err(err) => toast.show(err.to_string()),
            }
        });
    };
    let saved_id = Memo::new(move |_| {
        let url = current.with(|place| place.as_ref().map(|place| place.url.clone()))?;
        saved.with(|saved| saved.iter().find(|(_, place)| place.url == url).map(|(id, _)| id.clone()))
    });
    let star = move |_| {
        let Some(page) = current.get_untracked() else { return };
        let existing = saved_id.get_untracked();
        spawn_local(async move {
            let done = match existing {
                Some(id) => places::unsave(&id).await.map(|_| "Removed from Launchpad"),
                None => places::save(&page.title, &page.url).await.map(|_| "Saved to Launchpad"),
            };
            match done {
                Ok(text) => toast.show(text),
                Err(err) => toast.show(err.to_string()),
            }
            load_saved();
        });
    };

    view! {
        <div class="popup">
            <div class="hero">
                <form class="field big" on:submit=move |ev| { ev.prevent_default(); submit(true); }>
                    {icon(SEARCH)}
                    <input
                        type="text"
                        autofocus
                        placeholder="Search or type an address"
                        aria-label="Search or type an address"
                        prop:value=move || query.get()
                        on:input=move |ev| query.set(event_target_value(&ev))
                        on:keydown=move |ev| if ev.key() == "Enter" && ev.shift_key() { ev.prevent_default(); submit(false); }
                    />
                    <span class="kbd">"↵"</span>
                </form>
                <div class="hint-line">
                    <span>"Enter opens a new tab, Shift+Enter uses this one. In the address bar, type "<span class="kbd">"lp"</span>" and a space."</span>
                </div>
            </div>
            <div class="body">
                <section>
                    <div class="row-head"><span class="section-title">"Most visited"</span></div>
                    {move || {
                        let sites = top.get();
                        if loaded.get() && sites.is_empty() {
                            return view! { <div class="card empty small">"Sites you visit often will show up here."</div> }.into_any();
                        }
                        view! {
                            <div class="tiles">
                                {sites.into_iter().map(|place| {
                                    let url = place.url.clone();
                                    view! {
                                        <button class="tile" title=place.url.clone() on:click=move |_| open(url.clone())>
                                            {letter(&place)}
                                            <span class="tile-name">{place.name()}</span>
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </section>
                <section class="card">
                    <div class="card-head">
                        <span class="section-title">"Recently visited"</span>
                        <span class="chip">{move || format!("{} today", visits.get())}</span>
                    </div>
                    <div class="list">
                        {move || recent.get().into_iter().map(|(place, when)| {
                            let url = place.url.clone();
                            view! {
                                <button class="item clickable" title=place.url.clone() on:click=move |_| open(url.clone())>
                                    {letter(&place)}
                                    <div class="main"><span class="title">{place.name()}</span><span class="sub">{place.host()}</span></div>
                                    <span class="faint when">{icon(CLOCK)}{places::ago(when)}</span>
                                </button>
                            }
                        }).collect_view()}
                    </div>
                </section>
                <section class="card">
                    <div class="card-head">
                        <span class="section-title">"Saved"</span>
                        <Show when=move || current.with(Option::is_some)>
                            <button class="btn" class:starred=move || saved_id.with(Option::is_some) on:click=star>
                                {icon(STAR)}{move || if saved_id.with(Option::is_some) { "Saved" } else { "Save this page" }}
                            </button>
                        </Show>
                    </div>
                    {move || {
                        let list = saved.get();
                        if list.is_empty() {
                            return view! { <div class="empty small">"Pages you save land in a Launchpad bookmarks folder."</div> }.into_any();
                        }
                        view! {
                            <div class="list">
                                {list.into_iter().take(6).map(|(_, place)| {
                                    let url = place.url.clone();
                                    view! {
                                        <button class="item clickable" title=place.url.clone() on:click=move |_| open(url.clone())>
                                            {letter(&place)}
                                            <div class="main"><span class="title">{place.name()}</span><span class="sub">{place.host()}</span></div>
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </section>
            </div>
            <footer class="foot">
                <span>"Bookmarks · History · Top sites"</span>
                <a class="icon-btn" href="/options.html" target="_blank" title="Settings and about" aria-label="Settings and about">{icon(GEAR)}</a>
            </footer>
            {toast.view()}
        </div>
    }
}
