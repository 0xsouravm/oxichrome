use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::cookies::{self, Query};
use oxichrome::{popup, tabs};
use serde_json::json;

use crate::rules::{self, Stopped};

#[popup(style = ["base.css", "quiet-web.css"])]
fn Popup() -> impl IntoView {
    let site = RwSignal::new(None::<String>);
    let tab_id = RwSignal::new(None::<i32>);
    let access = RwSignal::new(false);
    let jar = RwSignal::new(0usize);
    let pages = RwSignal::new(0u32);
    let requests = RwSignal::new(Vec::<Stopped>::new());
    let note = RwSignal::new(String::new());

    spawn_local(async move {
        let found: Vec<tabs::Tab> = tabs::query(&json!({ "active": true, "currentWindow": true })).await.unwrap_or_default();
        if let Some(tab) = found.into_iter().next() {
            let url = tab.url.unwrap_or_default();
            site.set(rules::is_web(&url).then(|| rules::host(&url)));
            tab_id.set(tab.id);
        }
        access.set(rules::has_all_sites().await);
        if let (Some(host), true) = (site.get_untracked(), access.get_untracked()) {
            jar.set(cookies::all(Query::new().domain(host)).await.map_or(0, |jar| jar.len()));
        }
        pages.set(rules::pages_stopped().await);
        requests.set(rules::requests_stopped().await);
    });

    let block = move |_| {
        let Some(host) = site.get_untracked() else { return };
        spawn_local(async move {
            match rules::change(|state| state.blocked.push(host)).await {
                Ok(_) => {
                    if let Some(id) = tab_id.get_untracked() {
                        let _ = tabs::reload(id, false).await;
                    }
                }
                Err(err) => note.set(err.to_string()),
            }
        });
    };
    let clear = move |_| {
        let Some(host) = site.get_untracked() else { return };
        spawn_local(async move {
            match cookies::remove_all(Query::new().domain(&host)).await {
                Ok(gone) => {
                    note.set(format!("Cleared {} cookies for {host}", gone.len()));
                    jar.set(0);
                }
                Err(err) => note.set(err.to_string()),
            }
        });
    };

    view! {
        <div class="popup">
            <header class="head">
                <img class="mark" src="/icons/48.png" alt="" />
                <div class="titles">
                    <h1>"Quiet Web"</h1>
                    <p>{move || site.get().unwrap_or_else(|| "Not a web page".into())}</p>
                </div>
                <a class="btn ghost" href="/options.html" target="_blank">"Settings"</a>
            </header>
            <div class="body">
                <Show when=move || site.with(Option::is_some)>
                    <section class="card">
                        <div class="setting">
                            <div class="main">
                                <div class="label">"Cookies"</div>
                                <div class="hint">{move || if access.get() { format!("{} stored for this site", jar.get()) } else { "Allow access to all sites in the settings to see them.".into() }}</div>
                            </div>
                            <button class="btn" on:click=clear disabled=move || jar.get() == 0>"Clear"</button>
                        </div>
                        <div class="pad">
                            <button class="btn danger" on:click=block>{move || format!("Never open {}", site.get().unwrap_or_default())}</button>
                        </div>
                    </section>
                </Show>
                <section class="card">
                    <div class="card-head"><span class="section-title">"Trackers stopped"</span></div>
                    {move || {
                        if requests.with(Vec::is_empty) {
                            let hint = if access.get() { "None yet." } else { "Allow access to all sites in the settings to see them here." };
                            return view! { <div class="pad muted">{hint}</div> }.into_any();
                        }
                        view! {
                            <div class="list">
                                {requests.get().into_iter().map(|entry| view! {
                                    <div class="item"><span class="main title">{entry.host}</span><span class="chip">{entry.kind}</span></div>
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </section>
            </div>
            <footer class="foot">
                <span>{move || match pages.get() { 1 => "1 page stopped".to_string(), n => format!("{n} pages stopped") }}</span>
                <span>{move || note.get()}</span>
            </footer>
        </div>
    }
}
