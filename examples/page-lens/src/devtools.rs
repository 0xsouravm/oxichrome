use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::devtools;
use oxichrome::devtools_panel;

use crate::minutes;
use crate::page::PageInfo;

const INSPECT: &str = r#"(() => {
  if (document.readyState === "loading") throw new Error("The page is still loading");
  const root = document.querySelector("article, main, [role=main]") || document.body;
  const text = (el) => el.innerText.split(/\s+/).filter(Boolean);
  return {
    title: document.title,
    url: location.href,
    words: text(root).length,
    links: document.links.length,
    headings: [...root.querySelectorAll("h1, h2, h3")]
      .map((el) => ({ level: Number(el.tagName[1]), text: text(el).join(" ") }))
      .filter((h) => h.text),
  };
})()"#;

fn load(info: RwSignal<Option<Result<PageInfo, String>>>, tries: u8) {
    spawn_local(async move {
        let found = devtools::eval::<PageInfo>(INSPECT).await.map_err(|err| err.to_string());
        if found.is_err() && tries > 0 {
            set_timeout(move || load(info, tries - 1), Duration::from_millis(250));
        } else {
            info.set(Some(found));
        }
    });
}

#[devtools_panel(title = "Page Lens", style = ["base.css", "page-lens.css"])]
fn Outline() -> impl IntoView {
    let info = RwSignal::new(None::<Result<PageInfo, String>>);
    let refresh = move || load(info, 20);
    let _ = devtools::on_shown(refresh);
    let _ = devtools::on_navigated(move |_url| refresh());

    view! {
        <div class="panel">
            <header class="head">
                <div class="titles">
                    <h1>"Page Lens"</h1>
                    <p>{format!("Tab {}", devtools::inspected_tab_id().unwrap_or_default())}</p>
                </div>
                <button class="btn" on:click=move |_| refresh()>"Refresh"</button>
            </header>
            <div class="body">
                {move || match info.get() {
                    None => view! { <p class="muted">"Loading…"</p> }.into_any(),
                    Some(Err(reason)) => view! { <p class="mono faint">{reason}</p> }.into_any(),
                    Some(Ok(found)) => view! {
                        <p>{format!("{} words, {} min read, {} links", found.words, minutes(found.words), found.links)}</p>
                        <div class="list outline">
                            {found.headings.into_iter().map(|heading| view! {
                                <div class="item" data-depth=heading.level.saturating_sub(1).to_string()><span class="title">{heading.text}</span></div>
                            }).collect_view()}
                        </div>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}
