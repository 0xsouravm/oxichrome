mod color;

use std::result::Result as StdResult;
use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::prelude::*;
use oxichrome::storage;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use color::Color;

#[extension(
    name = "Color Picker",
    description = "Pick a color from anywhere on screen, copy it as HEX, RGB or HSL, and keep the last few in a palette that follows you between computers.",
    version = "0.3.0",
    permissions = [Storage, ClipboardWrite],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = "Pick a color", shortcut = "Alt+Shift+C" },
    gecko_id = "color-picker@oxichrome.dev"
)]
pub struct ColorPicker;

const HISTORY: &str = "history";
const KEEP: usize = 15;

#[wasm_bindgen]
extern "C" {
    type EyeDropper;

    #[wasm_bindgen(constructor, catch)]
    fn new() -> StdResult<EyeDropper, JsValue>;

    #[wasm_bindgen(method)]
    fn open(this: &EyeDropper) -> js_sys::Promise;
}

fn has_eyedropper() -> bool {
    js_sys::Reflect::has(&js_sys::global(), &JsValue::from_str("EyeDropper")).unwrap_or(false)
}

async fn eyedropper() -> Option<Color> {
    let dropper = EyeDropper::new().ok()?;
    let picked = JsFuture::from(dropper.open()).await.ok()?;
    let hex = js_sys::Reflect::get(&picked, &JsValue::from_str("sRGBHex")).ok()?.as_string()?;
    Color::parse(&hex)
}

async fn copy(text: &str) -> bool {
    let Some(window) = web_sys::window() else { return false };
    JsFuture::from(window.navigator().clipboard().write_text(text)).await.is_ok()
}

#[popup(style = ["base.css", "color-picker.css"])]
fn Popup() -> impl IntoView {
    let current = RwSignal::new(Color::parse("#6e56cf").unwrap_or_default());
    let history = RwSignal::new(Vec::<String>::new());
    let picking = RwSignal::new(false);
    let copied = RwSignal::new(None::<&'static str>);
    let note = RwSignal::new(None::<String>);
    let can_pick = has_eyedropper();

    spawn_local(async move {
        let saved: Vec<String> = storage::sync::get(HISTORY).await.ok().flatten().unwrap_or_default();
        if let Some(first) = saved.first().and_then(|hex| Color::parse(hex)) {
            current.set(first);
        }
        history.set(saved);
    });

    let remember = move |color: Color| {
        current.set(color);
        history.update(|list| {
            let hex = color.hex();
            list.retain(|old| *old != hex);
            list.insert(0, hex);
            list.truncate(KEEP);
        });
        let list = history.get_untracked();
        spawn_local(async move {
            if let Err(err) = storage::sync::set(HISTORY, &list).await {
                note.set(Some(err.to_string()));
            }
        });
    };
    let pick = move |_| {
        picking.set(true);
        note.set(None);
        spawn_local(async move {
            match eyedropper().await {
                Some(color) => remember(color),
                None => note.set(Some("Nothing picked. Press Esc to cancel, or click anywhere on screen to pick.".into())),
            }
            picking.set(false);
        });
    };
    let copy_as = move |label: &'static str, text: String| {
        spawn_local(async move {
            if copy(&text).await {
                copied.set(Some(label));
                set_timeout(move || if copied.get_untracked() == Some(label) { copied.set(None) }, Duration::from_millis(1400));
            } else {
                note.set(Some("The clipboard didn't take it. Click the popup first, then try again.".into()));
            }
        });
    };
    let clear = move |_| {
        history.set(Vec::new());
        spawn_local(async move {
            let _ = storage::sync::remove(HISTORY).await;
        });
    };

    view! {
        <div class="popup picker" style=move || format!("--picked: {}; --picked-ink: {}", current.get().hex(), current.get().ink())>
            <div class="preview">
                <span class="preview-hex">{move || current.get().hex()}</span>
                <span class="preview-name">{move || current.get().describe()}</span>
            </div>
            <div class="body">
                {if can_pick {
                    view! {
                        <button class="btn primary lg pick" on:click=pick disabled=move || picking.get()>
                            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m14 7 3 3"/><path d="M5 19l1-4 9-9a2.1 2.1 0 0 1 3 3l-9 9z"/><path d="M5 19h0"/></svg>
                            {move || if picking.get() { "Click anywhere to pick…" } else { "Pick from screen" }}
                        </button>
                    }.into_any()
                } else {
                    view! {
                        <label class="btn lg pick fallback">
                            <input
                                type="color"
                                prop:value=move || current.get().hex()
                                on:change=move |ev| if let Some(color) = Color::parse(&event_target_value(&ev)) { remember(color) }
                            />
                            "Choose a color"
                        </label>
                        <p class="hint-line">"This browser has no eyedropper yet, so the system color picker opens instead."</p>
                    }.into_any()
                }}
                {move || note.get().map(|text| view! { <div class="notice">{text}</div> })}
                <section class="card formats">
                    {move || {
                        let color = current.get();
                        [("HEX", color.hex()), ("RGB", color.rgb()), ("HSL", color.hsl())].into_iter().map(|(label, text)| {
                            let value = text.clone();
                            view! {
                                <button class="format" title=format!("Copy {text}") on:click=move |_| copy_as(label, value.clone())>
                                    <span class="format-label">{label}</span>
                                    <span class="mono format-value">{text.clone()}</span>
                                    <span class="format-copy">{move || if copied.get() == Some(label) { "Copied" } else { "Copy" }}</span>
                                </button>
                            }
                        }).collect_view()
                    }}
                </section>
                <section>
                    <div class="row-head">
                        <span class="section-title">"Recent"</span>
                        <Show when=move || history.with(|list| !list.is_empty())>
                            <button class="btn ghost small" on:click=clear>"Clear"</button>
                        </Show>
                    </div>
                    {move || {
                        let list = history.get();
                        if list.is_empty() {
                            return view! { <div class="empty small">"Colors you pick land here, on every computer you're signed in to."</div> }.into_any();
                        }
                        view! {
                            <div class="swatches">
                                {list.into_iter().filter_map(|hex| Color::parse(&hex)).map(|color| view! {
                                    <button
                                        class="swatch"
                                        class:selected=move || current.get() == color
                                        style=format!("background: {}", color.hex())
                                        title=color.hex()
                                        aria-label=format!("Use {}", color.hex())
                                        on:click=move |_| current.set(color)
                                    ></button>
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </section>
            </div>
        </div>
    }
}
