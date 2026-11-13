use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::notifications::{self, Notification};
use oxichrome::options_page;

use crate::popup::{icon, today};
use crate::timer::{self, Mode, Session, Settings};

const MINUS: &str = r#"<path d="M5 12h14"/>"#;
const PLUS: &str = r#"<path d="M12 5v14"/><path d="M5 12h14"/>"#;
const DAY: f64 = 24.0 * 3600.0 * 1000.0;

#[component]
fn Stepper(label: &'static str, hint: &'static str, unit: &'static str, min: u32, max: u32, value: Signal<u32>, set: Callback<u32>) -> impl IntoView {
    let change = move |delta: i64| {
        let next = (i64::from(value.get_untracked()) + delta).clamp(i64::from(min), i64::from(max)) as u32;
        set.run(next);
    };
    view! {
        <div class="setting">
            <div class="main">
                <div class="label">{label}</div>
                <div class="hint">{hint}</div>
            </div>
            <div class="stepper" role="group" aria-label=label>
                <button aria-label=format!("Less {}", label.to_lowercase()) disabled=move || { value.get() <= min } on:click=move |_| change(-1)>{icon(MINUS)}</button>
                <span class="value">{move || value.get()}<small>{unit}</small></span>
                <button aria-label=format!("More {}", label.to_lowercase()) disabled=move || { value.get() >= max } on:click=move |_| change(1)>{icon(PLUS)}</button>
            </div>
        </div>
    }
}

#[component]
fn Toggle(label: &'static str, hint: &'static str, value: Signal<bool>, set: Callback<bool>) -> impl IntoView {
    view! {
        <div class="setting">
            <div class="main">
                <div class="label">{label}</div>
                <div class="hint">{hint}</div>
            </div>
            <button
                class="switch"
                role="switch"
                aria-label=label
                aria-checked=move || value.get().to_string()
                on:click=move |_| set.run(!value.get_untracked())
            ></button>
        </div>
    }
}

fn when(ended_at: f64) -> String {
    let date = js_sys::Date::new(&ended_at.into());
    let time = format!("{:02}:{:02}", date.get_hours(), date.get_minutes());
    if ended_at >= timer::start_of_today() {
        format!("Today, {time}")
    } else if ended_at >= timer::start_of_today() - DAY {
        format!("Yesterday, {time}")
    } else {
        let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        format!("{} {}, {time}", months[date.get_month() as usize], date.get_date())
    }
}

fn persist(settings: RwSignal<Settings>, change: impl FnOnce(&mut Settings)) {
    settings.update(change);
    let fresh = settings.get_untracked();
    spawn_local(async move { timer::save_settings(&fresh).await });
}

