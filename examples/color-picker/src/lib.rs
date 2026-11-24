mod color;

use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::prelude::*;
use oxichrome::storage;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use color::Color;

#[extension(
    name = "Color Picker",
    description = "Pick a color from anywhere on screen, copy it as HEX, RGB or HSL, and keep a palette that follows you between computers.",
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
    type ColorSelection;

    #[wasm_bindgen(constructor, catch)]
    fn new() -> Result<EyeDropper, JsValue>;

    #[wasm_bindgen(method)]
    fn open(this: &EyeDropper) -> js_sys::Promise;

    #[wasm_bindgen(method, getter, js_name = sRGBHex)]
    fn srgb_hex(this: &ColorSelection) -> String;
}

fn has_eyedropper() -> bool {
    js_sys::Reflect::has(&js_sys::global(), &"EyeDropper".into()).unwrap_or(false)
}

async fn eyedropper() -> Option<Color> {
    let picked = JsFuture::from(EyeDropper::new().ok()?.open()).await.ok()?;
    Color::parse(&picked.unchecked_into::<ColorSelection>().srgb_hex())
}

async fn copy(text: &str) -> bool {
    let Some(window) = web_sys::window() else { return false };
    JsFuture::from(window.navigator().clipboard().write_text(text)).await.is_ok()
}

#[popup(style = ["base.css", "color-picker.css"])]
fn Popup() -> impl IntoView {
    let current = RwSignal::new(Color { r: 0x6e, g: 0x56, b: 0xcf });
    let history = RwSignal::new(Vec::<String>::new());
    let note = RwSignal::new(None::<String>);

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
            list.retain(|old| *old != color.hex());
            list.insert(0, color.hex());
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
        spawn_local(async move {
            if let Some(color) = eyedropper().await {
                remember(color);
            }
        })
    };
    let copy_text = move |text: String| {
        spawn_local(async move {
            note.set(Some(if copy(&text).await { format!("Copied {text}") } else { "Could not copy".into() }));
        })
    };
    let clear = move |_| {
        history.set(Vec::new());
        spawn_local(async move {
            let _ = storage::sync::remove(HISTORY).await;
        });
    };

    view! {
        <div class="popup picker">
            <div class="preview" style=move || format!("background: {}; color: {}", current.get().hex(), current.get().ink())>
                <span class="preview-hex">{move || current.get().hex()}</span>
            </div>
            <div class="body">
                {if has_eyedropper() {
                    view! { <button class="btn primary lg pick" on:click=pick>"Pick from screen"</button> }.into_any()
                } else {
                    view! {
                        <label class="btn lg pick">
                            <input
                                type="color"
                                prop:value=move || current.get().hex()
                                on:change=move |ev| if let Some(color) = Color::parse(&event_target_value(&ev)) { remember(color) }
                            />
                            "Choose a color"
                        </label>
                    }.into_any()
                }}
                <section class="card formats">
                    {move || {
                        let color = current.get();
                        [("HEX", color.hex()), ("RGB", color.rgb()), ("HSL", color.hsl())].map(|(label, text)| {
                            let copied = text.clone();
                            view! {
                                <button class="format" title="Copy" on:click=move |_| copy_text(copied.clone())>
                                    <span class="format-label">{label}</span>
                                    <span class="mono format-value">{text}</span>
                                </button>
                            }
                        }).collect_view()
                    }}
                </section>
                {move || note.get().map(|text| view! { <div class="notice">{text}</div> })}
                <section>
                    <div class="row-head">
                        <span class="section-title">"Recent"</span>
                        <Show when=move || history.with(|list| !list.is_empty())>
                            <button class="btn ghost small" on:click=clear>"Clear"</button>
                        </Show>
                    </div>
                    <div class="swatches">
                        {move || history.get().into_iter().filter_map(|hex| Color::parse(&hex)).map(|color| view! {
                            <button
                                class="swatch"
                                class:selected=move || current.get() == color
                                style=format!("background: {}", color.hex())
                                title=color.hex()
                                on:click=move |_| current.set(color)
                            ></button>
                        }).collect_view()}
                    </div>
                </section>
            </div>
        </div>
    }
}
