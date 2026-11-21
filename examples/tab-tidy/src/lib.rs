mod tidy;

use std::future::Future;

use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::context_menus::Click;
use oxichrome::prelude::*;
use oxichrome::runtime::MessageSender;
use oxichrome::sessions::{self, Closed};
use oxichrome::side_panel;
use oxichrome::tabs::{Removed, Tab};
use wasm_bindgen::JsValue;

#[extension(
    name = t!(app_name),
    description = t!(app_desc),
    version = "0.3.0",
    default_locale = "en",
    permissions = [Tabs, TabGroups, Sessions, SidePanel],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = t!(app_name), shortcut = "Alt+Shift+T" },
    gecko_id = "tab-tidy@oxichrome.dev"
)]
pub struct TabTidy;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind")]
enum Notice {
    Changed,
}

async fn changed() {
    tidy::refresh_badge().await;
    let _ = runtime::send_message(&Notice::Changed).await;
}

#[background]
async fn start() {
    tidy::refresh_badge().await;
}

#[on(tabs::on_updated)]
async fn tab_updated(_tab_id: i32, _change: JsValue, tab: Tab) {
    if tab.status.as_deref() != Some("loading") {
        changed().await;
    }
}

#[on(tabs::on_removed)]
async fn tab_removed(_tab_id: i32, info: Removed) {
    if !info.is_window_closing {
        changed().await;
    }
}

#[on(sessions::on_changed)]
async fn sessions_changed() {
    changed().await;
}

#[command(description = t!(close_dupes), shortcut = "Alt+Shift+U")]
async fn close_duplicates() {
    tidy::close_duplicates().await;
}

#[menu(title = t!(close_dupes), contexts = [action])]
async fn menu_close_duplicates() {
    tidy::close_duplicates().await;
}

#[submenu(title = t!(app_name), contexts = [page])]
struct TidyMenu;

#[menu(title = t!(group_sites), parent = TidyMenu)]
async fn menu_group() {
    let _ = tidy::group_by_site().await;
}

#[menu(title = t!(move_window), parent = TidyMenu)]
async fn menu_move(click: Click) {
    if let Some(tab_id) = click.tab_id {
        let _ = tidy::move_to_new_window(tab_id).await;
    }
}

fn close_popup() {
    if let Some(window) = web_sys::window() {
        let _ = window.close();
    }
}

fn act(status: RwSignal<String>, task: impl Future<Output = Result<String>> + 'static) {
    spawn_local(async move {
        status.set(task.await.unwrap_or_else(|err| err.to_string()));
    });
}

fn tab_row(tab: Tab, status: RwSignal<String>, switched: fn()) -> Option<impl IntoView> {
    let (id, window_id) = (tab.id?, tab.window_id);
    let open = move |_| {
        spawn_local(async move {
            if tidy::switch_to(id, window_id).await.is_ok() {
                switched();
            }
        })
    };
    let sleep = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        act(status, async move { tabs::discard(id).await.map(|_| t!(slept)) });
    };
    let close = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        act(status, async move { tabs::remove(id).await.map(|()| String::new()) });
    };
    let asleep = tab.discarded.then(|| view! { " · " {t!(asleep)} });
    Some(view! {
        <div class="item clickable" class:current=tab.active role="button" on:click=open>
            <div class="main">
                <span class="title">{tidy::label(&tab)}</span>
                <span class="sub">{tidy::host(tab.url.as_deref().unwrap_or_default())}{asleep}</span>
            </div>
            <div class="actions">
                {(!tab.active && !tab.discarded).then(|| view! { <button class="btn ghost" on:click=sleep>{t!(sleep)}</button> })}
                <button class="btn ghost" on:click=close>{t!(close)}</button>
            </div>
        </div>
    })
}

