# Launchpad

One popup for getting somewhere: a search box that opens addresses and searches everything else with your own engine, your most visited sites, recent history, and a button that saves the open page. In the address bar, type `lp` and a space to search your bookmarks and history without opening the popup.

```sh
cd examples/launchpad
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| `#[omnibox]` with a keyword, a default row and styled descriptions (`matched`, `dim`, `url`) | `suggest` in `src/lib.rs` |
| `omnibox::on_input_entered` and `omnibox::open`, which follows the tab the user picked | `entered` and `go` in `src/lib.rs` |
| `search::query` with the user's default engine | `go` in `src/lib.rs` |
| `top_sites::get` for the most visited tiles | `top` in `src/lib.rs` |
| Bookmarks: a special folder, a folder made once, creating, listing, searching and removing bookmarks | `folder`, `saved`, `save`, `suggest`, `Options` |
| `history::search` for recent pages and the typed `history::on_visited` event, counted in `storage::session` | `recent`, `visited` in `src/lib.rs` |
| `management::get_self` for the version and install type | `Options` in `src/pages.rs` |
| Typed `tabs::Tab` from `tabs::query` for the open page | `current_page` in `src/pages.rs` |

## Try it

1. Open the popup, type `docs.rs/serde` and press Enter: it opens as an address. Type `rust lifetimes` and it goes to your search engine instead.
2. On any page, open the popup and click **Save this page**. It lands in Other bookmarks › Launchpad.
3. In the address bar type `lp`, a space, and part of a page title you've visited.
4. Open **Saved pages** from the popup footer to see the version and remove saved pages.
