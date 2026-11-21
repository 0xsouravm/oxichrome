# Page Lens

Look at a page before you read it: reading time, word and link counts, and its outline, with a click to jump to any heading. Copy a Markdown link, save the outline as a file, and follow how far you've read in the side panel. The developer tools get a Page Lens panel with the same outline, and on Wikipedia and MDN a small reading-time pill appears in the corner.

```sh
cd examples/page-lens
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| On-demand content scripts that take an argument and return a value: `inspect` returns a `PageInfo` struct, `scroll_to` takes a heading index | `src/page.rs` |
| A content script that starts by itself on matching pages, with its own stylesheet (`css = ["pill.css"]`) | `reading_pill` in `src/page.rs` |
| Typed messaging: the pill sends `Reading` with `runtime::request`, the background replies with an `Estimate` from its `#[on(runtime::on_message)]` handler and sets a badge on that tab | `reading` in `src/lib.rs` |
| A typed port: the side panel opens `tabs::connect`, the page accepts it with `runtime::on_connect` and streams `Progress` as you scroll, closing it on `pagehide` (Firefox keeps the ports of a page in its back/forward cache open) | `follow` in `src/page.rs`, `src/panel.rs` |
| An offscreen document for the clipboard, used by the popup and the menu | `copy` in `src/lib.rs` |
| `ActiveTab`: the popup and the menu read the current page with no host permission | `#[extension]` in `src/lib.rs` |
| An optional host permission asked for on click, with `permissions::request_on_click` | `src/panel.rs` |
| `downloads::save` for a file the extension made | `save_outline` in `src/popup.rs` |
| A context menu on pages and links | `copy_link` in `src/lib.rs` |
| A side panel, opened from the popup with `side_panel::open_on_click` | `src/panel.rs` |
| A devtools panel: `devtools::eval`, `inspected_tab_id`, refreshed on `on_shown` and `on_navigated`, retrying until the new page can run code | `src/devtools.rs` |

## Try it

1. Open an article and click the toolbar button (or press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>L</kbd>). Click a heading in the outline to jump to it.
2. Click **Save outline** to download it as Markdown, or right-click the page and pick **Copy link as Markdown**.
3. Click **Reading progress** and scroll the page: the side panel follows along. On a tab you haven't opened the popup on, it asks for access to all sites first.
4. Open the developer tools and pick the **Page Lens** panel.
5. Open a Wikipedia article: the pill shows the reading time and the toolbar badge shows the minutes.
