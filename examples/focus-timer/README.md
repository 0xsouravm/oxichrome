# Focus Timer

A focus timer with short and long breaks. The badge counts down the minutes, a notification tells you when a round ends (with buttons to start the next one or snooze), and focus pauses by itself when you step away from the computer. Settings sync between browsers through `storage.sync`. Session history stays on the device.

```sh
cd examples/focus-timer
cargo oxichrome build                    # Chrome: load dist/chromium as an unpacked extension
cargo oxichrome build --target firefox   # Firefox: load dist/firefox/manifest.json in about:debugging
```

## What it shows

| Feature | Where |
|---|---|
| An alarm at an exact time (`AlarmOptions::at`) for the end of a round, and a repeating one for the badge | `schedule` in `src/timer.rs` |
| Surviving a browser restart: `#[background]` checks the saved state, finishes an overdue round and recreates a lost alarm | `wake` |
| Notifications with two buttons, `on_button_clicked` and `on_clicked` (Firefox has no buttons, so a click starts the next round) | `finish`, `src/lib.rs` |
| `idle::on_state_changed` and `idle::set_detection_interval` to pause while you're away and say so when you're back | `on_idle` |
| Badge text, colour and tooltip that follow the timer | `badge` |
| `storage::sync` for settings, `storage` (local) for the timer and history | `src/timer.rs` |
| An options page with steppers, switches and a session history | `src/options.rs` |
| Two keyboard shortcuts and a toolbar shortcut | `#[command]`, `#[extension]` |

All state lives in storage, so the popup, the options page and the background can each change the timer and the others catch up.

## Try it

1. Open the popup (<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>F</kbd>), name your task and press **Start**. Press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd> anywhere to pause.
2. On the options page (the gear), set focus to 1 minute to see a round end quickly.
3. Leave the keyboard and mouse alone for longer than **Away after**: focus pauses, and a notification offers to resume when you're back.
