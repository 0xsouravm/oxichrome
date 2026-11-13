use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::popup;

use crate::timer::{self, Mode, Run, Session, Settings, Timer};

const RING: f64 = 2.0 * std::f64::consts::PI * 92.0;

pub const PLAY: &str = r#"<path d="M7 4.5v15l12-7.5z" fill="currentColor" stroke="none"/>"#;
pub const PAUSE: &str = r#"<rect x="6" y="4.5" width="4" height="15" rx="1" fill="currentColor" stroke="none"/><rect x="14" y="4.5" width="4" height="15" rx="1" fill="currentColor" stroke="none"/>"#;
pub const RESET: &str = r#"<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/>"#;
pub const SKIP: &str = r#"<path d="m5 4 10 8-10 8z"/><path d="M19 5v14"/>"#;
pub const GEAR: &str = r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>"#;

pub fn icon(paths: &'static str) -> impl IntoView {
    view! { <svg viewBox="0 0 24 24" aria-hidden="true" inner_html=paths></svg> }
}

pub fn today(sessions: &[Session]) -> (usize, f64) {
    let since = timer::start_of_today();
    let focus: Vec<&Session> = sessions.iter().filter(|session| session.mode == Mode::Focus && session.ended_at >= since).collect();
    (focus.len(), focus.iter().map(|session| session.minutes).sum())
}

#[popup(style = ["base.css", "focus-timer.css"])]
fn Popup() -> impl IntoView {
    let state = RwSignal::new(Timer::default());
    let settings = RwSignal::new(Settings::default());
    let sessions = RwSignal::new(Vec::<Session>::new());
    let now = RwSignal::new(timer::now());
    let loaded = RwSignal::new(false);
    let task = RwSignal::new(String::new());

    let reload = move || {
        spawn_local(async move {
            let fresh = timer::load().await;
            if !loaded.get_untracked() {
                task.set(fresh.task.clone());
            }
            settings.set(timer::settings().await);
            sessions.set(timer::sessions().await);
            state.set(fresh);
            loaded.set(true);
        });
    };
    reload();
    let clock = set_interval_with_handle(move || now.set(timer::now()), Duration::from_millis(250)).ok();
    let sync = set_interval_with_handle(reload, Duration::from_millis(1000)).ok();
    on_cleanup(move || {
        for handle in [clock, sync].into_iter().flatten() {
            handle.clear();
        }
    });

    let left = Memo::new(move |_| state.with(|state| settings.with(|settings| state.left_ms(settings, now.get()))));
    let progress = Memo::new(move |_| {
        let length = state.with(|state| settings.with(|settings| settings.length_ms(state.mode)));
        (1.0 - left.get() / length).clamp(0.0, 1.0)
    });
    let status = move || match state.with(|state| state.run) {
        Run::Stopped => "Ready".to_string(),
        Run::Running { .. } => state.with(|state| state.mode.name().to_string()),
        Run::Paused { by_idle: true, .. } => "Paused while away".to_string(),
        Run::Paused { .. } => "Paused".to_string(),
    };
    let toggle = move |_| {
        spawn_local(async move {
            state.set(timer::toggle().await);
            now.set(timer::now());
        });
    };
    let reset = move |_| {
        spawn_local(async move {
            state.set(timer::reset().await);
        });
    };
    let skip = move |_| {
        spawn_local(async move {
            state.set(timer::skip().await);
        });
    };
    let pick = move |mode: Mode| {
        spawn_local(async move {
            state.set(timer::set_mode(mode).await);
        });
    };
    let save_task = move |text: String| {
        task.set(text.clone());
        spawn_local(async move { timer::set_task(text).await });
    };
    let modes = [Mode::Focus, Mode::Short, Mode::Long];

    view! {
        <div class="popup" class:on-break=move || state.with(|state| state.mode.is_break())>
            <header class="head">
                <img class="mark" src="/icons/48.png" alt="" />
                <div class="titles">
                    <h1>"Focus Timer"</h1>
                    <p>{move || {
                        let every = settings.with(|settings| settings.long_every.max(1));
                        format!("Round {} of {}", state.with(|state| state.round % every) + 1, every)
                    }}</p>
                </div>
                <a class="icon-btn" href="/options.html" target="_blank" title="Settings and history" aria-label="Settings and history">{icon(GEAR)}</a>
            </header>
            <div class="body">
                <div class="segmented" role="group" aria-label="Mode">
                    {modes.into_iter().map(|mode| view! {
                        <button
                            aria-pressed=move || (state.with(|state| state.mode) == mode).to_string()
                            on:click=move |_| pick(mode)
                        >{mode.name()}</button>
                    }).collect_view()}
                </div>
                <div class="dial" class:running=move || state.with(|state| state.running())>
                    <svg viewBox="0 0 200 200" aria-hidden="true">
                        <circle class="track" cx="100" cy="100" r="92" />
                        <circle
                            class="progress"
                            cx="100"
                            cy="100"
                            r="92"
                            stroke-dasharray=RING.to_string()
                            stroke-dashoffset=move || (RING * (1.0 - progress.get())).to_string()
                        />
                    </svg>
                    <div class="readout">
                        <div class="time" role="timer" aria-live="off">{move || timer::clock(left.get())}</div>
                        <div class="status">{status}</div>
                    </div>
                </div>
                <label class="field">
                    <input
                        type="text"
                        maxlength="80"
                        placeholder="What are you working on?"
                        aria-label="What are you working on?"
                        prop:value=move || task.get()
                        on:change=move |ev| save_task(event_target_value(&ev))
                    />
                </label>
                <div class="controls">
                    <button class="icon-btn round" title="Reset" aria-label="Reset" on:click=reset>{icon(RESET)}</button>
                    <button class="btn primary lg play" on:click=toggle disabled=move || !loaded.get()>
                        {move || if state.with(|state| state.running()) { icon(PAUSE).into_any() } else { icon(PLAY).into_any() }}
                        {move || match state.with(|state| state.run) {
                            Run::Running { .. } => "Pause",
                            Run::Paused { .. } => "Resume",
                            Run::Stopped => "Start",
                        }}
                    </button>
                    <button class="icon-btn round" title="Skip to the next round" aria-label="Skip to the next round" on:click=skip>{icon(SKIP)}</button>
                </div>
                <div class="stats">
                    <div class="stat">
                        <div class="value">{move || sessions.with(|all| today(all).0)}</div>
                        <div class="label">"Sessions"</div>
                    </div>
                    <div class="stat">
                        <div class="value">{move || timer::duration_label(sessions.with(|all| today(all).1))}</div>
                        <div class="label">"Focused"</div>
                    </div>
                    <div class="stat">
                        <div class="dots" aria-hidden="true">
                            {move || {
                                let every = settings.with(|settings| settings.long_every.max(1));
                                let done = state.with(|state| state.round % every);
                                (0..every).map(|n| view! { <span class:done={n < done}></span> }).collect_view()
                            }}
                        </div>
                        <div class="label">"Long break"</div>
                    </div>
                </div>
            </div>
        </div>
    }
}