fn closed_row(closed: Closed, status: RwSignal<String>) -> impl IntoView {
    let session_id = closed.session_id().to_string();
    let restore = move |_| {
        let session_id = session_id.clone();
        act(status, async move { sessions::restore(&session_id).await.map(|_| String::new()) });
    };
    view! {
        <div class="item">
            <div class="main"><span class="title">{tidy::closed_label(&closed)}</span></div>
            <button class="btn ghost" on:click=restore>{t!(restore)}</button>
        </div>
    }
}

fn tidy_page(button: impl IntoView, switched: fn()) -> impl IntoView {
    let tabs = RwSignal::new(Vec::<Tab>::new());
    let closed = RwSignal::new(Vec::<Closed>::new());
    let query = RwSignal::new(String::new());
    let status = RwSignal::new(String::new());
    let load = move || {
        spawn_local(async move {
            tabs.set(tidy::all().await);
            closed.set(tidy::recently_closed().await);
        })
    };
    load();
    let _ = runtime::listen(move |_: Notice, _: MessageSender| async move { load() });

    let dupes = Memo::new(move |_| tabs.with(|tabs| tidy::duplicates(tabs).len()));
    let visible = Memo::new(move |_| {
        let query = query.get();
        tabs.with(|tabs| tabs.iter().filter(|tab| tidy::matches(tab, &query)).cloned().collect::<Vec<_>>())
    });
    let group = move |_| act(status, async { tidy::group_by_site().await.map(|groups| t!(grouped, groups)) });
    let close_dupes = move |_| act(status, async { Ok(t!(closed_dupes, tidy::close_duplicates().await)) });
    let enter = move |ev: leptos::ev::KeyboardEvent| {
        if let Some((Some(id), window_id)) = visible.get_untracked().first().filter(|_| ev.key() == "Enter").map(|tab| (tab.id, tab.window_id)) {
            spawn_local(async move {
                if tidy::switch_to(id, window_id).await.is_ok() {
                    switched();
                }
            });
        }
    };

    view! {
        <header class="head">
            <img class="mark" src="/icons/48.png" alt="" />
            <div class="titles">
                <h1>{t!(app_name)}</h1>
                <p>{move || t!(summary, tabs.with(Vec::len))}</p>
            </div>
            {button}
        </header>
        <div class="body">
            <label class="field">
                <input type="text" autofocus placeholder=t!(search) aria-label=t!(search) on:input=move |ev| query.set(event_target_value(&ev)) on:keydown=enter />
            </label>
            <div class="row-flex">
                <button class="btn" on:click=group>{t!(group_sites)}</button>
                <Show when=move || dupes.get() != 0>
                    <button class="btn primary" on:click=close_dupes>{move || t!(close_n_dupes, dupes.get())}</button>
                </Show>
                <span class="muted">{move || status.get()}</span>
            </div>
            <section class="card">
                <div class="list">{move || visible.get().into_iter().filter_map(|tab| tab_row(tab, status, switched)).collect_view()}</div>
            </section>
            <Show when=move || closed.with(|closed| !closed.is_empty())>
                <section class="card">
                    <div class="card-head"><span class="section-title">{t!(recently_closed)}</span></div>
                    <div class="list">{move || closed.get().into_iter().map(|item| closed_row(item, status)).collect_view()}</div>
                </section>
            </Show>
        </div>
    }
}

#[popup(style = ["base.css", "tab-tidy.css"])]
fn Popup() -> impl IntoView {
    let open_panel = side_panel::open_on_click(|opened| async move {
        if opened.is_ok() {
            close_popup();
        }
    });
    let button = view! { <button class="btn ghost" on:click=open_panel>{t!(open_panel)}</button> };
    view! { <div class="popup">{tidy_page(button, close_popup)}</div> }
}

#[side_panel(title = t!(app_name), style = ["base.css", "tab-tidy.css"])]
fn Panel() -> impl IntoView {
    let new_window = move |_| {
        spawn_local(async move {
            let _ = tidy::new_window().await;
        })
    };
    let button = view! { <button class="btn ghost" on:click=new_window>{t!(new_window)}</button> };
    view! { <div class="panel">{tidy_page(button, || {})}</div> }
}
