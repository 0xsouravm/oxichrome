use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::prelude::*;
use oxichrome::{action, storage};
use serde::{Deserialize, Serialize};

#[extension(
    name = "Counter",
    description = "The smallest oxichrome extension: a number you can change from the popup, kept in storage and shown on the toolbar.",
    version = "0.3.0",
    permissions = [Storage],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = "Counter" },
    gecko_id = "counter@oxichrome.dev"
)]
pub struct Counter;

const KEY: &str = "counter";
const STEPS: [i64; 3] = [1, 5, 10];

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Count {
    pub value: i64,
    pub step: i64,
    pub badge: bool,
}

impl Default for Count {
    fn default() -> Self {
        Count { value: 0, step: 1, badge: true }
    }
}

async fn load() -> Count {
    storage::get(KEY).await.ok().flatten().unwrap_or_default()
}

async fn save(count: Count) {
    let _ = storage::set(KEY, &count).await;
    let text = if count.badge { short(count.value) } else { String::new() };
    let _ = action::set_badge_background_color("#0a7fbd", None).await;
    let _ = action::set_badge_text(&text, None).await;
}

fn short(value: i64) -> String {
    match value.abs() {
        0..=9_999 => value.to_string(),
        10_000..=999_999 => format!("{}k", value / 1_000),
        _ => format!("{}m", value / 1_000_000),
    }
}

#[background]
async fn start() {
    save(load().await).await;
}

fn use_count() -> RwSignal<Count> {
    let count = RwSignal::new(Count::default());
    spawn_local(async move { count.set(load().await) });
    count
}

fn change(count: RwSignal<Count>, edit: impl FnOnce(&mut Count)) {
    count.update(edit);
    let next = count.get_untracked();
    spawn_local(save(next));
}

#[popup(style = ["base.css", "counter.css"])]
fn Popup() -> impl IntoView {
    let count = use_count();
    view! {
        <div class="popup counter">
            <header class="head">
                <img class="mark" src="/icons/48.png" alt="" />
                <div class="titles"><h1>"Counter"</h1><p>"Saved in extension storage"</p></div>
            </header>
            <div class="value" aria-live="polite">{move || count.with(|c| c.value)}</div>
            <div class="controls">
                <button class="round" aria-label="Decrease" on:click=move |_| change(count, |c| c.value -= c.step)>"−"</button>
                <button class="round primary" aria-label="Increase" on:click=move |_| change(count, |c| c.value += c.step)>"+"</button>
            </div>
            <div class="foot">
                <span>{move || format!("Steps of {}", count.with(|c| c.step))}</span>
                <button class="btn ghost" disabled=move || { count.with(|c| c.value == 0) } on:click=move |_| change(count, |c| c.value = 0)>"Reset"</button>
            </div>
        </div>
    }
}

#[options_page(style = ["base.css", "counter.css"])]
fn Options() -> impl IntoView {
    let count = use_count();
    view! {
        <main class="page">
            <header class="head">
                <img class="mark" src="/icons/128.png" alt="" />
                <div class="titles">
                    <h1>"Counter"</h1>
                    <p>"Changes here are saved right away and the popup picks them up the next time it opens."</p>
                </div>
            </header>
            <section class="card">
                <div class="setting">
                    <div class="main"><div class="label">"Step"</div><div class="hint">"How much + and − change the number."</div></div>
                    <div class="segmented" role="group" aria-label="Step">
                        {STEPS.into_iter().map(|step| view! {
                            <button
                                aria-pressed=move || count.with(|c| c.step == step).to_string()
                                on:click=move |_| change(count, |c| c.step = step)
                            >{step}</button>
                        }).collect_view()}
                    </div>
                </div>
                <div class="setting">
                    <div class="main"><div class="label">"Show on the toolbar"</div><div class="hint">"Puts the number on the extension's button."</div></div>
                    <button
                        class="switch"
                        role="switch"
                        aria-label="Show on the toolbar"
                        aria-checked=move || count.with(|c| c.badge).to_string()
                        on:click=move |_| change(count, |c| c.badge = !c.badge)
                    ></button>
                </div>
                <div class="setting">
                    <div class="main"><div class="label">"Current value"</div><div class="hint mono">{move || count.with(|c| c.value)}</div></div>
                    <button class="btn danger" on:click=move |_| change(count, |c| c.value = 0)>"Reset"</button>
                </div>
            </section>
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_text_fits() {
        assert_eq!(short(42), "42");
        assert_eq!(short(-9_999), "-9999");
        assert_eq!(short(12_345), "12k");
        assert_eq!(short(-3_400_000), "-3m");
    }
}
