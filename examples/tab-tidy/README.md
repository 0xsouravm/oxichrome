# Tab Tidy

A tab manager: search every open tab, switch to it, pin, mute, move it to its own window or close it, and clear out duplicate tabs in one click. The same list lives in the popup and in the side panel (Firefox's sidebar). English and German.

```sh
cd examples/tab-tidy
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| Popup and side panel sharing Leptos components, with one stylesheet for both (`style = [...]`) | `src/lib.rs`, `src/ui.rs` |
| `tabs::query` into your own struct (with `mutedInfo`), `tabs::update`, `tabs::remove` | `src/tidy.rs` |
| `windows::current`, `windows::create(NewWindow::new().tab(id))`, `windows::update(... focused(true))` | `src/tidy.rs` |
| `side_panel::open_on_click` from the popup | `Popup` in `src/lib.rs` |
| A keyboard shortcut (`#[command]`), reading it back with `COMMAND.shortcut()` and opening the browser's shortcut settings | `close_duplicates`, `Footer` |
| A shortcut for the toolbar button (`action = { shortcut = ... }`) | `#[extension]` |
| Right-click menus on the toolbar button and on pages, in a submenu | `menu_*` in `src/lib.rs` |
| Badge text, colour and title from the background, kept current with `tabs::on_updated` and `tabs::on_removed` | `refresh_badge` |
| `storage` for the running total of closed tabs | `closed_total` |
| Every user-facing string through `t!`, with `en` and `de` | `static/_locales` |

Pinned and active tabs are never closed as duplicates, and two addresses that differ only after `#` count as the same page.

## Try it

1. Open a few tabs, including the same page twice. The badge shows how many duplicates there are.
2. Press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd> (or click the toolbar button), type part of a title and press <kbd>Enter</kbd> to jump to it.
3. Hover a row to pin it, move it to a new window or close it.
4. Press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>U</kbd> anywhere to close duplicates, or right-click the toolbar button.
5. Click the panel button in the popup's corner to keep the list open beside the page.
