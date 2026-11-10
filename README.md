# oxichrome

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![Manifest V3](https://img.shields.io/badge/chrome-Manifest%20V3-4285F4.svg)](https://developer.chrome.com/docs/extensions/develop/migrate/what-is-mv3)
[![Website](https://img.shields.io/badge/website-oxichrome.dev-ff6600.svg)](https://oxichrome.dev)

![oxichrome](https://raw.githubusercontent.com/0xsouravm/oxichrome/main/assets/og.png)

Write browser extensions entirely in Rust, compiled to WebAssembly. Chrome and Firefox.

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
cargo oxichrome build                  # Chrome/Edge/Brave
cargo oxichrome build --target firefox # Firefox
```

---

## What it does

- Five proc macros (`#[extension]`, `#[background]`, `#[on]`, `#[popup]`, `#[options_page]`) transform your Rust into wasm-bindgen exports
- Typed bindings to `chrome.storage`, `chrome.tabs`, `chrome.runtime`
- Leptos for reactive popup and options page UI
- `cargo oxichrome build` handles everything: compilation, wasm-bindgen, manifest generation, JS/HTML shim generation, static asset copying, optional wasm-opt
- **`--target firefox`** generates a Firefox-compatible Manifest V3 extension
- Separate `dist/chromium/` and `dist/firefox/` output directories

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
oxichrome = { version = "0.2" }
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
```

**Chrome/Edge:** Load `dist/chromium/` in `chrome://extensions` with "Load unpacked".

**Firefox:** Build with `--target firefox`, then load `dist/firefox/manifest.json` in `about:debugging#/runtime/this-firefox`.

## CLI commands

| Command | Description |
|---------|-------------|
| `cargo oxichrome build` | Build for Chromium (default) |
| `cargo oxichrome build --release` | Optimized release build |
| `cargo oxichrome build --target firefox` | Build for Firefox |
| `cargo oxichrome clean` | Remove the `dist/` directory |
| `cargo oxichrome new <name>` | Scaffold a new extension project |

## Project structure

```
oxichrome/
├── oxichrome/            re-export crate (what users depend on)
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
    permissions = ["storage", "tabs"],
    host_permissions = ["https://example.org/*", "*://*.example.com/*"], // optional
    gecko_id = "my-extension@example.com" // optional, Firefox only
)]
struct MyExtension;
```

`host_permissions` lists the [match patterns](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Match_patterns) your extension can access, and is written to `host_permissions` in the manifest for both Chromium and Firefox. The key is left out of the manifest when the list is empty. Malformed patterns are compile errors.

`gecko_id` sets the add-on ID in the Firefox manifest (`browser_specific_settings.gecko.id`). Chromium builds ignore it. If you leave it out, the ID is generated from the extension name, for example `my-extension@oxichrome.dev`.

It accepts either form [allowed by Firefox](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/manifest.json/browser_specific_settings#id):

- Email-like, up to 80 characters: `my-extension@example.com`
- GUID in braces: `{daf44bf7-a45e-4450-979c-91cf07434c3d}`

An invalid ID is a compile error, for both Firefox and Chromium builds.

### `#[oxichrome::background]`

Marks an async function as the background service worker entry point. It runs every time the background starts: on install, on browser startup, and whenever the service worker or event page wakes up for an event.

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

Each argument is converted to the type you declare. `JsValue` always works, and some events have typed arguments, such as `alarms::Alarm` for `alarms::on_alarm`. A type the event doesn't provide at that position is a compile error. Your own types can implement `event::FromEventArg` plus `event::EventArg<event::events::AlarmsOnAlarm, 1>` (event marker, argument position) to opt in. If an argument can't be converted, that handler doesn't run, and the error appears in the background console as an unhandled promise rejection.

```rust
#[oxichrome::on(alarms::on_alarm)]
async fn on_alarm(alarm: oxichrome::alarms::Alarm) {
    oxichrome::log!("{} fired", alarm.name);
}
```

Listeners are registered synchronously when the background script starts, and events that arrive before WASM is ready are delivered once it is. This covers `on_installed` and events that wake a stopped service worker.

| Event | Typed arguments |
|---|---|
| `alarms::on_alarm` | `alarms::Alarm` |
| `notifications::on_clicked` | `id: String` |
| `notifications::on_closed` | `id: String, by_user: Option<bool>` |
| `notifications::on_button_clicked` | `id: String, index: u32` |

`#[on(runtime::on_message)]` handlers receive `(message, sender, send_response)`. Call `send_response` to reply, including after an `.await`. If no handler replies, the sender gets an empty response (`null` in Chrome, `undefined` in Firefox) once every handler has finished.

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

// Storage areas (the functions above use `local`)
oxichrome::storage::sync::set("key", &42).await?;
oxichrome::storage::session::set("key", &42).await?;

// Tabs
let tabs: Vec<Tab> = oxichrome::tabs::query(&query).await?;
let tab: Tab = oxichrome::tabs::create(&props).await?;
let tab: Tab = oxichrome::tabs::get(tab_id).await?;
let tab: Tab = oxichrome::tabs::update(tab_id, &props).await?;
oxichrome::tabs::remove(tab_id).await?;
oxichrome::tabs::send_message(tab_id, &msg).await?;

// Runtime
let url = oxichrome::runtime::get_url("icon.png");
oxichrome::runtime::send_message(&msg).await?;

// Action
oxichrome::action::set_badge_text("3", None).await?;
oxichrome::action::set_badge_background_color("#ff6600", None).await?;
oxichrome::action::set_title("My Extension", Some(tab_id)).await?;
oxichrome::action::set_icon("icon.png", None).await?;

// Alarms (requires the "alarms" permission)
use std::time::Duration;
use oxichrome::alarms::{self, AlarmOptions};
alarms::create("once", AlarmOptions::after(Duration::from_secs(90))).await?;
alarms::create("at", AlarmOptions::at(time_ms)).await?;
alarms::create("tick", AlarmOptions::after(Duration::from_secs(60)).then_every(Duration::from_secs(300))).await?;
let alarm = alarms::get("tick").await?;
let all = alarms::get_all().await?;
let removed: bool = alarms::clear("tick").await?;
alarms::clear_all().await?;
let created: bool = alarms::create_if_missing("tick", AlarmOptions::after(Duration::from_secs(60))).await?;

// Notifications (requires the "notifications" permission)
use oxichrome::notifications::{self, ListItem, Notification, Priority};
let id: String = notifications::create(&Notification::basic("icon.png", "Done", "Upload finished")).await?;
notifications::create_with_id("job", &Notification::progress("icon.png", "Uploading", "photo.jpg", 0)).await?;
let updated: bool = notifications::update("job", &Notification::progress("icon.png", "Uploading", "photo.jpg", 60)).await?;
let cleared: bool = notifications::clear("job").await?;
let open: Vec<String> = notifications::get_all().await?;
Notification::image("icon.png", "New photo", "From Sam", "photo.png");
Notification::list("icon.png", "3 new emails", "", [ListItem::new("Sam", "Lunch?")]);
Notification::basic("icon.png", "Reminder", "Standup")
    .context_message("Calendar")
    .event_time(time_ms)
    .priority(Priority::High)
    .button("Snooze")
    .button("Dismiss")
    .require_interaction()
    .silent();
```

`chrome.action` needs `action` in the manifest. oxichrome adds it when your extension has a `#[popup]` or an `#[on(action::on_clicked)]` handler. The click event doesn't fire while a popup is set.

An alarm starts either `after` a `Duration` or `at` a time in milliseconds since the epoch (for example `js_sys::Date::now() as u64 + 60_000` with `js-sys` in your dependencies), and `then_every` makes it repeat. A zero delay or a time in the past fires right away; Chrome waits at least 1 second, Firefox doesn't. A period under 1 second, or a delay or time that ends past the latest date a browser can hold (`alarms::MAX_TIME_MS`, in the year 275760), returns `Err` without reaching the browser. `get` and `get_all` return the same types: `scheduled_time` in milliseconds and `period` as a `Duration`.

Alarm names can be up to 1024 bytes of UTF-8, and an extension can have at most 500 alarms. These are Chrome's limits, and oxichrome enforces them in Firefox too. Going over them returns `InvalidArgument` or `LimitExceeded`. At the limit, replacing an existing alarm is rejected as well, because Chrome does the same.

Browsers clear an extension's alarms when it is reloaded or updated, and on browser restart. Firefox also clears them when the extension is disabled and enabled again. To keep an alarm, call `create_if_missing` from your `#[background]` function: it runs on every start, recreates the alarm when it's gone, and leaves an existing one alone. Calling `create` there instead would reset the schedule every time the service worker wakes up.

Every notification needs an icon: Chrome refuses to show one without it. The icon and the `image` of an image notification are paths in your extension (`"icon.png"` or `"/icons/48.png"`), `data:` URLs, or full URLs. oxichrome loads each one before showing the notification, and a file that is missing or isn't a decodable image returns `InvalidArgument` in both browsers. Without that check, Chrome fails and Firefox silently shows no icon.

`create` picks a unique id for you. `create_with_id` shows the notification under your id and replaces any open notification with the same id. `update` changes an open notification and returns `false` (creating nothing) when the id isn't open. Ids can be up to 500 bytes of UTF-8 and can't be empty or contain U+001F. Progress goes from 0 to 100, a list needs at least one item, and there can be at most 2 buttons. Anything outside these limits returns `InvalidArgument`. Chrome would otherwise accept it and then show nothing.

Firefox shows basic notifications only: an image, list items, progress, buttons, `require_interaction` and `silent` are left out there, and `on_button_clicked` never fires. Firefox has no native `update`, so oxichrome replaces the notification, which pops it up again. Priority has no visible effect in Firefox. Chrome on Windows, macOS and Linux never shows notifications below the default priority, so `Priority` only offers `Default`, `High` and `Max`.

`on_closed` fires when the user dismisses a notification or the system closes it, never because you called `clear` or replaced it. `by_user` is `Some` in Chrome and `None` in Firefox, which doesn't report it. Whether a click also closes the notification depends on the system: when it does, `on_closed` follows `on_clicked`. On macOS, in both browsers, a clicked notification stays open and `get_all` keeps listing it until it is closed. Notifications don't exist in content scripts.

The `action` setters take a `tab_id`: `Some(id)` applies to that tab, `None` changes the default for all tabs. `set_badge_text("", None)` clears the badge, and `set_popup("", None)` removes the popup so clicks fire `on_clicked`.

## Build output

```
dist/
├── chromium/                  # default target
│   ├── manifest.json
│   ├── background.js
│   ├── popup.html / popup.js
│   ├── options.html / options.js
│   └── wasm/
│       ├── crate_name.js
│       └── crate_name_bg.wasm
└── firefox/                   # --target firefox
    ├── manifest.json          # background scripts, gecko ID (custom via gecko_id)
    └── ...                    # same files as chromium/
```

## License

MIT
