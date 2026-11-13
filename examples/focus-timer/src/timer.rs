use std::time::Duration;

use oxichrome::alarms::{self, AlarmOptions};
use oxichrome::notifications::{self, Notification};
use oxichrome::{action, idle, storage};
use serde::{Deserialize, Serialize};

pub const DONE_ALARM: &str = "done";
pub const TICK_ALARM: &str = "tick";
pub const DONE_NOTE: &str = "done";
pub const AWAY_NOTE: &str = "away";
const ICON: &str = "icons/128.png";
const TIMER: &str = "timer";
const SESSIONS: &str = "sessions";
const SETTINGS: &str = "settings";
const MINUTE: f64 = 60_000.0;
const KEEP_SESSIONS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Focus,
    Short,
    Long,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Focus => "Focus",
            Mode::Short => "Short break",
            Mode::Long => "Long break",
        }
    }

    pub fn is_break(self) -> bool {
        self != Mode::Focus
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Run {
    #[default]
    Stopped,
    Running { ends_at: f64 },
    Paused { left_ms: f64, by_idle: bool },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Timer {
    pub mode: Mode,
    pub run: Run,
    pub round: u32,
    pub task: String,
    pub previous: Option<Mode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub focus_minutes: u32,
    pub short_minutes: u32,
    pub long_minutes: u32,
    pub long_every: u32,
    pub auto_start_breaks: bool,
    pub auto_start_focus: bool,
    pub notify: bool,
    pub pause_when_away: bool,
    pub away_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_minutes: 5,
            long_minutes: 15,
            long_every: 4,
            auto_start_breaks: true,
            auto_start_focus: false,
            notify: true,
            pause_when_away: true,
            away_minutes: 2,
        }
    }
}

impl Settings {
    pub fn length_ms(&self, mode: Mode) -> f64 {
        let minutes = match mode {
            Mode::Focus => self.focus_minutes,
            Mode::Short => self.short_minutes,
            Mode::Long => self.long_minutes,
        };
        f64::from(minutes.max(1)) * MINUTE
    }

    pub fn away_after(&self) -> Duration {
        Duration::from_secs(u64::from(self.away_minutes.max(1)) * 60)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub mode: Mode,
    pub minutes: f64,
    pub ended_at: f64,
    pub task: String,
}

pub fn now() -> f64 {
    js_sys::Date::now()
}

pub fn start_of_today() -> f64 {
    let day = js_sys::Date::new_0();
    day.set_hours(0);
    day.set_minutes(0);
    day.set_seconds(0);
    day.set_milliseconds(0);
    day.get_time()
}

impl Timer {
    pub fn left_ms(&self, settings: &Settings, at: f64) -> f64 {
        match self.run {
            Run::Stopped => settings.length_ms(self.mode),
            Run::Running { ends_at } => (ends_at - at).max(0.0),
            Run::Paused { left_ms, .. } => left_ms,
        }
    }

    pub fn running(&self) -> bool {
        matches!(self.run, Run::Running { .. })
    }

