# Focus Timer

A focus timer that alternates focus sessions and breaks. The badge counts down the minutes, a notification with buttons tells you when a round ends, and focus pauses by itself while you are away from the computer. Settings sync between browsers through `storage.sync`.

```sh
cd examples/focus-timer
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| A `done` alarm at the exact end of a round (`AlarmOptions::at`) and a repeating `tick` alarm for the badge, handled by a typed `alarms::on_alarm` | `run_for`, `alarm` |
| A notification with two buttons and the typed `on_button_clicked` (Firefox has no buttons, so `on_clicked` starts the next round) | `finish`, `button`, `clicked` |
| `idle::on_state_changed` pauses focus while you are away and resumes it when you are back; `idle::set_detection_interval` follows the setting | `away`, `save_settings`, `wake` |
| A keyboard command and a toolbar shortcut | `#[command] start_or_pause`, `#[extension]` |
| Badge text and colour that follow the timer | `badge` |
| `storage::sync` for settings, local `storage` for the timer | `settings`, `load`, `save` |
| A popup and an options page that share the same storage | `Popup`, `Options` |

## Try it

1. Open the popup (<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>F</kbd>) and press **Start**. Press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd> anywhere to pause.
2. On the options page, set focus to 1 minute to see a round end and the notification appear.
3. Leave the keyboard and mouse alone for longer than **Away after**: focus pauses, and resumes when you come back.
