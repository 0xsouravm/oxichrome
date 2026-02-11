# oxichrome

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![Manifest V3](https://img.shields.io/badge/chrome-Manifest%20V3-4285F4.svg)](https://developer.chrome.com/docs/extensions/develop/migrate/what-is-mv3)

Write Chrome extensions entirely in Rust, compiled to WebAssembly.

---

```rust
use leptos::prelude::*;
use oxichrome::prelude::*;

#[oxichrome::extension(
    name = "My Extension",
    version = "1.0.0",
    permissions = ["storage"]
)]
struct Extension;

#[oxichrome::background]
async fn start() {
    oxichrome::log!("Running!");
}

#[oxichrome::popup]
fn Popup() -> impl IntoView {
    view! { <p>"Hello from Rust."</p> }
}
```

```sh
cargo oxichrome build
# Load dist/ as an unpacked extension in Chrome.
```

---

## What it does

- Five proc macros (`#[extension]`, `#[background]`, `#[on]`, `#[popup]`, `#[options_page]`) transform your Rust into wasm-bindgen exports
- Typed bindings to `chrome.storage`, `chrome.tabs`, `chrome.runtime`
- Leptos for reactive popup and options page UI
- `cargo oxichrome build` handles everything: compilation, wasm-bindgen, manifest generation, JS/HTML shim generation, static asset copying, optional wasm-opt

Zero JavaScript written by hand.

## Install

```sh
cargo install cargo-oxichrome
```

Requires:
- Rust (stable)
- `wasm32-unknown-unknown` target (auto-installed on first build)
- `wasm-bindgen-cli` (auto-installed with version matching on first build)

## Quick start

```sh
cargo oxichrome new my-extension
cd my-extension
cargo oxichrome build
```

`cargo oxichrome new` scaffolds a project with the `oxichrome` crate already in `[dependencies]`:

```toml
[dependencies]
oxichrome = { version = "0.1" }
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
```

Load `dist/` in `chrome://extensions` with "Load unpacked".

## Project structure

```
oxichrome/
├── oxichrome/            re export crate (what users depend on)
├── oxichrome-core/       runtime: Chrome API bindings, error types, logging
├── oxichrome-macros/     proc macros
├── oxichrome-build/      source parsing, manifest/shim generation
├── oxichrome-cli/        the cargo oxichrome command
└── examples/
    ├── counter-extension/
    └── color-picker/
```

## Proc macros

### `#[oxichrome::extension(...)]`

Defines extension metadata. Applied to a struct.

```rust
#[oxichrome::extension(
    name = "My Extension",
    version = "1.0.0",
    description = "Optional description",
    permissions = ["storage", "tabs"]
)]
struct MyExtension;
```

### `#[oxichrome::background]`

Marks an async function as the background service worker entry point.

```rust
#[oxichrome::background]
async fn start() {
    oxichrome::log!("Started.");
}
```

### `#[oxichrome::on(namespace::event)]`

Registers an async function as a Chrome event listener.

```rust
#[oxichrome::on(runtime::on_installed)]
async fn on_install(details: oxichrome::__private::wasm_bindgen::JsValue) {
    oxichrome::log!("Installed: {:?}", details);
}
```

### `#[oxichrome::popup]`

Marks a Leptos component as the popup UI.

```rust
#[oxichrome::popup]
fn Popup() -> impl IntoView {
    view! { <p>"Hello."</p> }
}
```

### `#[oxichrome::options_page]`

Marks a Leptos component as the options page UI.

```rust
#[oxichrome::options_page]
fn Options() -> impl IntoView {
    view! { <h1>"Settings"</h1> }
}
```

## Chrome APIs

```rust
// Storage
let val: Option<i32> = oxichrome::storage::get("key").await?;
oxichrome::storage::set("key", &42).await?;
oxichrome::storage::remove("key").await?;

// Tabs
let tabs: Vec<Tab> = oxichrome::tabs::query(&query).await?;
let tab: Tab = oxichrome::tabs::create(&props).await?;
oxichrome::tabs::send_message(tab_id, &msg).await?;

// Runtime
let url = oxichrome::runtime::get_url("icon.png");
oxichrome::runtime::send_message(&msg).await?;
```

## Build output

```
dist/
├── manifest.json
├── background.js
├── popup.html
├── popup.js
├── options.html
├── options.js
└── wasm/
    ├── crate_name.js
    └── crate_name_bg.wasm
```

## License

MIT
