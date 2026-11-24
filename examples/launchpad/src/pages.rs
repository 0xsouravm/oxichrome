use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::management::{self, SelfInfo};
use oxichrome::omnibox::Disposition;
use oxichrome::{bookmarks, options_page, popup, tabs};

use crate::{go, is_web, Place, FOLDER};

async fn current_page() -> Option<Place> {
    let tab = tabs::active_tab().await.ok()??;
    let url = tab.url.filter(|url| is_web(url))?;
    Some(Place { title: tab.title.unwrap_or_default(), url })
}

fn rows(places: Vec<Place>, open: impl Fn(String) + Copy + 'static) -> impl IntoView {
    places
        .into_iter()
        .map(|place| {
            let url = place.url.clone();
            view! {
                <button class="item clickable" title=place.url.clone() on:click=move |_| open(url.clone())>
                    <div class="main"><span class="title">{place.name()}</span><span class="sub">{crate::host(&place.url)}</span></div>
                </button>
            }
        })
        .collect_view()
}

#[popup(style = ["base.css", "launchpad.css"])]
fn Popup() -> impl IntoView {
    let query = RwSignal::new(String::new());
    let page = RwSignal::new(None::<Place>);
    let saved = RwSignal::new(false);
    let top = RwSignal::new(Vec::<Place>::new());
    let recent = RwSignal::new(Vec::<Place>::new());
    let visits = RwSignal::new(0u32);
    let status = RwSignal::new(String::new());

    spawn_local(async move {
        if let Some(place) = current_page().await {
            saved.set(crate::saved().await.iter().any(|(_, mark)| mark.url == place.url));
            page.set(Some(place));
        }
        top.set(crate::top(8).await);
        recent.set(crate::recent(6).await);
        visits.set(crate::visits().await);
    });

    let open = move |text: String| {
        spawn_local(async move {
            if let Err(err) = go(&text, Disposition::NewForegroundTab).await {
                status.set(err.to_string());
            }
        });
    };
    let save = move |_| {
        let Some(place) = page.get_untracked() else { return };
        spawn_local(async move {
            match crate::save(&place).await {
                Ok(()) => saved.set(true),
                Err(err) => status.set(err.to_string()),
            }
        });
    };

    view! {
        <div class="popup">
            <div class="body">
                <form class="field" on:submit=move |ev| { ev.prevent_default(); open(query.get_untracked()); }>
                    <input type="text" autofocus placeholder="Search or type an address" aria-label="Search or type an address"
                        on:input=move |ev| query.set(event_target_value(&ev)) />
                </form>
                <div class="tiles">
                    {move || top.get().into_iter().map(|place| {
                        let url = place.url.clone();
                        view! { <button class="btn ghost tile" title=place.url.clone() on:click=move |_| open(url.clone())>{place.name()}</button> }
                    }).collect_view()}
                </div>
                {move || page.get().map(|place| view! {
                    <section class="card">
                        <div class="card-head">
                            <span class="section-title ellipsis">{place.name()}</span>
                            <button class="btn" disabled=move || saved.get() on:click=save>
                                {move || if saved.get() { "Saved" } else { "Save this page" }}
                            </button>
                        </div>
                    </section>
                })}
                <section class="card">
                    <div class="card-head">
                        <span class="section-title">"Recently visited"</span>
                        <span class="chip">{move || format!("{} visits this session", visits.get())}</span>
                    </div>
                    <div class="list">{move || rows(recent.get(), open)}</div>
                </section>
                <p class="muted">{move || status.get()}</p>
            </div>
            <footer class="foot">
                <span>"In the address bar, type "<span class="kbd">"lp"</span>" and a space"</span>
                <a href="/options.html" target="_blank">"Saved pages"</a>
            </footer>
        </div>
    }
}

#[options_page(style = ["base.css", "launchpad.css"])]
fn Options() -> impl IntoView {
    let me = RwSignal::new(None::<SelfInfo>);
    let saved = RwSignal::new(Vec::<(String, Place)>::new());
    let load = move || spawn_local(async move { saved.set(crate::saved().await) });
    load();
    spawn_local(async move { me.set(management::get_self().await.ok()) });
    let remove = move |id: String| {
        spawn_local(async move {
            let _ = bookmarks::remove(&id).await;
            load();
        });
    };

    view! {
        <main class="page">
            <header class="head">
                <img class="mark" src="/icons/128.png" alt="" />
                <div class="titles"><h1>"Launchpad"</h1><p>"Pages you saved from the popup, kept as ordinary bookmarks."</p></div>
            </header>
            <section class="card">
                <div class="card-head">
                    <span class="section-title">"Saved pages"</span>
                    <span class="chip">{format!("Other bookmarks › {FOLDER}")}</span>
                </div>
                <div class="list">
                    {move || saved.get().into_iter().map(|(id, place)| view! {
                        <div class="item">
                            <div class="main"><span class="title">{place.name()}</span><span class="sub">{place.url.clone()}</span></div>
                            <button class="icon-btn" aria-label=format!("Remove {}", place.name()) on:click=move |_| remove(id.clone())>"×"</button>
                        </div>
                    }).collect_view()}
                </div>
                <Show when=move || saved.with(Vec::is_empty)>
                    <div class="empty">"Nothing saved yet. Use Save this page in the popup."</div>
                </Show>
            </section>
            <section class="card">
                <div class="card-head"><span class="section-title">"About"</span></div>
                {move || me.get().map(|me| view! {
                    <dl class="about">
                        <dt>"Version"</dt><dd class="mono">{me.version}</dd>
                        <dt>"Installed"</dt><dd>{format!("{:?}", me.install_type)}</dd>
                        <dt>"ID"</dt><dd class="mono ellipsis">{me.id}</dd>
                    </dl>
                })}
            </section>
        </main>
    }
}
