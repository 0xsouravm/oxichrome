use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::cookies::{self, Cookie, Query};
use oxichrome::{permissions, popup, tabs};
use serde_json::{json, Value};

use crate::rules::{self, Stats, LISTS};
use crate::ui::{icon, Switch, Toast, BAN, COOKIE, GEAR, LOCK, PAUSED, SHIELD};

async fn active() -> Option<tabs::Tab> {
    let found: Vec<tabs::Tab> = tabs::query(&json!({ "active": true, "currentWindow": true })).await.ok()?;
    found.into_iter().next()
}

async fn reload(tab_id: Option<i32>) {
    if let Some(id) = tab_id {
        let _: Result<Value, _> = tabs::update(id, &json!({ "url": tabs::get::<tabs::Tab>(id).await.ok().and_then(|tab| tab.url) })).await;
    }
}

#[popup(style = ["base.css", "quiet-web.css"])]
fn Popup() -> impl IntoView {
    let toast = Toast::new();
    let site = RwSignal::new(String::new());
    let tab_id = RwSignal::new(None::<i32>);
    let web = RwSignal::new(false);
    let paused = RwSignal::new(false);
    let lists = RwSignal::new(Vec::<String>::new());
    let stats = RwSignal::new(Stats::default());
    let access = RwSignal::new(None::<bool>);
    let jar = RwSignal::new(Vec::<Cookie>::new());

    let load_cookies = move || {
        spawn_local(async move {
            let host = site.get_untracked();
            let allowed = permissions::contains(rules::site_access(&host)).await.unwrap_or(false);
            access.set(Some(allowed));
            if allowed {
                jar.set(cookies::all(Query::new().domain(&host)).await.unwrap_or_default());
            }
        });
    };
    spawn_local(async move {
        if let Some(tab) = active().await {
            let url = tab.url.unwrap_or_default();
            web.set(rules::is_web(&url));
            site.set(rules::host(&url));
            tab_id.set(tab.id);
        }
        paused.set(rules::state().await.paused.contains(&site.get_untracked()));
        lists.set(rules::enabled_lists().await);
        stats.set(rules::stats().await);
        if web.get_untracked() {
            load_cookies();
        }
    });

    let pause = Callback::new(move |on: bool| {
        let host = site.get_untracked();
        spawn_local(async move {
            let done = rules::change(|state| {
                state.paused.retain(|site| *site != host);
                if on {
                    state.paused.push(host.clone());
                }
            })
            .await;
            match done {
                Ok(_) => {
                    paused.set(on);
                    reload(tab_id.get_untracked()).await;
                }
                Err(err) => toast.show(err.to_string()),
            }
        });
    });
    let block = move |_| {
        let host = site.get_untracked();
        spawn_local(async move {
            match rules::change(|state| {
                state.paused.retain(|site| *site != host);
                state.blocked.push(host.clone());
            })
            .await
            {
                Ok(_) => reload(tab_id.get_untracked()).await,
                Err(err) => toast.show(err.to_string()),
            }
        });
    };
    let flip_list = move |id: &'static str| {
        Callback::new(move |on: bool| {
            spawn_local(async move {
                match rules::set_list(id, on).await {
                    Ok(()) => lists.set(rules::enabled_lists().await),
                    Err(err) => toast.show(err.to_string()),
                }
            });
        })
    };
    let clear = move |_| {
        let host = site.get_untracked();
        spawn_local(async move {
            match cookies::remove_all(Query::new().domain(&host)).await {
                Ok(_) => {
                    toast.show(format!("Cleared cookies for {host}"));
                    jar.set(Vec::new());
                }
                Err(err) => toast.show(err.to_string()),
            }
        });
    };
    let allow = move || {
        permissions::request_on_click(rules::site_access(&site.get_untracked()), move |granted| async move {
            match granted {
                Ok(true) => load_cookies(),
                Ok(false) => toast.show("Cookies stay hidden without access to this site"),
                Err(err) => toast.show(err.to_string()),
            }
        })
    };

    view! {
        <div class="popup" class:is-paused=move || paused.get()>
            <header class="head">
                <img class="mark" src="/icons/48.png" alt="" />
                <div class="titles">
                    <h1>"Quiet Web"</h1>
                    <p>{move || if web.get() { site.get() } else { "Not a web page".to_string() }}</p>
                </div>
                <a class="icon-btn" href="/options.html" target="_blank" title="Settings" aria-label="Settings">{icon(GEAR)}</a>
            </header>
            <div class="body">
                <Show
                    when=move || web.get()
                    fallback=|| view! {
                        <div class="card empty">
                            {icon(SHIELD)}
                            <strong>"Nothing to block here"</strong>
                            <span>"Quiet Web works on http and https pages."</span>
                        </div>
                    }
                >
                    <section class="card status">
                        <div class="setting">
                            <span class="status-icon">{move || if paused.get() { icon(PAUSED).into_any() } else { icon(SHIELD).into_any() }}</span>
                            <div class="main">
                                <div class="label">{move || if paused.get() { "Paused on this site" } else { "Protected" }}</div>
                                <div class="hint">{move || if paused.get() { "Everything loads here, including trackers.".to_string() } else { format!("Lists are on for {}.", site.get()) }}</div>
                            </div>
                            <Switch label="Protection on this site".into() on=Signal::derive(move || !paused.get()) flip=Callback::new(move |on: bool| pause.run(!on)) />
                        </div>
                        <div class="divider"></div>
                        <button class="btn ghost danger block-site" on:click=block>{icon(BAN)}{move || format!("Never open {}", site.get())}</button>
                    </section>
                </Show>
                <section class="card">
                    <div class="card-head"><span class="section-title">"Filter lists"</span></div>
                    {LISTS.iter().map(|list| {
                        let id = list.id;
                        view! {
                            <div class="setting">
                                <div class="main">
                                    <div class="label">{list.name}</div>
                                    <div class="hint">{list.about}</div>
                                </div>
                                <Switch label=list.name.to_string() on=Signal::derive(move || lists.with(|on| on.iter().any(|set| set == id))) flip=flip_list(id) />
                            </div>
                        }
                    }).collect_view()}
                </section>
                <Show when=move || web.get()>
                    <section class="card">
                        <div class="card-head">
                            <span class="section-title row-flex">{icon(COOKIE)}"Cookies"</span>
                            <Show when=move || { access.get() == Some(true) }>
                                <button class="btn ghost danger" on:click=clear disabled=move || jar.with(|jar| jar.is_empty())>"Clear"</button>
                            </Show>
                        </div>
                        {move || match access.get() {
                            None => view! { <div class="pad muted">"Checking…"</div> }.into_any(),
                            Some(false) => view! {
                                <div class="pad stack">
                                    <span class="muted">"Quiet Web needs access to this site to read its cookies."</span>
                                    <button class="btn" on:click=allow()>{icon(LOCK)}{format!("Allow on {}", site.get_untracked())}</button>
                                </div>
                            }.into_any(),
                            Some(true) if jar.with(|jar| jar.is_empty()) => view! { <div class="pad muted">"No cookies for this site."</div> }.into_any(),
                            Some(true) => view! {
                                <div class="pad cookie-list">
                                    {jar.get().into_iter().take(12).map(|cookie| view! {
                                        <span class="chip" title=format!("{} · {}", cookie.domain, cookie.path)>{cookie.name}</span>
                                    }).collect_view()}
                                    {move || { let more = jar.with(|jar| jar.len().saturating_sub(12)); (more > 0).then(|| view! { <span class="chip">{format!("+{more}")}</span> }) }}
                                </div>
                            }.into_any(),
                        }}
                    </section>
                </Show>
            </div>
            <footer class="foot">
                <span>{move || stats.with(|stats| format!("{} pages stopped today", stats.stopped))}</span>
                <span>{move || stats.with(|stats| format!("{} loaded", stats.loaded))}</span>
            </footer>
            {toast.view()}
        </div>
    }
}
