mod tidy;
mod ui;

use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::commands::Press;
use oxichrome::context_menus::Click;
use oxichrome::prelude::*;
use oxichrome::side_panel;

use crate::ui::{icon, Footer, TabBrowser, Toast, PANEL, PLUS};

#[extension(
    name = t!(app_name),
    description = t!(app_desc),
    version = "0.3.0",
    default_locale = "en",
    permissions = [Tabs, Storage, SidePanel],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = t!(action_title), shortcut = "Alt+Shift+T" },
    gecko_id = "tab-tidy@oxichrome.dev"
)]
pub struct TabTidy;

#[background]
async fn start() {
    tidy::refresh_badge().await;
}

#[on(tabs::on_updated)]
async fn tab_updated() {
    tidy::refresh_badge().await;
}

#[on(tabs::on_removed)]
async fn tab_removed() {
    tidy::refresh_badge().await;
}

#[command(description = t!(cmd_dupes), shortcut = "Alt+Shift+U")]
async fn close_duplicates() {
    tidy::close_duplicates().await;
}

#[command(description = t!(cmd_move))]
async fn move_tab(press: Press) {
    if let Some(tab_id) = press.tab_id {
        let _ = tidy::move_to_new_window(tab_id).await;
    }
}

#[menu(title = t!(menu_dupes), contexts = [action])]
async fn menu_close_duplicates() {
    tidy::close_duplicates().await;
}

#[submenu(title = t!(app_name), contexts = [page])]
struct TidyMenu;

#[menu(title = t!(menu_move), parent = TidyMenu)]
async fn menu_move(click: Click) {
    if let Some(tab_id) = click.tab_id {
        let _ = tidy::move_to_new_window(tab_id).await;
    }
}

#[menu(title = t!(menu_pin), parent = TidyMenu)]
async fn menu_pin(click: Click) {
    if let Some(tab_id) = click.tab_id {
        let _ = tidy::toggle_pinned(tab_id).await;
    }
}

#[menu(title = t!(menu_close_site), parent = TidyMenu)]
async fn menu_close_site(click: Click) {
    if let Some(url) = click.page_url {
        tidy::close_site(&url, click.tab_id).await;
        tidy::refresh_badge().await;
    }
}

fn header(subtitle: Signal<String>, button: impl IntoView) -> impl IntoView {
    view! {
        <header class="head">
            <img class="mark" src="/icons/48.png" alt="" />
            <div class="titles">
                <h1>{t!(app_name)}</h1>
                <p>{move || subtitle.get()}</p>
            </div>
            {button}
        </header>
    }
}

fn summary() -> Signal<String> {
    let text = RwSignal::new(String::new());
    let load = move || {
        spawn_local(async move {
            let tabs = tidy::all().await;
            let mut windows: Vec<i32> = tabs.iter().map(|tab| tab.window_id).collect();
            windows.sort_unstable();
            windows.dedup();
            text.set(t!(summary, tidy::tabs_label(tabs.len()), tidy::windows_label(windows.len())));
        });
    };
    load();
    let ticker = set_interval_with_handle(load, std::time::Duration::from_millis(1500)).ok();
    on_cleanup(move || {
        if let Some(ticker) = ticker {
            ticker.clear();
        }
    });
    text.into()
}

fn close_popup() {
    if let Some(window) = web_sys::window() {
        let _ = window.close();
    }
}

#[popup(style = ["base.css", "tab-tidy.css"])]
fn Popup() -> impl IntoView {
    let toast = Toast::new();
    let open_panel = side_panel::open_on_click(move |opened| async move {
        match opened {
            Ok(()) => close_popup(),
            Err(err) => toast.show(err.to_string()),
        }
    });
    let button = view! {
        <button class="icon-btn" title=t!(open_panel) aria-label=t!(open_panel) on:click=open_panel>{icon(PANEL)}</button>
    };
    view! {
        <div class="popup">
            {header(summary(), button)}
            <div class="body">
                <TabBrowser switched=Callback::new(|_| close_popup()) toast />
            </div>
            <Footer />
            {toast.view()}
        </div>
    }
}

#[side_panel(title = t!(panel_title), style = ["base.css", "tab-tidy.css"])]
fn Panel() -> impl IntoView {
    let toast = Toast::new();
    let new_window = move |_| {
        spawn_local(async move {
            if let Err(err) = tidy::new_window().await {
                toast.show(err.to_string());
            }
        });
    };
    let button = view! {
        <button class="btn" on:click=new_window>{icon(PLUS)}{t!(new_window)}</button>
    };
    view! {
        <div class="panel">
            {header(summary(), button)}
            <div class="body">
                <TabBrowser switched=Callback::new(|_| {}) toast />
            </div>
            <Footer />
            {toast.view()}
        </div>
    }
}
