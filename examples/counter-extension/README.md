# Counter

The smallest useful oxichrome extension, and the one to read first. A number lives in extension storage. The popup changes it, the settings page picks the step and whether the number shows on the toolbar, and the background service worker puts the badge back when the browser starts.

```sh
cd examples/counter-extension
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| `#[extension]` with icons, an action and a Firefox id | top of `src/lib.rs` |
| `#[background]` that runs when the service worker starts | `start` |
| `#[popup]` and `#[options_page]` written as Leptos components, with stylesheets from `static/` | `Popup`, `Options` |
| `storage::get` and `storage::set` with a serde struct and `#[serde(default)]`, so old saved data still loads after you add a field | `Count`, `load`, `save` |
| `action::set_badge_text` and `set_badge_background_color` | `save` |

Everything else in `examples/` builds on these pieces.
