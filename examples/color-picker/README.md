# Color Picker

Pick a color from anywhere on screen with the browser's EyeDropper, copy it as HEX, RGB or HSL, and keep the last fifteen in sync storage, so the palette follows you to every computer you're signed in to. Browsers without an EyeDropper (Firefox today) get the system color picker instead.

```sh
cd examples/color-picker
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| Calling a web API oxichrome doesn't wrap with a small `#[wasm_bindgen] extern` block, and checking it exists first | `EyeDropper`, `has_eyedropper` in `src/lib.rs` |
| A fallback for browsers without it: an `<input type=color>` | the `has_eyedropper()` branch in `Popup` |
| `storage::sync` for data that should follow the user | `HISTORY`, `remember`, `clear` |
| Writing to the clipboard from the popup with the `ClipboardWrite` permission | `copy` |
| An action shortcut (Alt+Shift+C) in `#[extension]` | top of `src/lib.rs` |
| Plain Rust for the color math, tested with `cargo test` | `src/color.rs` |

## Try it

- Open the popup (or press Alt+Shift+C) and click **Pick from screen**, then click any pixel. In Firefox, click **Choose a color**.
- Click a HEX, RGB or HSL row to copy it.
- Click a swatch under **Recent** to bring it back; open the popup on another signed-in computer to see the same list.
