use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::declarative_net_request as dnr;
use oxichrome::{options_page, permissions};

use crate::rules::{self, State, LISTS};

#[component]
fn Switch(label: &'static str, on: Signal<bool>, flip: Callback<bool>, #[prop(optional)] disabled: Signal<bool>) -> impl IntoView {
    view! {
        <button class="switch" role="switch" aria-label=label aria-checked=move || on.get().to_string()
            disabled=move || disabled.get() on:click=move |_| flip.run(!on.get_untracked())></button>
    }
}

#[options_page(style = ["base.css", "quiet-web.css"])]
fn Options() -> impl IntoView {
    let state = RwSignal::new(State::default());
    let lists = RwSignal::new(Vec::<String>::new());
    let all_sites = RwSignal::new(false);
    let badge = RwSignal::new(false);
    let badge_note = RwSignal::new(None::<String>);
    let draft = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);

    spawn_local(async move {
        state.set(rules::state().await);
        lists.set(rules::enabled_lists().await);
        all_sites.set(rules::has_all_sites().await);
    });

    let edit = move |change: Box<dyn FnOnce(&mut State)>| {
        spawn_local(async move {
            match rules::change(change).await {
                Ok(next) => state.set(next),
                Err(err) => error.set(Some(err.to_string())),
            }
        });
    };
    let add = move || match rules::clean_site(&draft.get_untracked()) {
        Ok(site) => {
            error.set(None);
            draft.set(String::new());
            edit(Box::new(move |state| state.blocked.push(site)));
        }
        Err(message) => error.set(Some(message)),
    };
    let flip_list = move |id: &'static str| {
        Callback::new(move |on: bool| {
            spawn_local(async move {
                match rules::set_list(id, on).await {
                    Ok(()) => lists.set(rules::enabled_lists().await),
                    Err(err) => error.set(Some(err.to_string())),
                }
            });
        })
    };
    let show_badge = Callback::new(move |on: bool| {
        spawn_local(async move {
            match dnr::show_match_count_on_badge(on).await {
                Ok(()) => badge.set(on),
                Err(err) => badge_note.set(Some(err.to_string())),
            }
        });
    });
    let grant = move || {
        permissions::request_on_click(rules::all_sites(), move |granted| async move {
            match granted {
                Ok(granted) => all_sites.set(granted),
                Err(err) => error.set(Some(err.to_string())),
            }
        })
    };
    let revoke = move |_| {
        spawn_local(async move {
            match permissions::remove(rules::all_sites()).await {
                Ok(_) => all_sites.set(false),
                Err(err) => error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <main class="page">
            <header class="head">
                <img class="mark" src="/icons/128.png" alt="" />
                <div class="titles">
                    <h1>"Quiet Web"</h1>
                    <p>"Network rules that run inside the browser."</p>
                </div>
            </header>

            <section class="card">
                <div class="card-head"><span class="section-title">"Filter lists"</span></div>
                {LISTS.map(|(id, name)| view! {
                    <div class="setting">
                        <div class="main label">{name}</div>
                        <Switch label=name on=Signal::derive(move || lists.with(|on| on.iter().any(|set| set == id))) flip=flip_list(id) />
                    </div>
                }).collect_view()}
            </section>

            <section class="card">
                <div class="card-head"><span class="section-title">"Blocked sites"</span></div>
                <form class="pad add-site" on:submit=move |ev| { ev.prevent_default(); add(); }>
                    <label class="field grow">
                        <input type="text" placeholder="youtube.com" aria-label="Site to block"
                            prop:value=move || draft.get() on:input=move |ev| draft.set(event_target_value(&ev)) />
                    </label>
                    <button class="btn primary" type="submit">"Block"</button>
                </form>
                {move || error.get().map(|message| view! { <div class="notice danger">{message}</div> })}
                <div class="list">
                    {move || state.get().blocked.into_iter().map(|site| {
                        let gone = site.clone();
                        view! {
                            <div class="item">
                                <span class="main title">{site.clone()}</span>
                                <button class="icon-btn" aria-label=format!("Unblock {site}")
                                    on:click=move |_| { let gone = gone.clone(); edit(Box::new(move |state| state.blocked.retain(|s| *s != gone))); }>"✕"</button>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </section>

            <section class="card">
                <div class="setting">
                    <div class="main">
                        <div class="label">"Access to all sites"</div>
                        <div class="hint">"Blocking works either way. With access, blocked sites show a page that says why, tracking parameters can be stripped, and stopped trackers are listed."</div>
                    </div>
                    {move || if all_sites.get() {
                        view! { <button class="btn" on:click=revoke>"Remove"</button> }.into_any()
                    } else {
                        view! { <button class="btn primary" on:click=grant()>"Allow"</button> }.into_any()
                    }}
                </div>
                <div class="setting">
                    <div class="main">
                        <div class="label">"Strip tracking parameters"</div>
                        <div class="hint">"Removes utm_source, fbclid and gclid from addresses. Needs access to all sites."</div>
                    </div>
                    <Switch label="Strip tracking parameters" on=Signal::derive(move || state.with(|s| s.strip_params) && all_sites.get())
                        disabled=Signal::derive(move || !all_sites.get()) flip=Callback::new(move |on: bool| edit(Box::new(move |state| state.strip_params = on))) />
                </div>
                <div class="setting">
                    <div class="main">
                        <div class="label">"Count blocked requests on the toolbar"</div>
                        <div class="hint">{move || badge_note.get().unwrap_or_else(|| "Chrome only.".into())}</div>
                    </div>
                    <Switch label="Count blocked requests on the toolbar" on=badge.into() flip=show_badge />
                </div>
            </section>
        </main>
    }
}