#[options_page(style = ["base.css", "focus-timer.css"])]
fn Options() -> impl IntoView {
    let settings = RwSignal::new(Settings::default());
    let sessions = RwSignal::new(Vec::<Session>::new());
    let note = RwSignal::new(None::<Result<String, String>>);
    spawn_local(async move {
        settings.set(timer::settings().await);
        sessions.set(timer::sessions().await);
    });

    let number = move |read: fn(&Settings) -> u32, write: fn(&mut Settings, u32)| {
        (Signal::derive(move || settings.with(read)), Callback::new(move |value: u32| persist(settings, |s| write(s, value))))
    };
    let flag = move |read: fn(&Settings) -> bool, write: fn(&mut Settings, bool)| {
        (Signal::derive(move || settings.with(read)), Callback::new(move |value: bool| persist(settings, |s| write(s, value))))
    };

    let (focus, set_focus) = number(|s| s.focus_minutes, |s, v| s.focus_minutes = v);
    let (short, set_short) = number(|s| s.short_minutes, |s, v| s.short_minutes = v);
    let (long, set_long) = number(|s| s.long_minutes, |s, v| s.long_minutes = v);
    let (every, set_every) = number(|s| s.long_every, |s, v| s.long_every = v);
    let (away, set_away) = number(|s| s.away_minutes, |s, v| s.away_minutes = v);
    let (breaks, set_breaks) = flag(|s| s.auto_start_breaks, |s, v| s.auto_start_breaks = v);
    let (focus_auto, set_focus_auto) = flag(|s| s.auto_start_focus, |s, v| s.auto_start_focus = v);
    let (notify, set_notify) = flag(|s| s.notify, |s, v| s.notify = v);
    let (pause, set_pause) = flag(|s| s.pause_when_away, |s, v| s.pause_when_away = v);

    let week = move || {
        let since = timer::start_of_today() - 6.0 * DAY;
        sessions.with(|all| all.iter().filter(|s| s.mode == Mode::Focus && s.ended_at >= since).map(|s| s.minutes).sum::<f64>())
    };
    let total = move || sessions.with(|all| all.iter().filter(|s| s.mode == Mode::Focus).map(|s| s.minutes).sum::<f64>());
    let clear = move |_| {
        spawn_local(async move {
            timer::clear_sessions().await;
            sessions.set(Vec::new());
        });
    };
    let test = move |_| {
        spawn_local(async move {
            let shown = notifications::create(&Notification::basic("icons/128.png", "Focus Timer", "Notifications are working.").context_message("Test")).await;
            note.set(Some(shown.map(|_| "Sent. Check your system's notification area.".to_string()).map_err(|err| err.to_string())));
        });
    };

    view! {
        <main class="page">
            <header class="head">
                <img class="mark" src="/icons/128.png" alt="" />
                <div class="titles">
                    <h1>"Focus Timer"</h1>
                    <p>"Settings sync across your browsers. History stays on this device."</p>
                </div>
            </header>

            <section class="stats">
                <div class="stat"><div class="value">{move || sessions.with(|all| today(all).0)}</div><div class="label">"Sessions today"</div></div>
                <div class="stat"><div class="value">{move || timer::duration_label(sessions.with(|all| today(all).1))}</div><div class="label">"Focused today"</div></div>
                <div class="stat"><div class="value">{move || timer::duration_label(week())}</div><div class="label">"Last 7 days"</div></div>
                <div class="stat"><div class="value">{move || timer::duration_label(total())}</div><div class="label">"All time"</div></div>
            </section>

            <section class="card">
                <div class="card-head"><span class="section-title">"Lengths"</span></div>
                <Stepper label="Focus" hint="How long one focus session runs." unit="min" min=1 max=120 value=focus set=set_focus />
                <Stepper label="Short break" hint="The pause between focus sessions." unit="min" min=1 max=30 value=short set=set_short />
                <Stepper label="Long break" hint="A longer rest after a few rounds." unit="min" min=1 max=60 value=long set=set_long />
                <Stepper label="Long break every" hint="Focus sessions before a long break." unit="" min=1 max=8 value=every set=set_every />
            </section>

            <section class="card">
                <div class="card-head"><span class="section-title">"Flow"</span></div>
                <Toggle label="Start breaks by themselves" hint="When focus ends, the break starts right away." value=breaks set=set_breaks />
                <Toggle label="Start focus by itself" hint="When a break ends, the next focus session starts." value=focus_auto set=set_focus_auto />
                <Toggle label="Notify me" hint="A notification when a session or break ends, with buttons to start or snooze." value=notify set=set_notify />
            </section>

            <section class="card">
                <div class="card-head"><span class="section-title">"When you step away"</span></div>
                <Toggle label="Pause focus when I'm away" hint="Uses the idle API. A locked screen counts as away in Chrome." value=pause set=set_pause />
                <Stepper label="Away after" hint="Minutes without keyboard or mouse input." unit="min" min=1 max=30 value=away set=set_away />
            </section>

            <section class="card">
                <div class="card-head">
                    <span class="section-title">"Recent sessions"</span>
                    <button class="btn ghost danger" on:click=clear disabled=move || sessions.with(|all| all.is_empty())>"Clear history"</button>
                </div>
                {move || {
                    let recent: Vec<Session> = sessions.with(|all| all.iter().rev().take(12).cloned().collect());
                    if recent.is_empty() {
                        return view! {
                            <div class="empty">
                                <strong>"No sessions yet"</strong>
                                <span>"Finished focus sessions and breaks show up here."</span>
                            </div>
                        }.into_any();
                    }
                    view! {
                        <div class="list">
                            {recent.into_iter().map(|session| view! {
                                <div class="item">
                                    <span class="mode-dot" class:break=session.mode.is_break()></span>
                                    <div class="main">
                                        <span class="title">{if session.task.trim().is_empty() { session.mode.name().to_string() } else { session.task.clone() }}</span>
                                        <span class="sub">{when(session.ended_at)}</span>
                                    </div>
                                    <span class="chip">{timer::duration_label(session.minutes)}</span>
                                </div>
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </section>

            <section class="card">
                <div class="setting">
                    <div class="main">
                        <div class="label">"Test notifications"</div>
                        <div class="hint">"If nothing appears, check that your system allows notifications from the browser."</div>
                    </div>
                    <button class="btn" on:click=test>"Send a test"</button>
                </div>
                {move || note.get().map(|result| match result {
                    Ok(text) => view! { <div class="notice ok inset">{text}</div> }.into_any(),
                    Err(text) => view! { <div class="notice danger inset">{text}</div> }.into_any(),
                })}
            </section>
        </main>
    }
}
