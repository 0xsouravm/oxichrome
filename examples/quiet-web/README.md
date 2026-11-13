# Quiet Web

A small blocker built on network rules that run inside the browser. Two filter lists ship with the extension (trackers on, annoyances off), you keep your own list of sites that never open, you can pause protection on a site that breaks, strip tracking parameters from addresses, and see and clear the cookies of the site you're on.

```sh
cd examples/quiet-web
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| Rule set files checked at compile time, one of them off at install (`rule_sets`, `disabled_rule_sets`) | `static/rules`, `#[extension]` |
| Turning rule sets on and off with `update_rule_sets` and reading them with `enabled_rule_sets` | `set_list` in `src/rules.rs` |
| Dynamic rules built from your own state: `allow_all_requests` to pause a site, `block` or a `redirect` to a page of the extension, and a `Transform` that removes query parameters | `rules_for` |
| Replacing every dynamic rule in one `update_dynamic_rules` call, so storage stays the single source of truth | `apply` |
| A page in `web_accessible` that blocked sites are redirected to | `static/blocked.html` |
| Optional host permissions: asked for one site from the popup, or for every site from the options page, and given back with `permissions::remove` | `src/popup.rs`, `src/options.rs` |
| Re-applying rules from `permissions::on_added` and `on_removed`, since redirects need host access | `src/lib.rs` |
| `cookies::all` and `cookies::remove_all` for one site | `src/popup.rs` |
| `web_navigation::on_completed` and `on_error_occurred` (`NavError::Blocked`) to count pages | `src/lib.rs` |
| A Chrome-only call (`show_match_count_on_badge`) whose `Err` explains itself in Firefox | `src/options.rs` |

## Try it

1. Open a news site and click the toolbar button: trackers are already blocked, and the cookies card asks for access to read that site's cookies.
2. Click **Never open …**. The tab reloads into the blocked page once Quiet Web has access to all sites, or into the browser's own error page before that.
3. Turn protection off for a site that breaks. It's listed under **Paused on** in the settings, with a button to resume.
4. In the settings, allow access to all sites and turn on **Strip tracking parameters**, then open a link with `?utm_source=x`.
