use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::downloads::{self, Conflict, Save};
use oxichrome::{popup, side_panel, tabs};
use serde_json::json;

use crate::page::{self, PageInfo};
use crate::{copy, markdown_link, minutes};

pub async fn active_tab() -> Option<tabs::Tab> {
    let found: Vec<tabs::Tab> = tabs::query(&json!({ "active": true, "currentWindow": true })).await.ok()?;
    found.into_iter().next()
}

pub fn outline_markdown(info: &PageInfo) -> String {
    let mut out = format!("# {}\n\n{} words, {} min read\n\n", markdown_link(&info.title, &info.url), info.words, minutes(info.words));
    for heading in &info.headings {
        out.push_str(&format!("{}- {}\n", "  ".repeat(heading.level.saturating_sub(1) as usize), heading.text));
    }
    out
}

#[popup(style = ["base.css", "page-lens.css"])]
fn Popup() -> impl IntoView {
    let tab_id = RwSignal::new(None::<i32>);
    let info = RwSignal::new(None::<Result<PageInfo, String>>);
    let status = RwSignal::new(String::new());

    spawn_local(async move {
        let Some(id) = active_tab().await.and_then(|tab| tab.id) else {
            info.set(Some(Err("There's no tab to look at.".into())));
            return;
        };
        tab_id.set(Some(id));
        info.set(Some(page::inspect::SCRIPT.run(id, ()).await.map_err(|err| err.to_string())));
    });

    let jump = move |index: usize| {
        let Some(id) = tab_id.get_untracked() else { return };
        spawn_local(async move {
            let _ = page::scroll_to::SCRIPT.run(id, index).await;
        });
    };
    let copy_link = move |_| {
        let Some(Ok(found)) = info.get_untracked() else { return };
        spawn_local(async move {
            status.set(match copy::OFFSCREEN.run(markdown_link(&found.title, &found.url)).await {
                Ok(true) => "Markdown link copied".into(),
                Ok(false) => "The browser didn't allow copying".into(),
                Err(err) => err.to_string(),
            });
        });
    };
    let save_outline = move |_| {
        let Some(Ok(found)) = info.get_untracked() else { return };
        spawn_local(async move {
            let saved = downloads::save(Save::text("outline.md", outline_markdown(&found)).on_conflict(Conflict::Uniquify)).await;
            status.set(saved.map_or_else(|err| err.to_string(), |_| "Saved outline.md".into()));
        });
    };
    let open_panel = side_panel::open_on_click(move |opened| async move {
        if let Err(err) = opened {
            status.set(err.to_string());
        }
    });

    view! {
        <div class="popup">
            <header class="head">
                <img class="mark" src="/icons/48.png" alt="" />
                <div class="titles">
                    <h1 class="ellipsis">{move || match info.get() {
                        Some(Ok(found)) if !found.title.is_empty() => found.title,
                        _ => "Page Lens".to_string(),
                    }}</h1>
                    <p>{move || status.get()}</p>
                </div>
            </header>
            <div class="body">
                {move || match info.get() {
                    None => view! { <div class="card empty">"Reading the page…"</div> }.into_any(),
                    Some(Err(reason)) => view! {
                        <div class="card empty">
                            <strong>"Page Lens can't read this page"</strong>
                            <span class="mono faint">{reason}</span>
                        </div>
                    }.into_any(),
                    Some(Ok(found)) => {
                        let top = found.headings.iter().map(|heading| heading.level).min().unwrap_or(1);
                        view! {
                            <div class="stats">
                                <div class="stat"><div class="value">{format!("{} min", minutes(found.words))}</div><div class="label">"Reading time"</div></div>
                                <div class="stat"><div class="value">{found.words}</div><div class="label">"Words"</div></div>
                                <div class="stat"><div class="value">{found.links}</div><div class="label">"Links"</div></div>
                            </div>
                            <section class="card">
                                <div class="card-head"><span class="section-title">"Outline"</span></div>
                                <div class="list outline">
                                    {found.headings.into_iter().enumerate().map(|(index, heading)| view! {
                                        <button class="item" data-depth=heading.level.saturating_sub(top).to_string() on:click=move |_| jump(index)>
                                            <span class="title">{heading.text}</span>
                                        </button>
                                    }).collect_view()}
                                </div>
                            </section>
                        }.into_any()
                    }
                }}
            </div>
            <div class="actions-row">
                <button class="btn" on:click=copy_link disabled=move || !matches!(info.get(), Some(Ok(_)))>"Copy Markdown link"</button>
                <button class="btn" on:click=save_outline disabled=move || !matches!(info.get(), Some(Ok(_)))>"Save outline"</button>
                <button class="btn primary" on:click=open_panel>"Reading progress"</button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::Heading;

    #[test]
    fn outline_nests_by_level() {
        let info = PageInfo {
            title: "Rust".into(),
            url: "https://x.dev".into(),
            words: 400,
            links: 0,
            headings: vec![Heading { level: 1, text: "Intro".into() }, Heading { level: 2, text: "Ownership".into() }],
        };
        assert_eq!(outline_markdown(&info), "# [Rust](https://x.dev)\n\n400 words, 2 min read\n\n- Intro\n  - Ownership\n");
    }
}
