# Launchpad

One popup for getting somewhere: a search box that knows the difference between an address and a search, your most visited sites, recent history, and pages you saved. In the address bar, type `lp` and a space to search your bookmarks and history without opening the popup.

```sh
cd examples/launchpad
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| `#[omnibox]` with a keyword, a default suggestion and styled descriptions (`matched`, `dim`, `url`) | `suggest` in `src/lib.rs` |
| `omnibox::on_input_entered` and `omnibox::open`, which respects the tab the user picked | `entered` in `src/lib.rs` |
| `search::query` with the user's own default engine, in this tab or a new one | `go` in `src/places.rs` |
| `top_sites::get` for the most visited tiles | `top` |
| `history::search`, `history::on_visited` and `history::delete_range` | `recent_history`, `src/lib.rs`, `src/options.rs` |
| Bookmarks: finding a special folder, making a folder once, creating, listing, searching and removing bookmarks | `folder`, `saved`, `save`, `matching` |
| `storage::session` for a counter that resets when the browser closes | `count_visit` |
| `management::get_self` for the about card, and `uninstall_on_click` for an uninstall button that keeps the user gesture | `src/options.rs` |

## Try it

1. Press Alt+Shift+K, type `docs.rs/serde` and press Enter: it opens as an address. Type `rust lifetimes` and it goes to your search engine instead.
2. On any page, click **Save this page**. It lands in Other bookmarks › Launchpad, and the star fills in.
3. In the address bar type `lp`, a space, and part of a page title you've visited.
4. Open the settings from the gear: remove saved pages, forget the last hour of history, or uninstall.
