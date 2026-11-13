# Color Picker

Pick a color from anywhere on screen with the browser's EyeDropper, copy it as HEX, RGB or HSL, and keep the last fifteen in a palette stored in sync storage, so it follows you to every computer you're signed in to. In browsers without an EyeDropper (Firefox today) the button becomes the system color picker.

```sh
cd examples/color-picker
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| Calling a web API that `web-sys` doesn't cover yet with a small `#[wasm_bindgen] extern` block, and checking it exists first | `EyeDropper`, `has_eyedropper` in `src/lib.rs` |
| A fallback when a browser lacks a feature, instead of a broken button | the `can_pick` branch in `Popup` |
| `storage::sync` for data that should follow the user | `HISTORY` |
| Writing to the clipboard from the popup with the `ClipboardWrite` permission | `copy` |
| Plain Rust for the logic (parsing, HSL, contrast, color names), tested with `cargo test` | `src/color.rs` |
| An action shortcut (Alt+Shift+C) declared in `#[extension]` | top of `src/lib.rs` |
