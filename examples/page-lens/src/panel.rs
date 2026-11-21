use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::permissions::{self, PermissionSet};
use oxichrome::runtime::Port;
use oxichrome::{side_panel, tabs};

use crate::page::{self, Progress};
use crate::popup::active_tab;

fn all_sites() -> PermissionSet {
    PermissionSet::new().origin("https://*/*").origin("http://*/*")
}

#[side_panel(title = "Page Lens", style = ["base.css", "page-lens.css"])]
fn Panel() -> impl IntoView {
    let title = RwSignal::new(String::new());
    let progress = RwSignal::new(None::<Progress>);
    let status = RwSignal::new("Open a page, then follow it here.".to_string());
    let allowed = RwSignal::new(true);
    let port = StoredValue::new_local(None::<Port>);
    let run = StoredValue::new(0u32);

    let follow = move || {
        run.update_value(|run| *run += 1);
        let mine = run.get_value();
        spawn_local(async move {
            if let Some(old) = port.get_value() {
                old.disconnect();
            }
            progress.set(None);
            allowed.set(permissions::contains(all_sites()).await.unwrap_or(false));
            let Some(tab) = active_tab().await else { return };
            let Some(id) = tab.id else { return };
            title.set(tab.title.unwrap_or_default());
            if let Err(err) = page::follow::SCRIPT.run(id, ()).await {
                status.set(err.to_string());
                return;
            }
            let Ok(next) = tabs::connect(id, "progress", None) else { return };
            port.set_value(Some(next.clone()));
            status.set(String::new());
            while let Some(Ok(update)) = next.recv::<Progress>().await {
                progress.set(Some(update));
            }
            if run.get_value() == mine {
                progress.set(None);
                status.set("The page changed or closed. Follow it again.".into());
            }
        });
    };
    follow();

    let allow = permissions::request_on_click(all_sites(), move |granted| async move {
        if matches!(granted, Ok(true)) {
            follow();
        }
    });

    view! {
        <div class="panel">
            <header class="head">
                <img class="mark" src="/icons/48.png" alt="" />
                <div class="titles">
                    <h1 class="ellipsis">{move || title.get()}</h1>
                    <p>"Reading progress"</p>
                </div>
            </header>
            <div class="body">
                <section class="card">
                    <div class="pad stack">
                        {move || match progress.get() {
                            Some(now) => view! {
                                <div class="meter"><span style:width=format!("{}%", now.percent)></span></div>
                                <p class="progress">{format!("{}% read, {} min left", now.percent, now.minutes_left)}</p>
                            }.into_any(),
                            None => view! { <p class="muted">{move || status.get()}</p> }.into_any(),
                        }}
                        <div class="row-flex">
                            <button class="btn" on:click=move |_| follow()>"Follow this tab"</button>
                            <button class="btn primary" style:display=move || if allowed.get() { "none" } else { "" } on:click=allow>"Allow on all sites"</button>
                        </div>
                    </div>
                </section>
            </div>
        </div>
    }
}
