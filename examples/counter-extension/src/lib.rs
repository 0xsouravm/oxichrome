use leptos::prelude::*;
use oxichrome::prelude::*;

#[oxichrome::extension(
    name = "Counter Extension",
    version = "0.1.0",
    description = "A simple counter stored in chrome.storage.local",
    permissions = ["storage"]
)]
struct CounterExtension;

#[oxichrome::background]
async fn start() {
    oxichrome::log!("Counter Extension background service worker started!");
}

#[oxichrome::on(runtime::on_installed)]
async fn handle_install(details: oxichrome::__private::wasm_bindgen::JsValue) {
    oxichrome::log!("Counter Extension installed: {:?}", details);

    if let Err(e) = oxichrome::storage::set("counter", &0i32).await {
        oxichrome::log!("Failed to initialize counter: {}", e);
    } else {
        oxichrome::log!("Counter initialized to 0.");
    }
}

#[oxichrome::popup]
fn Popup() -> impl IntoView {
    let count = RwSignal::new(0i32);

    Effect::new(move || {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(Some(val)) = oxichrome::storage::get::<i32>("counter").await {
                count.set(val);
            }
        });
    });

    let update = move |delta: i32| {
        count.update(|c| *c += delta);
        let val = count.get_untracked();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = oxichrome::storage::set("counter", &val).await;
        });
    };

    let increment = move |_| update(1);
    let decrement = move |_| update(-1);
    let reset = move |_| {
        count.set(0);
        wasm_bindgen_futures::spawn_local(async move {
            let _ = oxichrome::storage::set("counter", &0i32).await;
        });
    };

    view! {
        <style>
            "* { margin: 0; padding: 0; box-sizing: border-box; }
            body { width: 240px; font-family: -apple-system, BlinkMacSystemFont, sans-serif;
                   background: #0f0f0f; color: #e0e0e0; padding: 1.25rem; }
            h1 { font-size: 0.65rem; font-weight: 500; letter-spacing: 0.15em;
                 text-transform: uppercase; color: #888; margin-bottom: 1rem; text-align: center; }
            .count { font-size: 3.5rem; font-weight: 200; font-variant-numeric: tabular-nums;
                     line-height: 1; text-align: center; color: #fff; margin-bottom: 1.25rem; }
            .controls { display: flex; gap: 0.5rem; justify-content: center; }
            button { width: 44px; height: 44px; border-radius: 50%; border: 1.5px solid #333;
                     background: transparent; color: #e0e0e0; font-size: 1.25rem; cursor: pointer;
                     transition: all 0.15s ease; display: flex; align-items: center;
                     justify-content: center; }
            button:hover { border-color: #e0e0e0; color: #fff; }
            button:active { background: #e0e0e0; color: #0f0f0f; }
            .reset { width: auto; border-radius: 22px; padding: 0 1rem; font-size: 0.7rem;
                     letter-spacing: 0.08em; text-transform: uppercase; }"
        </style>
        <h1>"Counter"</h1>
        <div class="count">{move || count.get()}</div>
        <div class="controls">
            <button on:click=decrement>"\u{2212}"</button>
            <button on:click=increment>"+"</button>
            <button class="reset" on:click=reset>"Reset"</button>
        </div>
    }
}

#[oxichrome::options_page]
fn Options() -> impl IntoView {
    let count = RwSignal::new(0i32);

    Effect::new(move || {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(Some(val)) = oxichrome::storage::get::<i32>("counter").await {
                count.set(val);
            }
        });
    });

    let update = move |delta: i32| {
        count.update(|c| *c += delta);
        let val = count.get_untracked();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = oxichrome::storage::set("counter", &val).await;
        });
    };

    let increment = move |_| update(1);
    let decrement = move |_| update(-1);
    let reset = move |_| {
        count.set(0);
        wasm_bindgen_futures::spawn_local(async move {
            let _ = oxichrome::storage::set("counter", &0i32).await;
        });
    };

    view! {
        <style>
            "* { margin: 0; padding: 0; box-sizing: border-box; }
            body { font-family: -apple-system, BlinkMacSystemFont, sans-serif;
                   background: #0f0f0f; color: #e0e0e0; display: flex;
                   justify-content: center; align-items: center; min-height: 100vh; }
            .container { text-align: center; padding: 3rem; }
            h1 { font-size: 1rem; font-weight: 500; letter-spacing: 0.15em;
                 text-transform: uppercase; color: #888; margin-bottom: 2rem; }
            .count { font-size: 8rem; font-weight: 200; font-variant-numeric: tabular-nums;
                     line-height: 1; margin-bottom: 2.5rem; color: #fff; }
            .controls { display: flex; gap: 1rem; justify-content: center; }
            button { width: 56px; height: 56px; border-radius: 50%; border: 1.5px solid #333;
                     background: transparent; color: #e0e0e0; font-size: 1.5rem; cursor: pointer;
                     transition: all 0.15s ease; display: flex; align-items: center;
                     justify-content: center; }
            button:hover { border-color: #e0e0e0; color: #fff; }
            button:active { background: #e0e0e0; color: #0f0f0f; }
            .reset { width: auto; border-radius: 28px; padding: 0 1.5rem; font-size: 0.8rem;
                     letter-spacing: 0.08em; text-transform: uppercase; }"
        </style>
        <div class="container">
            <h1>"Counter Extension"</h1>
            <div class="count">{move || count.get()}</div>
            <div class="controls">
                <button on:click=decrement>"\u{2212}"</button>
                <button on:click=increment>"+"</button>
                <button class="reset" on:click=reset>"Reset"</button>
            </div>
        </div>
    }
}
