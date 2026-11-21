use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use oxichrome::permissions::PermissionSet;
use oxichrome::prelude::*;
use oxichrome::runtime::MessageSender;
use oxichrome::user_scripts::{self, Availability, UserScript, WorldConfig};

#[extension(
    name = "Snippets",
    description = "Your own bits of JavaScript, each one running on the site you pick.",
    version = "0.3.0",
    permissions = [Storage],
    optional_permissions = [UserScripts],
    optional_host_permissions = ["*://*/*"],
    icons = { 16 = "icons/16.png", 32 = "icons/32.png", 48 = "icons/48.png", 128 = "icons/128.png" },
    action = { default_title = "Snippets" },
    gecko_id = "snippets@oxichrome.dev"
)]
pub struct Snippets;

const SNIPPETS: &str = "snippets";
const RUNS: &str = "runs";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snippet {
    pub site: String,
    pub code: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub count: u32,
    pub last_url: String,
}

#[derive(Serialize, Deserialize)]
struct Ran {
    snippet: String,
}

pub fn site(input: &str) -> std::result::Result<String, &'static str> {
    let rest = input.trim().split_once("://").map_or(input.trim(), |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").trim_start_matches("*.").to_lowercase();
    if host.is_empty() {
        return Err("Type a site, like github.com");
    }
    if host.contains(':') {
        return Err("Leave the port out: a site matches on every port");
    }
    if !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.') {
        return Err("A site has only letters, digits, dots and dashes");
    }
    Ok(host)
}

fn pattern(site: &str) -> String {
    if site.chars().all(|c| c.is_ascii_digit() || c == '.') {
        format!("*://{site}/*")
    } else {
        format!("*://*.{site}/*")
    }
}

impl Snippet {
    fn script(&self) -> UserScript {
        UserScript::new(&self.site)
            .matches(pattern(&self.site))
            .code(&self.code)
            .code(format!("chrome.runtime.sendMessage({{ snippet: \"{}\" }});", self.site))
    }
}

async fn load<T: serde::de::DeserializeOwned + Default>(key: &str) -> T {
    storage::get(key).await.ok().flatten().unwrap_or_default()
}

async fn restore() -> Result<()> {
    if user_scripts::availability().await != Availability::Ready {
        return Ok(());
    }
    user_scripts::configure_world(&WorldConfig { messaging: true, ..Default::default() }).await?;
    let registered: Vec<String> = user_scripts::get_scripts().await?.into_iter().map(|script| script.id).collect();
    let snippets: Vec<Snippet> = load(SNIPPETS).await;
    let missing: Vec<UserScript> = snippets.iter().filter(|s| !registered.contains(&s.site)).map(Snippet::script).collect();
    user_scripts::register(&missing).await
}

async fn save(snippet: Snippet) -> Result<Vec<Snippet>> {
    let script = snippet.script();
    if user_scripts::get(&snippet.site).await?.is_some() {
        user_scripts::update(&[script]).await?;
    } else {
        user_scripts::register(&[script]).await?;
    }
    let mut snippets: Vec<Snippet> = load(SNIPPETS).await;
    snippets.retain(|s| s.site != snippet.site);
    snippets.push(snippet);
    storage::set(SNIPPETS, &snippets).await?;
    Ok(snippets)
}

async fn delete(site: String) -> Result<Vec<Snippet>> {
    user_scripts::unregister(&[&site]).await?;
    let mut snippets: Vec<Snippet> = load(SNIPPETS).await;
    snippets.retain(|s| s.site != site);
    storage::set(SNIPPETS, &snippets).await?;
    Ok(snippets)
}

#[background]
async fn start() {
    if let Err(err) = restore().await {
        oxichrome::log!("snippets: {err}");
    }
}

#[on(permissions::on_added)]
async fn granted(_set: PermissionSet) {
    if let Err(err) = restore().await {
        oxichrome::log!("snippets: {err}");
    }
}

#[on(runtime::on_user_script_message)]
async fn ran(message: Ran, sender: MessageSender) {
    let mut runs: HashMap<String, Run> = load(RUNS).await;
    let run = runs.entry(message.snippet).or_default();
    run.count += 1;
    run.last_url = sender.url.unwrap_or_default();
    let _ = storage::set(RUNS, &runs).await;
}

fn run_line(run: Option<&Run>) -> String {
    match run {
        Some(run) => format!("Ran {} {}, last on {}", run.count, if run.count == 1 { "time" } else { "times" }, run.last_url),
        None => "Not run yet".into(),
    }
}

#[on(action::on_clicked)]
async fn clicked() {
    let _ = runtime::open_options_page().await;
}

