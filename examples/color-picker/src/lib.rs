use leptos::prelude::*;
use oxichrome::prelude::*;
use wasm_bindgen::prelude::*;

#[oxichrome::extension(
    name = "Color Picker",
    version = "0.1.0",
    description = "Pick colors from anywhere on screen using the EyeDropper API",
    permissions = ["storage"]
)]
struct ColorPickerExtension;

// EyeDropper Web API bindings (Chrome 95+)

#[wasm_bindgen]
extern "C" {
    type EyeDropper;

    #[wasm_bindgen(constructor)]
    fn new() -> EyeDropper;

    #[wasm_bindgen(method)]
    fn open(this: &EyeDropper) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["navigator", "clipboard"], js_name = "writeText")]
    fn clipboard_write_text(text: &str) -> js_sys::Promise;
}

#[oxichrome::background]
async fn start() {
    oxichrome::log!("Color Picker background service worker started!");
}

const MAX_HISTORY: usize = 20;
const STORAGE_KEY: &str = "color_history";

#[oxichrome::popup]
fn Popup() -> impl IntoView {
    let current_color = RwSignal::new(String::from("#000000"));
    let history = RwSignal::new(Vec::<String>::new());
    let picking = RwSignal::new(false);
    let error_msg = RwSignal::new(Option::<String>::None);
    let copied = RwSignal::new(false);

    Effect::new(move || {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(Some(colors)) =
                oxichrome::storage::get::<Vec<String>>(STORAGE_KEY).await
            {
                if let Some(first) = colors.first() {
                    current_color.set(first.clone());
                }
                history.set(colors);
            }
        });
    });

    let pick_color = move |_| {
        picking.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            let dropper = EyeDropper::new();
            match wasm_bindgen_futures::JsFuture::from(dropper.open()).await {
                Ok(result) => {
                    let hex = js_sys::Reflect::get(&result, &JsValue::from_str("sRGBHex"))
                        .unwrap_or(JsValue::from_str("#000000"))
                        .as_string()
                        .unwrap_or_else(|| "#000000".into());

                    current_color.set(hex.clone());

                    history.update(|h| {
                        h.retain(|c| c != &hex);
                        h.insert(0, hex);
                        h.truncate(MAX_HISTORY);
                    });

                    let colors = history.get_untracked();
                    let _ = oxichrome::storage::set(STORAGE_KEY, &colors).await;
                }
                Err(_) => {
                    error_msg.set(Some("Cancelled or EyeDropper unavailable".into()));
                }
            }
            picking.set(false);
        });
    };

    let copy_hex = move |_| {
        let hex = current_color.get_untracked();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = wasm_bindgen_futures::JsFuture::from(clipboard_write_text(&hex)).await;
            copied.set(true);
            let cb = Closure::once(move || copied.set(false));
            let global = js_sys::global();
            let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
                .unwrap()
                .unchecked_into::<js_sys::Function>();
            let _ = set_timeout.call2(
                &JsValue::NULL,
                cb.as_ref(),
                &JsValue::from_f64(1500.0),
            );
            cb.forget();
        });
    };

    let on_swatch_click = move |color: String| {
        current_color.set(color);
    };

    view! {
        <style>
            "* { margin: 0; padding: 0; box-sizing: border-box; }
            body { width: 280px; font-family: -apple-system, BlinkMacSystemFont, sans-serif;
                   background: #0f0f0f; color: #e0e0e0; padding: 1.25rem; }
            h1 { font-size: 0.65rem; font-weight: 500; letter-spacing: 0.15em;
                 text-transform: uppercase; color: #888; margin-bottom: 1rem; text-align: center; }
            .preview { width: 100%; height: 100px; border-radius: 12px; border: 1.5px solid #333;
                       margin-bottom: 0.75rem; transition: background-color 0.2s ease; }
            .hex { font-size: 1.5rem; font-weight: 300; font-family: 'SF Mono', 'Cascadia Code',
                   'Fira Code', monospace; text-align: center; color: #fff;
                   margin-bottom: 1rem; letter-spacing: 0.05em; cursor: pointer;
                   transition: color 0.15s ease; user-select: none; }
            .hex:hover { color: #aaa; }
            .copied { font-size: 0.6rem; color: #50e070; text-align: center;
                      margin-top: -0.7rem; margin-bottom: 0.7rem; letter-spacing: 0.1em;
                      text-transform: uppercase; }
            .pick-btn { display: block; width: 100%; padding: 0.7rem; border-radius: 22px;
                        border: 1.5px solid #333; background: transparent; color: #e0e0e0;
                        font-size: 0.8rem; letter-spacing: 0.08em; text-transform: uppercase;
                        cursor: pointer; transition: all 0.15s ease; }
            .pick-btn:hover { border-color: #e0e0e0; color: #fff; }
            .pick-btn:active { background: #e0e0e0; color: #0f0f0f; }
            .pick-btn:disabled { opacity: 0.4; cursor: not-allowed; }
            .error { font-size: 0.65rem; color: #e05050; text-align: center;
                     margin-top: 0.5rem; }
            .divider { height: 1px; background: #222; margin: 1rem 0; }
            .history-label { font-size: 0.6rem; font-weight: 500; letter-spacing: 0.15em;
                             text-transform: uppercase; color: #666; margin-bottom: 0.6rem; }
            .history-grid { display: grid; grid-template-columns: repeat(5, 1fr);
                            gap: 6px; }
            .swatch { width: 100%; aspect-ratio: 1; border-radius: 8px; border: 1.5px solid #333;
                      cursor: pointer; transition: all 0.15s ease; }
            .swatch:hover { border-color: #e0e0e0; transform: scale(1.1); }
            .empty { font-size: 0.65rem; color: #555; text-align: center; padding: 0.75rem 0; }"
        </style>
        <h1>"Color Picker"</h1>
        <div
            class="preview"
            style:background-color=move || current_color.get()
        ></div>
        <div class="hex" on:click=copy_hex title="Click to copy">
            {move || current_color.get()}
        </div>
        {move || copied.get().then(|| view! { <div class="copied">"Copied!"</div> })}
        <button
            class="pick-btn"
            on:click=pick_color
            disabled=move || picking.get()
        >
            {move || if picking.get() { "Picking..." } else { "Pick Color" }}
        </button>
        {move || error_msg.get().map(|msg| view! { <div class="error">{msg}</div> })}
        <div class="divider"></div>
        <div class="history-label">"History"</div>
        {move || {
            let colors = history.get();
            if colors.is_empty() {
                view! { <div class="empty">"No colors picked yet"</div> }.into_any()
            } else {
                view! {
                    <div class="history-grid">
                        {colors
                            .into_iter()
                            .map(|color| {
                                let bg = color.clone();
                                let click_color = color.clone();
                                view! {
                                    <div
                                        class="swatch"
                                        style:background-color=bg
                                        title=color
                                        on:click=move |_| on_swatch_click(click_color.clone())
                                    ></div>
                                }
                            })
                            .collect_view()}
                    </div>
                }
                .into_any()
            }
        }}
    }
}
