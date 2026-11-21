use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::alarms::{Alarm, AlarmOptions};
use oxichrome::idle;
use oxichrome::notifications::Notification;
use oxichrome::prelude::*;

#[extension(
    name = "Focus Timer",
    description = "A focus timer with breaks, a countdown badge, reminders and a pause when you step away.",
    version = "0.3.0",
    permissions = [Alarms, Notifications, Storage, Idle],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = "Focus Timer", shortcut = "Alt+Shift+F" },
    gecko_id = "focus-timer@oxichrome.dev"
)]
pub struct FocusTimer;

const DONE: &str = "done";
const TICK: &str = "tick";
const TIMER: &str = "timer";
const SETTINGS: &str = "settings";
const ICON: &str = "icons/128.png";
const MINUTE: f64 = 60_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Focus,
    Break,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Focus => "Focus",
            Mode::Break => "Break",
        }
    }

    fn next(self) -> Mode {
        match self {
            Mode::Focus => Mode::Break,
            Mode::Break => Mode::Focus,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Run {
    #[default]
    Stopped,
    Running { ends_at: f64 },
    Paused { left_ms: f64, by_idle: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Timer {
    pub mode: Mode,
    pub run: Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub focus: u32,
    pub rest: u32,
    pub away: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { focus: 25, rest: 5, away: 2 }
    }
}

impl Settings {
    fn length_ms(&self, mode: Mode) -> f64 {
        let minutes = match mode {
            Mode::Focus => self.focus,
            Mode::Break => self.rest,
        };
        f64::from(minutes.max(1)) * MINUTE
    }

    fn away_after(&self) -> Duration {
        Duration::from_secs(u64::from(self.away.max(1)) * 60)
    }
}

impl Timer {
    fn left_ms(&self, settings: &Settings, now: f64) -> f64 {
        match self.run {
            Run::Stopped => settings.length_ms(self.mode),
            Run::Running { ends_at } => (ends_at - now).max(0.0),
            Run::Paused { left_ms, .. } => left_ms,
        }
    }

    fn running(&self) -> bool {
        matches!(self.run, Run::Running { .. })
    }

    fn badge(&self, settings: &Settings, now: f64) -> String {
        match self.run {
            Run::Stopped => String::new(),
            Run::Running { .. } => ((self.left_ms(settings, now) / MINUTE).ceil() as u64).to_string(),
            Run::Paused { .. } => "II".to_string(),
        }
    }
}

fn clock(ms: f64) -> String {
    let total = (ms / 1000.0).ceil() as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

fn now() -> f64 {
    js_sys::Date::now()
}

async fn settings() -> Settings {
    storage::sync::get(SETTINGS).await.ok().flatten().unwrap_or_default()
}

async fn save_settings(settings: Settings) {
    let _ = storage::sync::set(SETTINGS, &settings).await;
    let _ = idle::set_detection_interval(settings.away_after()).await;
}

async fn load() -> Timer {
    storage::get(TIMER).await.ok().flatten().unwrap_or_default()
}

async fn save(timer: Timer) -> Timer {
    let _ = storage::set(TIMER, &timer).await;
    badge(&timer).await;
    timer
}

async fn badge(timer: &Timer) {
    let text = timer.badge(&settings().await, now());
    let color = if timer.mode == Mode::Break { "#23935a" } else { "#dc5f26" };
    let _ = action::set_badge_background_color(color, None).await;
    let _ = action::set_badge_text(&text, None).await;
}

async fn run_for(mut timer: Timer, left_ms: f64) -> Timer {
    let ends_at = now() + left_ms;
    timer.run = Run::Running { ends_at };
    let _ = alarms::create(DONE, AlarmOptions::at(ends_at as u64)).await;
    let _ = alarms::create(TICK, AlarmOptions::after(Duration::from_secs(60)).then_every(Duration::from_secs(60))).await;
    save(timer).await
}

async fn stop(mut timer: Timer, run: Run) -> Timer {
    timer.run = run;
    let _ = alarms::clear(DONE).await;
    let _ = alarms::clear(TICK).await;
    save(timer).await
}

async fn start() -> Timer {
    let timer = load().await;
    run_for(timer, timer.left_ms(&settings().await, now())).await
}

async fn pause(by_idle: bool) -> Timer {
    let timer = load().await;
    match timer.run {
        Run::Running { ends_at } => stop(timer, Run::Paused { left_ms: (ends_at - now()).max(0.0), by_idle }).await,
        _ => timer,
    }
}

async fn toggle() -> Timer {
    if load().await.running() { pause(false).await } else { start().await }
}

async fn reset() -> Timer {
    stop(load().await, Run::Stopped).await
}

async fn finish() {
    let timer = load().await;
    if !timer.running() {
        return;
    }
    let next = timer.mode.next();
    stop(Timer { mode: next, run: Run::Stopped }, Run::Stopped).await;
    let note = Notification::basic(ICON, format!("{} done", timer.mode.name()), format!("Start the {} when you're ready.", next.name().to_lowercase()))
        .button(format!("Start {}", next.name().to_lowercase()))
        .button("5 more minutes")
        .require_interaction();
    let _ = notifications::create_with_id(DONE, &note).await;
}

async fn snooze() {
    let timer = load().await;
    run_for(Timer { mode: timer.mode.next(), ..timer }, 5.0 * MINUTE).await;
}

#[background]
async fn wake() {
    let _ = idle::set_detection_interval(settings().await.away_after()).await;
    let timer = load().await;
    match timer.run {
        Run::Running { ends_at } if ends_at <= now() => finish().await,
        _ => badge(&timer).await,
    }
}

#[on(alarms::on_alarm)]
async fn alarm(alarm: Alarm) {
    match alarm.name.as_str() {
        DONE => finish().await,
        TICK => badge(&load().await).await,
        _ => {}
    }
}

#[on(notifications::on_button_clicked)]
async fn button(id: String, index: u32) {
    let _ = notifications::clear(&id).await;
    match index {
        0 => {
            start().await;
        }
        _ => snooze().await,
    }
}

#[on(notifications::on_clicked)]
async fn clicked(id: String) {
    let _ = notifications::clear(&id).await;
    if !load().await.running() {
        start().await;
    }
}

#[on(idle::on_state_changed)]
async fn away(state: idle::State) {
    let timer = load().await;
    match (state, timer.run) {
        (idle::State::Idle | idle::State::Locked, Run::Running { .. }) if timer.mode == Mode::Focus => {
            pause(true).await;
        }
        (idle::State::Active, Run::Paused { by_idle: true, .. }) => {
            start().await;
        }
        _ => {}
    }
}

#[command(description = "Start or pause the timer", shortcut = "Alt+Shift+P")]
async fn start_or_pause() {
    toggle().await;
}

#[popup(style = ["base.css", "focus-timer.css"])]
fn Popup() -> impl IntoView {
    let timer = RwSignal::new(Timer::default());
    let config = RwSignal::new(Settings::default());
    let clock_now = RwSignal::new(now());
    let refresh = move || {
        clock_now.set(now());
        spawn_local(async move {
            config.set(settings().await);
            timer.set(load().await);
        });
    };
    refresh();
    set_interval(refresh, Duration::from_millis(500));
    let status = move || match timer.with(|t| t.run) {
        Run::Stopped => "Ready".to_string(),
        Run::Running { .. } => timer.with(|t| t.mode.name().to_string()),
        Run::Paused { by_idle: true, .. } => "Paused while away".to_string(),
        Run::Paused { .. } => "Paused".to_string(),
    };
    view! {
        <div class="popup focus" class:on-break=move || timer.with(|t| t.mode == Mode::Break)>
            <header class="head">
                <img class="mark" src="/icons/48.png" alt="" />
                <div class="titles"><h1>"Focus Timer"</h1><p>"Alt+Shift+P starts or pauses"</p></div>
                <a class="btn ghost" href="/options.html" target="_blank">"Settings"</a>
            </header>
            <div class="readout">
                <div class="time" role="timer">{move || clock(timer.with(|t| config.with(|c| t.left_ms(c, clock_now.get()))))}</div>
                <div class="status">{status}</div>
            </div>
            <div class="controls">
                <button class="btn" on:click=move |_| spawn_local(async move { timer.set(reset().await) })>"Reset"</button>
                <button class="btn primary lg play" on:click=move |_| spawn_local(async move { timer.set(toggle().await) })>
                    {move || match timer.with(|t| t.run) {
                        Run::Running { .. } => "Pause",
                        Run::Paused { .. } => "Resume",
                        Run::Stopped => "Start",
                    }}
                </button>
            </div>
        </div>
    }
}

fn minutes(config: RwSignal<Settings>, label: &'static str, hint: &'static str, read: fn(&Settings) -> u32, write: fn(&mut Settings, u32)) -> impl IntoView {
    let change = move |ev| {
        let value = event_target_value(&ev).parse().unwrap_or(1).clamp(1, 120);
        config.update(|c| write(c, value));
        spawn_local(save_settings(config.get_untracked()));
    };
    view! {
        <div class="setting">
            <div class="main"><div class="label">{label}</div><div class="hint">{hint}</div></div>
            <label class="field minutes">
                <input type="number" min="1" max="120" aria-label=label prop:value=move || config.with(read).to_string() on:change=change />
                <span class="muted">"min"</span>
            </label>
        </div>
    }
}

#[options_page(style = ["base.css", "focus-timer.css"])]
fn Options() -> impl IntoView {
    let config = RwSignal::new(Settings::default());
    spawn_local(async move { config.set(settings().await) });
    view! {
        <main class="page">
            <header class="head">
                <img class="mark" src="/icons/128.png" alt="" />
                <div class="titles"><h1>"Focus Timer"</h1><p>"Settings sync across your browsers."</p></div>
            </header>
            <section class="card">
                {minutes(config, "Focus", "How long one focus session runs.", |c| c.focus, |c, v| c.focus = v)}
                {minutes(config, "Break", "The rest between focus sessions.", |c| c.rest, |c, v| c.rest = v)}
                {minutes(config, "Away after", "Focus pauses after this long without input and resumes when you are back.", |c| c.away, |c, v| c.away = v)}
            </section>
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_alternate() {
        assert_eq!(Mode::Focus.next(), Mode::Break);
        assert_eq!(Mode::Break.next(), Mode::Focus);
    }

    #[test]
    fn time_left_and_badge() {
        let settings = Settings::default();
        let stopped = Timer::default();
        assert_eq!(stopped.left_ms(&settings, 0.0), 25.0 * MINUTE);
        assert_eq!(stopped.badge(&settings, 0.0), "");
        let running = Timer { mode: Mode::Focus, run: Run::Running { ends_at: 90_000.0 } };
        assert_eq!(running.badge(&settings, 0.0), "2");
        assert_eq!(running.left_ms(&settings, 100_000.0), 0.0);
        let paused = Timer { mode: Mode::Break, run: Run::Paused { left_ms: 1.0, by_idle: false } };
        assert_eq!(paused.badge(&settings, 0.0), "II");
    }

    #[test]
    fn clock_rounds_up() {
        assert_eq!(clock(25.0 * MINUTE), "25:00");
        assert_eq!(clock(61_001.0), "01:02");
        assert_eq!(clock(0.0), "00:00");
    }
}
