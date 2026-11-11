use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

use crate::error::{OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::tabs;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Match(String),
    Url(String),
    Dim(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Description {
    parts: Vec<Part>,
}

impl Description {
    pub fn new() -> Self {
        Description::default()
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.parts.push(Part::Text(text.into()));
        self
    }

    pub fn matched(mut self, text: impl Into<String>) -> Self {
        self.parts.push(Part::Match(text.into()));
        self
    }

    pub fn url(mut self, text: impl Into<String>) -> Self {
        self.parts.push(Part::Url(text.into()));
        self
    }

    pub fn dim(mut self, text: impl Into<String>) -> Self {
        self.parts.push(Part::Dim(text.into()));
        self
    }

    pub fn plain(&self) -> String {
        self.parts
            .iter()
            .map(|part| match part {
                Part::Text(text) | Part::Match(text) | Part::Url(text) | Part::Dim(text) => clean(text),
            })
            .collect()
    }

    pub fn markup(&self) -> String {
        self.parts
            .iter()
            .map(|part| match part {
                Part::Text(text) => escape(&clean(text)),
                Part::Match(text) => format!("<match>{}</match>", escape(&clean(text))),
                Part::Url(text) => format!("<url>{}</url>", escape(&clean(text))),
                Part::Dim(text) => format!("<dim>{}</dim>", escape(&clean(text))),
            })
            .collect()
    }
}

impl From<&str> for Description {
    fn from(text: &str) -> Self {
        Description::new().text(text)
    }
}

impl From<String> for Description {
    fn from(text: String) -> Self {
        Description::new().text(text)
    }
}

fn clean(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    content: String,
    description: Description,
}

impl Suggestion {
    pub fn new(content: impl Into<String>, description: impl Into<Description>) -> Self {
        Suggestion { content: content.into(), description: description.into() }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn description(&self) -> &Description {
        &self.description
    }
}

#[derive(Serialize)]
struct Prepared {
    content: String,
    markup: String,
    plain: String,
}

#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "the omnibox function gets the typed text as `String`, not `{Self}`",
    label = "use `input: String` here"
)]
pub trait __Input {
    fn __from_text(text: String) -> Self;
}

impl __Input for String {
    fn __from_text(text: String) -> Self {
        text
    }
}

#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "the omnibox function returns `Vec<omnibox::Suggestion>`, not `{Self}`",
    label = "return `Vec<omnibox::Suggestion>` here",
    note = "build each one with `Suggestion::new(content, description)`; `content` is what `omnibox::on_input_entered` gets when it's picked"
)]
pub trait __Output {
    fn __into_list(self) -> Vec<Suggestion>;
}

impl __Output for Vec<Suggestion> {
    fn __into_list(self) -> Vec<Suggestion> {
        self
    }
}

#[doc(hidden)]
pub fn __prepare(fn_name: &str, suggestions: Vec<Suggestion>) -> JsValue {
    let mut prepared = Vec::with_capacity(suggestions.len());
    for suggestion in suggestions {
        let plain = suggestion.description.plain();
        if suggestion.content.trim().is_empty() {
            crate::log!("[oxichrome] `{fn_name}` returned a suggestion with empty content (\"{plain}\"); browsers need something to send to `omnibox::on_input_entered`, so it was left out");
            continue;
        }
        if plain.trim().is_empty() {
            crate::log!("[oxichrome] `{fn_name}` returned a suggestion with an empty description (content \"{}\"); browsers refuse it, so it was left out", suggestion.content);
            continue;
        }
        prepared.push(Prepared { content: suggestion.content, markup: suggestion.description.markup(), plain });
    }
    crate::json::to_js(&prepared).unwrap_or(JsValue::NULL)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum Disposition {
    CurrentTab,
    NewForegroundTab,
    NewBackgroundTab,
    #[serde(other)]
    Other,
}

impl FromEventArg for Disposition {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        crate::json::from_js(&value)
    }
}

impl EventArg<events::OmniboxOnInputEntered, 1> for String {}
impl EventArg<events::OmniboxOnInputChanged, 1> for String {}
impl EventArg<events::OmniboxOnInputEntered, 2> for Disposition {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveTab {
    active: bool,
    current_window: bool,
}

#[derive(Serialize)]
struct NewTab<'a> {
    url: &'a str,
    active: bool,
}

#[derive(Serialize)]
struct Navigate<'a> {
    url: &'a str,
}

#[derive(Deserialize)]
struct Found {
    id: Option<i32>,
}

fn check_url(url: &str) -> Result<()> {
    let scheme = url.split_once(':').map(|(scheme, _)| scheme).unwrap_or("");
    let valid = scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !valid {
        return Err(OxichromeError::InvalidArgument(format!(
            "`{url}` isn't a full URL; start it with a scheme like `https://`, or build a search URL from the text"
        )));
    }
    if scheme.eq_ignore_ascii_case("javascript") {
        return Err(OxichromeError::InvalidArgument(format!(
            "`{url}` is a `javascript:` URL; browsers don't open those from extensions"
        )));
    }
    Ok(())
}

pub async fn open(url: &str, disposition: Disposition) -> Result<()> {
    check_url(url)?;
    match disposition {
        Disposition::NewForegroundTab | Disposition::NewBackgroundTab => {
            let _: tabs::Tab = tabs::create(&NewTab { url, active: disposition == Disposition::NewForegroundTab }).await?;
        }
        Disposition::CurrentTab | Disposition::Other => {
            let found: Vec<Found> = tabs::query(&ActiveTab { active: true, current_window: true }).await?;
            match found.first().and_then(|tab| tab.id) {
                Some(id) => {
                    let _: tabs::Tab = tabs::update(id, &Navigate { url }).await?;
                }
                None => {
                    let _: tabs::Tab = tabs::create(&NewTab { url, active: true }).await?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptions_are_safe_for_both_browsers() {
        let description = Description::new().text("Search for ").matched("a & b <c>").text(" on ").url("docs.rs").dim(" \"1.0\"\n");
        assert_eq!(description.markup(), "Search for <match>a &amp; b &lt;c&gt;</match> on <url>docs.rs</url><dim> &quot;1.0&quot; </dim>");
        assert_eq!(description.plain(), "Search for a & b <c> on docs.rs \"1.0\" ");
        assert_eq!(Description::from("it's").markup(), "it&apos;s");
        assert_eq!(Suggestion::new("x", "y").description().plain(), "y");
    }

    #[test]
    fn open_needs_a_full_url() {
        for good in ["https://docs.rs", "http://127.0.0.1:8000/x?q=a b", "about:blank", "moz-extension://abc/x.html"] {
            assert!(check_url(good).is_ok(), "{good}");
        }
        for bad in ["plain words", "", " https://x", "docs.rs/serde", "/path", "1http://x", "JavaScript:alert(1)", "javascript:void(0)"] {
            assert!(check_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn reads_dispositions() {
        for (text, want) in [("\"currentTab\"", Disposition::CurrentTab), ("\"newForegroundTab\"", Disposition::NewForegroundTab), ("\"newBackgroundTab\"", Disposition::NewBackgroundTab), ("\"split\"", Disposition::Other)] {
            assert_eq!(serde_json::from_str::<Disposition>(text).unwrap(), want);
        }
    }
}
