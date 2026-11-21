# Tab Tidy

A tab manager: search every open tab and jump to it, close duplicate tabs, group tabs by site, put a tab to sleep to free its memory, and bring back recently closed tabs and windows. The same page lives in the popup and in the side panel (Firefox's sidebar). English and German. All of it works in Chrome and Firefox; a browser without tab groups shows the error next to the button instead.

```sh
cd examples/tab-tidy
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| Typed tabs: `tabs::query` into `Vec<tabs::Tab>`, `tabs::update`, `tabs::remove` | `src/tidy.rs` |
| Typed events: `tabs::on_updated` with the `Tab`, `tabs::on_removed` with `tabs::Removed` | `tab_updated`, `tab_removed` in `src/lib.rs` |
| Tab groups: `tabs::group`, then `tab_groups::update` for the title and colour | `group_by_site` |
| `tabs::discard` to free a background tab's memory | `tab_row` |
| Sessions: `sessions::recently_closed`, `sessions::restore` and `#[on(sessions::on_changed)]` | `closed_row`, `sessions_changed` |
| Typed messages from the background to open pages (`runtime::send_message`, `runtime::listen`) so the lists stay current | `Notice`, `tidy_page` |
| `windows::create` (empty, or moving a tab), `windows::update(... focused(true))` | `src/tidy.rs` |
| Popup and side panel sharing one Leptos view, `side_panel::open_on_click` | `Popup`, `Panel` |
| A keyboard shortcut (`#[command]`) and one for the toolbar button (`action = { shortcut = ... }`) | `close_duplicates`, `#[extension]` |
| Right-click menus on the toolbar button and on pages, in a submenu | `menu_*` |
| Badge text with the number of duplicate tabs | `refresh_badge` |
| Every user-facing string through `t!`, with `en` and `de` | `static/_locales` |

Pinned and active tabs are never closed as duplicates, and two addresses that differ only after `#` count as the same page. Grouping skips pinned tabs and tabs already in a group, and only groups a site with two or more tabs in the same window.

## Try it

1. Open a few tabs, including the same page twice. The badge shows how many duplicates there are.
2. Press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd>, type part of a title and press <kbd>Enter</kbd> to jump to it.
3. Click "Group tabs by site", or right-click a page and pick it from the Tab Tidy submenu.
4. Hover a background tab and click "Sleep". It shows "asleep" and loads again when you select it.
5. Close a tab, then click "Restore" under "Recently closed".
6. Press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>U</kbd> anywhere to close duplicates.
