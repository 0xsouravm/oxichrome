use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::declarative_net_request as dnr;
use oxichrome::{options_page, permissions};

use crate::rules::{self, State, Stats, LISTS};
use crate::ui::{icon, Switch, Toast, BAN, CLOSE, LOCK, PAUSED, PLAY};

#[options_page(style = ["base.css", "quiet-web.css"])]
fn Options() -> impl IntoView {
    let toast = Toast::new();
    let state = RwSignal::new(State::default());
    let stats = RwSignal::new(Stats::default());
    let lists = RwSignal::new(Vec::<String>::new());
    let all_sites = RwSignal::new(false);
    let badge = RwSignal::new(false);
    let badge_note = RwSignal::new(None::<String>);
    let draft = RwSignal::new(String::new());
    let draft_error = RwSignal::new(None::<String>);

    let refresh = move || {
        spawn_local(async move {
            state.set(rules::state().await);
            stats.set(rules::stats().await);
            lists.set(rules::enabled_lists().await);
            all_sites.set(rules::has_all_sites().await);
        });
    };
    refresh();

    let edit = move |change: Box<dyn FnOnce(&mut State)>| {
        spawn_local(async move {
            match rules::change(change).await {
                Ok(next) => state.set(next),
                Err(err) => toast.show(err.to_string()),
            }
        });
    };
    let add = move || {
        match rules::clean_site(&draft.get_untracked()) {
            Ok(site) => {
                draft_error.set(None);
                draft.set(String::new());
                edit(Box::new(move |state| {
                    state.paused.retain(|paused| *paused != site);
                    state.blocked.push(site);
                }));
            }
            Err(message) => draft_error.set(Some(message)),
        }
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
    let strip = Callback::new(move |on: bool| edit(Box::new(move |state| state.strip_params = on)));
    let show_badge = Callback::new(move |on: bool| {
        spawn_local(async move {
            match dnr::show_match_count_on_badge(on).await {
                Ok(()) => {
                    badge.set(on);
                    badge_note.set(None);
                }
                Err(err) => badge_note.set(Some(err.to_string())),
            }
        });
    });
    let grant = move || {
        permissions::request_on_click(rules::all_sites(), move |granted| async move {
            match granted {
                Ok(true) => {
                    all_sites.set(true);
                    toast.show("Blocked sites now show a page that says why");
                }
                Ok(false) => toast.show("Nothing changed"),
                Err(err) => toast.show(err.to_string()),
            }
        })
    };
    let revoke = move |_| {
        spawn_local(async move {
            match permissions::remove(rules::all_sites()).await {
                Ok(_) => all_sites.set(false),
                Err(err) => toast.show(err.to_string()),
            }
        });
    };

    view! {
        <main class="page">
            <header class="head">
                <img class="mark" src="/icons/128.png" alt="" />
                <div class="titles">
                    <h1>"Quiet Web"</h1>
                    <p>"Network rules that run inside the browser. Nothing you visit leaves your computer."</p>
                </div>
            </header>

            <section class="stats">
                <div class="stat"><div class="value">{move || stats.with(|s| s.stopped)}</div><div class="label">"Pages stopped today"</div></div>
                <div class="stat"><div class="value">{move || stats.with(|s| s.loaded)}</div><div class="label">"Pages loaded today"</div></div>
                <div class="stat"><div class="value">{move || state.with(|s| s.blocked.len())}</div><div class="label">"Blocked sites"</div></div>
                <div class="stat"><div class="value">{move || state.with(|s| s.paused.len())}</div><div class="label">"Paused sites"</div></div>
            </section>

            <section class="card">
                <div class="card-head"><span class="section-title">"Filter lists"</span></div>
                {LISTS.iter().map(|list| {
                    let id = list.id;
                    view! {
                        <div class="setting">
                            <div class="main"><div class="label">{list.name}</div><div class="hint">{list.about}</div></div>
                            <Switch label=list.name.to_string() on=Signal::derive(move || lists.with(|on| on.iter().any(|set| set == id))) flip=flip_list(id) />
                        </div>
                    }
                }).collect_view()}
            </section>

            <section class="card">
                <div class="card-head"><span class="section-title">"Blocked sites"</span></div>
                <form class="pad add-site" on:submit=move |ev| { ev.prevent_default(); add(); }>
                    <label class="field grow">
                        {icon(BAN)}
                        <input
                            type="text"
                            placeholder="youtube.com"
                            aria-label="Site to block"
                            prop:value=move || draft.get()
                            on:input=move |ev| { draft.set(event_target_value(&ev)); draft_error.set(None); }
                        />
                    </label>
                    <button class="btn primary" type="submit">"Block"</button>
                </form>
                {move || draft_error.get().map(|message| view! { <div class="notice danger inset">{message}</div> })}
                {move || {
                    let blocked = state.with(|s| s.blocked.clone());
                    if blocked.is_empty() {
                        return view! { <div class="empty small">"Sites you block can't be opened, in any tab."</div> }.into_any();
                    }
                    view! {
                        <div class="list">
                            {blocked.into_iter().map(|site| {
                                let gone = site.clone();
                                view! {
                                    <div class="item">
                                        <span class="favicon">{site.chars().next().map(|c| c.to_ascii_uppercase().to_string())}</span>
                                        <div class="main"><span class="title">{site.clone()}</span></div>
                                        <button class="icon-btn" title=format!("Unblock {site}") aria-label=format!("Unblock {site}") on:click=move |_| {
                                            let gone = gone.clone();
                                            edit(Box::new(move |state| state.blocked.retain(|s| *s != gone)));
                                        }>{icon(CLOSE)}</button>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </section>

            <Show when=move || state.with(|s| !s.paused.is_empty())>
                <section class="card">
                    <div class="card-head"><span class="section-title">"Paused on"</span></div>
                    <div class="list">
                        {move || state.with(|s| s.paused.clone()).into_iter().map(|site| {
                            let back = site.clone();
                            view! {
                                <div class="item">
                                    <span class="favicon paused">{icon(PAUSED)}</span>
                                    <div class="main"><span class="title">{site.clone()}</span><span class="sub">"Lists don't run here"</span></div>
                                    <button class="btn" on:click=move |_| {
                                        let back = back.clone();
                                        edit(Box::new(move |state| state.paused.retain(|s| *s != back)));
                                    }>{icon(PLAY)}"Resume"</button>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                </section>
            </Show>

            <section class="card">
                <div class="card-head"><span class="section-title">"Extras"</span></div>
                <div class="setting">
                    <div class="main">
                        <div class="label">"Strip tracking parameters"</div>
                        <div class="hint">"Removes utm_source, fbclid, gclid and friends from addresses before the page loads. Needs access to all sites."</div>
                    </div>
                    <Switch label="Strip tracking parameters".into() on=Signal::derive(move || state.with(|s| s.strip_params) && all_sites.get()) disabled=Signal::derive(move || !all_sites.get()) flip=strip />
                </div>
                <div class="setting">
                    <div class="main">
                        <div class="label">"Count blocked requests on the toolbar"</div>
                        <div class="hint">{move || badge_note.get().unwrap_or_else(|| "The browser counts them per tab. Chrome only.".to_string())}</div>
                    </div>
                    <Switch label="Count blocked requests on the toolbar".into() on=badge.into() flip=show_badge />
                </div>
            </section>

            <section class="card">
                <div class="setting">
                    <span class="status-icon">{icon(LOCK)}</span>
                    <div class="main">
                        <div class="label">{move || if all_sites.get() { "Access to all sites: on" } else { "Access to all sites: off" }}</div>
                        <div class="hint">"Blocking works either way. With access, a blocked site shows a page that says why, and tracking parameters can be stripped."</div>
                    </div>
                    {move || if all_sites.get() {
                        view! { <button class="btn" on:click=revoke>"Remove"</button> }.into_any()
                    } else {
                        view! { <button class="btn primary" on:click=grant()>"Allow"</button> }.into_any()
                    }}
                </div>
            </section>
            {toast.view()}
        </main>
    }
}
