use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::management::{self, InstallType, SelfInfo};
use oxichrome::{history, options_page};

use crate::places::{self, Place};
use crate::ui::{icon, letter, Toast, CLOSE, TRASH};

fn install_label(kind: InstallType) -> &'static str {
    match kind {
        InstallType::Development => "Loaded unpacked",
        InstallType::Normal => "From a store",
        InstallType::Admin => "Installed by an administrator",
        InstallType::Sideload => "Installed by another program",
        InstallType::Other => "Installed",
    }
}

#[options_page(style = ["base.css", "launchpad.css"])]
fn Options() -> impl IntoView {
    let toast = Toast::new();
    let me = RwSignal::new(None::<SelfInfo>);
    let saved = RwSignal::new(Vec::<(String, Place)>::new());
    let confirm = RwSignal::new(false);

    let load = move || {
        spawn_local(async move {
            saved.set(places::saved().await);
        });
    };
    load();
    spawn_local(async move {
        me.set(management::get_self().await.ok());
    });

    let unsave = move |id: String| {
        spawn_local(async move {
            if let Err(err) = places::unsave(&id).await {
                toast.show(err.to_string());
            }
            load();
        });
    };
    let forget = move |_| {
        if !confirm.get_untracked() {
            confirm.set(true);
            return;
        }
        confirm.set(false);
        spawn_local(async move {
            let now = js_sys::Date::now();
            match history::delete_range(now - 3_600_000.0, now).await {
                Ok(()) => toast.show("The last hour is gone from your history"),
                Err(err) => toast.show(err.to_string()),
            }
        });
    };
    let uninstall = management::uninstall_on_click(move |kept| async move {
        match kept {
            Ok(false) => toast.show("Glad you're staying"),
            Ok(true) => {}
            Err(err) => toast.show(err.to_string()),
        }
    });

    view! {
        <main class="page">
            <header class="head">
                <img class="mark" src="/icons/128.png" alt="" />
                <div class="titles">
                    <h1>"Launchpad"</h1>
                    <p>"Everything here comes from the browser's own bookmarks, history and top sites."</p>
                </div>
            </header>

            <section class="card">
                <div class="card-head">
                    <span class="section-title">"Saved pages"</span>
                    <span class="chip">{move || format!("Other bookmarks › {}", places::FOLDER)}</span>
                </div>
                {move || {
                    let list = saved.get();
                    if list.is_empty() {
                        return view! { <div class="empty small">"Nothing saved yet. Use “Save this page” in the popup."</div> }.into_any();
                    }
                    view! {
                        <div class="list">
                            {list.into_iter().map(|(id, place)| {
                                let name = place.name();
                                view! {
                                    <div class="item">
                                        {letter(&place)}
                                        <div class="main"><span class="title">{name.clone()}</span><span class="sub">{place.url.clone()}</span></div>
                                        <button class="icon-btn" title=format!("Remove {name}") aria-label=format!("Remove {name}") on:click=move |_| unsave(id.clone())>{icon(CLOSE)}</button>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </section>

            <section class="card">
                <div class="setting">
                    <div class="main">
                        <div class="label">"Forget the last hour"</div>
                        <div class="hint">"Removes every page you visited in the last 60 minutes from the browser's history."</div>
                    </div>
                    <button class="btn danger" class:armed=move || confirm.get() on:click=forget>
                        {icon(TRASH)}{move || if confirm.get() { "Click again to forget" } else { "Forget" }}
                    </button>
                </div>
            </section>

            <section class="card">
                <div class="card-head"><span class="section-title">"About this extension"</span></div>
                {move || me.get().map(|me| view! {
                    <dl class="about">
                        <dt>"Version"</dt><dd class="mono">{me.version.clone()}</dd>
                        <dt>"Installed"</dt><dd>{install_label(me.install_type)}</dd>
                        <dt>"ID"</dt><dd class="mono ellipsis">{me.id.clone()}</dd>
                        <dt>"Permissions"</dt>
                        <dd class="chips">{me.permissions.iter().map(|permission| view! { <span class="chip">{format!("{permission:?}")}</span> }).collect_view()}</dd>
                    </dl>
                })}
                <div class="divider"></div>
                <div class="setting">
                    <div class="main">
                        <div class="label">"Uninstall Launchpad"</div>
                        <div class="hint">"The browser asks you to confirm. Your bookmarks stay where they are."</div>
                    </div>
                    <button class="btn danger" on:click=uninstall>"Uninstall"</button>
                </div>
            </section>
            {toast.view()}
        </main>
    }
}