    fn next_mode(&self, settings: &Settings) -> Mode {
        match self.mode {
            Mode::Focus if self.round + 1 >= settings.long_every.max(1) => Mode::Long,
            Mode::Focus => Mode::Short,
            Mode::Short | Mode::Long => Mode::Focus,
        }
    }
}

pub fn clock(ms: f64) -> String {
    let total = (ms / 1000.0).ceil() as u64;
    let (hours, minutes, seconds) = (total / 3600, (total / 60) % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

pub fn duration_label(minutes: f64) -> String {
    let minutes = minutes.round() as u64;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

pub async fn settings() -> Settings {
    storage::sync::get(SETTINGS).await.ok().flatten().unwrap_or_default()
}

pub async fn save_settings(settings: &Settings) {
    let _ = storage::sync::set(SETTINGS, settings).await;
    let _ = idle::set_detection_interval(settings.away_after()).await;
    let timer = load().await;
    if timer.run == Run::Stopped {
        badge(&timer, settings).await;
    }
}

pub async fn load() -> Timer {
    storage::get(TIMER).await.ok().flatten().unwrap_or_default()
}

async fn save(timer: &Timer, settings: &Settings) {
    let _ = storage::set(TIMER, timer).await;
    badge(timer, settings).await;
}

pub async fn sessions() -> Vec<Session> {
    storage::get(SESSIONS).await.ok().flatten().unwrap_or_default()
}

pub async fn clear_sessions() {
    let _ = storage::remove(SESSIONS).await;
}

async fn record(session: Session) {
    let mut all = sessions().await;
    all.push(session);
    if all.len() > KEEP_SESSIONS {
        all.drain(..all.len() - KEEP_SESSIONS);
    }
    let _ = storage::set(SESSIONS, &all).await;
}

async fn schedule(ends_at: f64) {
    let _ = alarms::create(DONE_ALARM, AlarmOptions::at(ends_at as u64)).await;
    let _ = alarms::create(TICK_ALARM, AlarmOptions::after(Duration::from_secs(60)).then_every(Duration::from_secs(60))).await;
}

async fn unschedule() {
    let _ = alarms::clear(DONE_ALARM).await;
    let _ = alarms::clear(TICK_ALARM).await;
}

pub async fn badge(timer: &Timer, settings: &Settings) {
    let left = timer.left_ms(settings, now());
    let (text, color) = match timer.run {
        Run::Stopped => (String::new(), "#e0702f"),
        Run::Running { .. } => (format!("{}", (left / MINUTE).ceil() as u64), if timer.mode.is_break() { "#2f9e64" } else { "#e0702f" }),
        Run::Paused { .. } => ("❚❚".to_string(), "#7a7a80"),
    };
    let _ = action::set_badge_background_color(color, None).await;
    let _ = action::set_badge_text(&text, None).await;
    let title = match timer.run {
        Run::Stopped => "Focus Timer".to_string(),
        Run::Running { .. } => format!("{}: {} left", timer.mode.name(), clock(left)),
        Run::Paused { .. } => format!("{} paused at {}", timer.mode.name(), clock(left)),
    };
    let _ = action::set_title(&title, None).await;
}

pub async fn start() -> Timer {
    let settings = settings().await;
    let mut timer = load().await;
    let ends_at = now() + timer.left_ms(&settings, now());
    timer.run = Run::Running { ends_at };
    schedule(ends_at).await;
    let _ = notifications::clear(AWAY_NOTE).await;
    save(&timer, &settings).await;
    timer
}

pub async fn pause(by_idle: bool) -> Timer {
    let settings = settings().await;
    let mut timer = load().await;
    if let Run::Running { ends_at } = timer.run {
        timer.run = Run::Paused { left_ms: (ends_at - now()).max(0.0), by_idle };
        unschedule().await;
        save(&timer, &settings).await;
    }
    timer
}

pub async fn reset() -> Timer {
    let settings = settings().await;
    let mut timer = load().await;
    timer.run = Run::Stopped;
    unschedule().await;
    save(&timer, &settings).await;
    timer
}

pub async fn set_mode(mode: Mode) -> Timer {
    let settings = settings().await;
    let mut timer = load().await;
    timer.mode = mode;
    timer.run = Run::Stopped;
    unschedule().await;
    save(&timer, &settings).await;
    timer
}

pub async fn set_task(task: String) {
    let mut timer = load().await;
    timer.task = task;
    let _ = storage::set(TIMER, &timer).await;
}

pub async fn skip() -> Timer {
    let settings = settings().await;
    let mut timer = load().await;
    let next = timer.next_mode(&settings);
    if timer.mode == Mode::Focus {
        timer.round = if next == Mode::Long { 0 } else { timer.round + 1 };
    }
    timer.previous = Some(timer.mode);
    timer.mode = next;
    timer.run = Run::Stopped;
    unschedule().await;
    save(&timer, &settings).await;
    timer
}

pub async fn finish() {
    let settings = settings().await;
    let timer = load().await;
    let Run::Running { ends_at } = timer.run else { return };
    let ended = timer.mode;
    let length = settings.length_ms(ended);
    record(Session { mode: ended, minutes: length / MINUTE, ended_at: ends_at, task: timer.task.clone() }).await;
    let mut next = skip().await;
    let auto = if next.mode.is_break() { settings.auto_start_breaks } else { settings.auto_start_focus };
    if auto {
        next = start().await;
    }
    if settings.notify {
        let (title, message) = match ended {
            Mode::Focus if timer.task.trim().is_empty() => ("Focus session done".to_string(), format!("{} well spent.", duration_label(length / MINUTE))),
            Mode::Focus => ("Focus session done".to_string(), format!("{} on {}.", duration_label(length / MINUTE), timer.task.trim())),
            Mode::Short | Mode::Long => ("Break's over".to_string(), "Ready for the next round?".to_string()),
        };
        let first = if auto { "Pause".to_string() } else { format!("Start {}", next.mode.name().to_lowercase()) };
        let note = Notification::basic(ICON, title, message)
            .context_message(if auto { format!("{} started", next.mode.name()) } else { "Focus Timer".to_string() })
            .button(first)
            .button("5 more minutes")
            .require_interaction();
        let _ = notifications::create_with_id(DONE_NOTE, &note).await;
    }
}

pub async fn snooze() {
    let settings = settings().await;
    let mut timer = load().await;
    if let Some(previous) = timer.previous.take() {
        timer.mode = previous;
        if previous == Mode::Focus {
            timer.round = if timer.round == 0 { settings.long_every.max(1) - 1 } else { timer.round - 1 };
        }
    }
    let ends_at = now() + 5.0 * MINUTE;
    timer.run = Run::Running { ends_at };
    schedule(ends_at).await;
    save(&timer, &settings).await;
}

pub async fn toggle() -> Timer {
    if load().await.running() {
        pause(false).await
    } else {
        start().await
    }
}

pub async fn on_idle(state: idle::State) {
    let settings = settings().await;
    let timer = load().await;
    match state {
        idle::State::Idle | idle::State::Locked => {
            if settings.pause_when_away && timer.running() && timer.mode == Mode::Focus {
                pause(true).await;
            }
        }
        idle::State::Active => {
            if let Run::Paused { left_ms, by_idle: true } = timer.run {
                let note = Notification::basic(ICON, "Welcome back", format!("Your focus timer paused at {} while you were away.", clock(left_ms)))
                    .context_message("Focus Timer")
                    .button("Resume")
                    .button("Reset");
                let _ = notifications::create_with_id(AWAY_NOTE, &note).await;
            }
        }
    }
}

pub async fn wake() {
    let settings = settings().await;
    let _ = idle::set_detection_interval(settings.away_after()).await;
    let timer = load().await;
    if let Run::Running { ends_at } = timer.run {
        if ends_at <= now() {
            finish().await;
            return;
        }
        if alarms::get(DONE_ALARM).await.ok().flatten().is_none() {
            schedule(ends_at).await;
        }
    }
    badge(&timer, &settings).await;
}

pub async fn tick() {
    let settings = settings().await;
    badge(&load().await, &settings).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fourth_focus_earns_a_long_break() {
        let settings = Settings::default();
        let mut timer = Timer::default();
        assert_eq!(timer.next_mode(&settings), Mode::Short);
        timer.round = 3;
        assert_eq!(timer.next_mode(&settings), Mode::Long);
        timer.mode = Mode::Long;
        assert_eq!(timer.next_mode(&settings), Mode::Focus);
    }

    #[test]
    fn clocks_and_labels() {
        assert_eq!(clock(25.0 * MINUTE), "25:00");
        assert_eq!(clock(61_001.0), "01:02");
        assert_eq!(clock(3_725_000.0), "1:02:05");
        assert_eq!(duration_label(95.0), "1h 35m");
        assert_eq!(duration_label(25.0), "25m");
    }
}
