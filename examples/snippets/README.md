# Snippets

Your own small bits of JavaScript, one per site, like a tiny script manager. You type a site and some code on the settings page; from then on the code runs on every page of that site. Each run reports back to the extension, which counts it and remembers the last page. The settings page also says what the browser still needs before snippets can run, and where to turn it on.

```sh
cd examples/snippets
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| `UserScripts` in `optional_permissions` and sites in `optional_host_permissions` | top of `src/lib.rs` |
| `user_scripts::availability` and what to tell the user for `NeedsPermission` (both browsers) and `NeedsUserToggle` (Chrome) | `Options` |
| `permissions::request_on_click` for the permission, and for one site's access built at click time | `allow` and `add` in `Options` |
| `UserScript` with a match pattern and two code sources (the user's code, then a report through `chrome.runtime`, which both browsers give user scripts) | `Snippet::script` |
| `user_scripts::register`, `get`, `update` and `unregister` | `save`, `delete` |
| `user_scripts::get_scripts` to put back scripts Firefox drops on reload, from `#[background]` and `permissions::on_added` | `restore`, `start`, `granted` |
| `user_scripts::configure_world` turning on messaging for user scripts | `restore` |
| `#[on(runtime::on_user_script_message)]` with a typed message and its `MessageSender` url | `ran` |
| `#[on(action::on_clicked)]` and `runtime::open_options_page` | `clicked` |
| `storage::get` and `storage::set` with a `Vec` and a `HashMap` | `load`, `save`, `ran` |

## Try it

1. Click the toolbar button to open the settings page, then click Allow.
2. Chrome only: open `chrome://extensions`, click Details under Snippets and turn on Allow User Scripts, then click Check again.
3. Type `example.com` and `document.body.style.background = "gold";`, then Save snippet and allow access to the site.
4. Open https://example.com, then reopen the settings page: the snippet shows "Ran 1 time, last on https://example.com/".
5. Click a snippet to edit it (saving the same site updates it), or Remove it.
