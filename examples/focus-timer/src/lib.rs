mod options;
mod popup;
mod timer;

use oxichrome::alarms::Alarm;
use oxichrome::prelude::*;
use oxichrome::idle;

#[extension(
    name = "Focus Timer",
    description = "A focus timer with breaks, reminders, a running badge, and a pause when you step away.",
    version = "0.3.0",
    permissions = [Alarms, Notifications, Storage, Idle],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = "Focus Timer", shortcut = "Alt+Shift+F" },
    gecko_id = "focus-timer@oxichrome.dev"
)]
pub struct FocusTimer;

#[background]
async fn start() {
    timer::wake().await;
}

#[on(alarms::on_alarm)]
async fn alarm(alarm: Alarm) {
    match alarm.name.as_str() {
        timer::DONE_ALARM => timer::finish().await,
        timer::TICK_ALARM => timer::tick().await,
        _ => {}
    }
}

#[on(notifications::on_button_clicked)]
async fn button(id: String, index: u32) {
    let _ = notifications::clear(&id).await;
    match (id.as_str(), index) {
        (timer::DONE_NOTE, 0) => {
            timer::toggle().await;
        }
        (timer::DONE_NOTE, _) => timer::snooze().await,
        (timer::AWAY_NOTE, 0) => {
            timer::start().await;
        }
        (timer::AWAY_NOTE, _) => {
            timer::reset().await;
        }
        _ => {}
    }
}

#[on(notifications::on_clicked)]
async fn clicked(id: String) {
    let _ = notifications::clear(&id).await;
    if (id == timer::DONE_NOTE || id == timer::AWAY_NOTE) && !timer::load().await.running() {
        timer::start().await;
    }
}

#[on(idle::on_state_changed)]
async fn away(state: idle::State) {
    timer::on_idle(state).await;
}

#[command(description = "Start or pause the timer", shortcut = "Alt+Shift+P")]
async fn start_or_pause() {
    timer::toggle().await;
}

#[command(description = "Skip to the next round")]
async fn skip_round() {
    timer::skip().await;
}
