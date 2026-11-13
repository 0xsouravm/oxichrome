# Page Lens

Look at a page before you read it: reading time, word and link counts, and its outline, with a click to jump to any heading. Make the text easier to read, outline the links, copy a Markdown link, and keep quotes and notes for each page in the side panel, exported as one Markdown file. On Wikipedia and MDN a small reading-time pill appears in the corner.

```sh
cd examples/page-lens
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| On-demand content scripts that take an argument and return a struct: `inspect`, `selection`, `scroll_to` | `src/page.rs` |
| A content script that starts by itself on matching pages, with its own stylesheet (`css = [...]`) | `reading_pill` |
| Messaging: the page script sends a typed message, the background answers with `send_response` and sets a badge on that tab | `message` in `src/lib.rs` |
| `scripting::insert_css` and `remove_css`, remembered per tab in `storage::session` | `src/popup.rs` |
| An offscreen document for the clipboard, called from the popup and from a menu | `copy` in `src/lib.rs` |
| `ActiveTab`: the popup, the menus and the shortcut can read the current page with no host permission | `#[extension]` |
| Asking for every site only when the user wants it, with `permissions::request_on_click` | `Notes` in `src/panel.rs` |
| `downloads::save` for a file the extension made | `export` in `src/notes.rs` |
| A menu on selected text with `%s` in the title, and one on pages and links | `save_quote`, `copy_link` |
| A side panel that follows the tab you're looking at | `src/panel.rs` |

## Try it

1. Open an article and click the toolbar button (or press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>L</kbd>). Click a heading in the outline to jump to it.
2. Turn on **Readable text**, then open the popup again: the switch remembers the tab.
3. Select a sentence and right-click → **Save to Page Lens**, or press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>Q</kbd>.
4. Open **Notes**. **Add selection** asks once for access to all sites; until then, the menu and the shortcut still work through `ActiveTab`.
5. Browser pages (`chrome://`, `about:`) and the extension stores can't be read by any extension, and the popup says so.