#[options_page(style = ["base.css", "snippets.css"])]
fn Options() -> impl IntoView {
    let state = RwSignal::new(None::<Availability>);
    let snippets = RwSignal::new(Vec::<Snippet>::new());
    let runs = RwSignal::new(HashMap::<String, Run>::new());
    let site_input = RwSignal::new(String::new());
    let code = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let check = move || spawn_local(async move {
        let _ = restore().await;
        state.set(Some(user_scripts::availability().await));
    });
    check();
    spawn_local(async move {
        snippets.set(load(SNIPPETS).await);
        runs.set(load(RUNS).await);
    });
    let done = move |result: Result<Vec<Snippet>>| match result {
        Ok(list) => {
            snippets.set(list);
            error.set(None);
        }
        Err(err) => error.set(Some(err.to_string())),
    };
    let add = move |ev: leptos::ev::MouseEvent| {
        let snippet = match site(&site_input.get_untracked()) {
            Ok(site) => Snippet { site, code: code.get_untracked() },
            Err(reason) => return error.set(Some(reason.into())),
        };
        let access = PermissionSet::new().origin(pattern(&snippet.site));
        permissions::request_on_click(access, move |granted| {
            let snippet = snippet.clone();
            async move {
                match granted {
                    Ok(true) => done(save(snippet).await),
                    Ok(false) => error.set(Some(format!("Snippets needs access to {} to run there", snippet.site))),
                    Err(err) => error.set(Some(err.to_string())),
                }
            }
        })(ev);
    };
    let allow = move || permissions::request_on_click(Permission::UserScripts, move |_| async move { check() });
    view! {
        <main class="page">
            <header class="head">
                <img class="mark" src="/icons/128.png" alt="" />
                <div class="titles">
                    <h1>"Snippets"</h1>
                    <p>"Each snippet runs on every page of its site, after the page loads."</p>
                </div>
            </header>
            <section class="card">
                {move || match state.get() {
                    None => view! { <div class="pad muted">"Checking..."</div> }.into_any(),
                    Some(Availability::Ready) => view! { <div class="notice ok status">"Snippets are on."</div> }.into_any(),
                    Some(Availability::NeedsPermission) => view! {
                        <div class="setting status">
                            <div class="main">
                                <div class="label">"Allow user scripts"</div>
                                <div class="hint">"Snippets need the user scripts permission. In Firefox you can also grant it in about:addons, Snippets, Permissions."</div>
                            </div>
                            <button class="btn primary" on:click=allow()>"Allow"</button>
                        </div>
                    }.into_any(),
                    Some(Availability::NeedsUserToggle) => view! {
                        <div class="setting status">
                            <div class="main">
                                <div class="label">"Turn on Allow User Scripts"</div>
                                <div class="hint">"Chrome also needs a switch: open chrome://extensions, click Details under Snippets and turn on Allow User Scripts."</div>
                            </div>
                            <button class="btn" on:click=move |_| check()>"Check again"</button>
                        </div>
                    }.into_any(),
                }}
            </section>
            <section class="card">
                <div class="pad stack">
                    <label class="field">
                        <input id="site" type="text" placeholder="github.com" aria-label="Site" prop:value=move || site_input.get() on:input=move |ev| site_input.set(event_target_value(&ev)) />
                    </label>
                    <textarea id="code" class="area mono" placeholder="document.body.style.zoom = 1.1;" aria-label="Code" prop:value=move || code.get() on:input=move |ev| code.set(event_target_value(&ev))></textarea>
                    <button id="save" class="btn primary" type="button" on:click=add disabled=move || state.get() != Some(Availability::Ready)>"Save snippet"</button>
                </div>
                {move || error.get().map(|message| view! { <div class="notice danger">{message}</div> })}
            </section>
            <section class="card">
                <div class="list">
                    {move || snippets.get().into_iter().map(|s| {
                        let (title, site, edit, gone) = (s.site.clone(), s.site.clone(), s.clone(), s.site.clone());
                        view! {
                            <div class="item clickable" on:click=move |_| { site_input.set(edit.site.clone()); code.set(edit.code.clone()); }>
                                <div class="main">
                                    <div class="title">{title}</div>
                                    <div class="sub mono">{s.code.lines().next().unwrap_or("").to_string()}</div>
                                    <div class="sub">{move || runs.with(|r| run_line(r.get(&site)))}</div>
                                </div>
                                <button class="btn ghost danger" on:click=move |ev| { ev.stop_propagation(); let site = gone.clone(); spawn_local(async move { done(delete(site).await) }); }>"Remove"</button>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </section>
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_from_what_people_type() {
        assert_eq!(site(" GitHub.com "), Ok("github.com".into()));
        assert_eq!(site("https://news.ycombinator.com/item?id=1"), Ok("news.ycombinator.com".into()));
        assert_eq!(site("*.example.org"), Ok("example.org".into()));
        assert!(site("").is_err());
        assert!(site("localhost:8080").is_err());
        assert!(site("my site").is_err());
    }

    #[test]
    fn patterns_cover_subdomains_but_not_ips() {
        assert_eq!(pattern("github.com"), "*://*.github.com/*");
        assert_eq!(pattern("127.0.0.1"), "*://127.0.0.1/*");
    }

    #[test]
    fn run_lines() {
        assert_eq!(run_line(None), "Not run yet");
        let run = Run { count: 1, last_url: "https://example.com/".into() };
        assert_eq!(run_line(Some(&run)), "Ran 1 time, last on https://example.com/");
    }
}
