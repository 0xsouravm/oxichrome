# Quiet Web

A small blocker built on network rules that run inside the browser. Two filter lists ship with the extension (trackers on, annoyances off), you keep your own list of sites that never open, the popup lists the tracker requests that were stopped, and you can clear the cookies of the site you're on.

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
| Dynamic rules built from storage: `block`, a `redirect` to an extension page, and a `Transform` that removes query parameters, replaced in one `update_dynamic_rules` call | `rules_for` and `apply` |
| A page in `web_accessible` that blocked sites are redirected to | `static/blocked.html` |
| `web_request::on_error_occurred` with `urls` and `types` filters, logging tracker requests a rule stopped, one storage write at a time | `tracker_stopped` in `src/lib.rs`, `log_request` |
| `web_navigation::on_error_occurred` (`NavError::Blocked`) to count stopped pages | `page_failed` |
| Optional host permissions asked for with `request_on_click` and given back with `permissions::remove` | `src/options.rs` |
| Re-applying rules from `permissions::on_added` and `on_removed`, since redirects need host access | `src/lib.rs` |
| `cookies::all` and `cookies::remove_all` for one site | `src/popup.rs` |
| A Chrome-only call (`show_match_count_on_badge`) whose `Err` explains itself in Firefox | `src/options.rs` |

## Try it

1. Open the settings and add `youtube.com` to the blocked sites. Opening it now fails with the browser's error page.
2. Click **Allow** next to "Access to all sites". Blocked sites now open the Quiet Web page instead.
3. Visit a news site and open the popup: stopped tracker requests are listed with their type, and **Clear** removes the site's cookies.
4. Turn on **Strip tracking parameters**, then open a link with `?utm_source=x`.
