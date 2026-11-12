use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

const MAX_URL: usize = 65536;
const DEFAULT_LIMIT: u32 = 100;
const ADDABLE: &[&str] = &["http", "https", "ftp", "ws", "wss", "file"];

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub url: String,
    pub title: String,
    pub last_visit: f64,
    pub visit_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Visit {
    pub visit_id: String,
    pub time: f64,
    pub referrer: Option<String>,
    pub transition: Transition,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Transition {
    Link,
    Typed,
    AutoBookmark,
    AutoSubframe,
    ManualSubframe,
    Generated,
    AutoToplevel,
    FormSubmit,
    Reload,
    Keyword,
    KeywordGenerated,
    Other(String),
}

impl Transition {
    fn from_name(name: &str) -> Self {
        match name {
            "link" => Transition::Link,
            "typed" => Transition::Typed,
            "auto_bookmark" => Transition::AutoBookmark,
            "auto_subframe" => Transition::AutoSubframe,
            "manual_subframe" => Transition::ManualSubframe,
            "generated" => Transition::Generated,
            "auto_toplevel" => Transition::AutoToplevel,
            "form_submit" => Transition::FormSubmit,
            "reload" => Transition::Reload,
            "keyword" => Transition::Keyword,
            "keyword_generated" => Transition::KeywordGenerated,
            other => Transition::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removed {
    All,
    Urls(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Search {
    text: String,
    since: Option<f64>,
    until: Option<f64>,
    limit: u32,
}

impl Default for Search {
    fn default() -> Self {
        Search { text: String::new(), since: None, until: None, limit: DEFAULT_LIMIT }
    }
}

impl Search {
    pub fn new() -> Self {
        Search::default()
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn since(mut self, time: f64) -> Self {
        self.since = Some(time);
        self
    }

    pub fn until(mut self, time: f64) -> Self {
        self.until = Some(time);
        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawItem {
    url: Option<String>,
    title: Option<String>,
    last_visit_time: Option<f64>,
    visit_count: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawVisit {
    visit_id: String,
    visit_time: Option<f64>,
    referring_visit_id: Option<String>,
    transition: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRemoved {
    all_history: bool,
    urls: Option<Vec<String>>,
}

fn unexpected(message: &str) -> OxichromeError {
    OxichromeError::UnexpectedValue(message.to_string())
}

fn count(value: f64) -> Result<u32> {
    if value.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&value) {
        Ok(value as u32)
    } else {
        Err(OxichromeError::UnexpectedValue(format!("expected a visit count, got {value}")))
    }
}

impl RawItem {
    fn item(self) -> Result<Item> {
        Ok(Item {
            url: self.url.ok_or_else(|| unexpected("the history item has no url"))?,
            title: self.title.unwrap_or_default(),
            last_visit: self.last_visit_time.ok_or_else(|| unexpected("the history item has no last visit time"))?,
            visit_count: count(self.visit_count.unwrap_or(0.0))?,
        })
    }
}

impl RawVisit {
    fn visit(self) -> Result<Visit> {
        Ok(Visit {
            time: self.visit_time.ok_or_else(|| unexpected("the visit has no time"))?,
            referrer: self.referring_visit_id.filter(|id| !id.is_empty() && id != "0" && id != "-1"),
            transition: Transition::from_name(self.transition.as_deref().unwrap_or("link")),
            visit_id: self.visit_id,
        })
    }
}

fn parse<T: DeserializeOwned>(value: &JsValue) -> Result<T> {
    crate::json::from_js(value)
}

impl FromEventArg for Item {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        parse::<RawItem>(&value)?.item()
    }
}

impl FromEventArg for Removed {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        let raw: RawRemoved = parse(&value)?;
        Ok(if raw.all_history { Removed::All } else { Removed::Urls(raw.urls.unwrap_or_default()) })
    }
}

impl EventArg<events::HistoryOnVisited, 1> for Item {}
impl EventArg<events::HistoryOnVisitRemoved, 1> for Removed {}

fn invalid(message: String) -> OxichromeError {
    OxichromeError::InvalidArgument(message)
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    match message.as_str() {
        "oxichrome:no-permission" => invalid(
            "the history API isn't available; add `History` to `permissions` in #[oxichrome::extension] (if it's optional, request it first)".to_string(),
        ),
        _ => OxichromeError::Js(JsError::new(JsValue::from_str(&message))),
    }
}

async fn call(op: &str, args: serde_json::Value) -> Result<JsValue> {
    let promise = js_bridge::history_call(op, &args.to_string()).map_err(browser_error)?;
    let text = JsFuture::from(promise).await.map_err(browser_error)?;
    let text = text.as_string().unwrap_or_else(|| "null".to_string());
    Ok(js_sys::JSON::parse(&text)?)
}

fn scheme(url: &str) -> Option<&str> {
    let (scheme, _) = url.split_once(':')?;
    let mut chars = scheme.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
    valid.then_some(scheme)
}

fn check_url(url: &str) -> Result<()> {
    if url.is_empty() {
        return Err(invalid("the url can't be empty".to_string()));
    }
    if let Some(bad) = url.chars().find(|c| c.is_whitespace() || c.is_control()) {
        return Err(invalid(format!("the url `{url}` contains {bad:?}; encode spaces and other special characters, like `%20` for a space")));
    }
    if url.len() > MAX_URL {
        return Err(invalid(format!("the url is {} bytes long; history keeps at most {MAX_URL}", url.len())));
    }
    if scheme(url).is_none() {
        return Err(invalid(format!("`{url}` isn't a full url; start it with a scheme, like `https://`")));
    }
    Ok(())
}

fn check_addable(url: &str) -> Result<()> {
    check_url(url)?;
    let scheme = scheme(url).unwrap_or_default().to_ascii_lowercase();
    if !ADDABLE.contains(&scheme.as_str()) {
        return Err(invalid(format!(
            "`{url}` can't be added to history; both browsers keep only http, https, ftp, ws, wss and file urls"
        )));
    }
    let rest = &url[scheme.len() + 1..];
    let Some(after) = rest.strip_prefix("//") else {
        return Err(invalid(format!("`{url}` isn't a full url; write it like `{scheme}://example.com/`")));
    };
    if scheme != "file" && after.split(['/', '?', '#']).next().unwrap_or_default().is_empty() {
        return Err(invalid(format!("`{url}` has no host; write it like `{scheme}://example.com/`")));
    }
    Ok(())
}

fn check_time(time: f64, what: &str) -> Result<()> {
    if !time.is_finite() || time < 0.0 {
        return Err(invalid(format!("the {what} must be milliseconds since the epoch (0 or more), got {time}")));
    }
    Ok(())
}

fn check_range(start: f64, end: f64, first: &str, second: &str) -> Result<()> {
    check_time(start, first)?;
    check_time(end, second)?;
    if start > end {
        return Err(invalid(format!("the {first} ({start}) is after the {second} ({end})")));
    }
    Ok(())
}

pub async fn search(query: &Search) -> Result<Vec<Item>> {
    if query.limit == 0 {
        return Err(invalid("ask for at least 1 result".to_string()));
    }
    let since = query.since.unwrap_or(0.0);
    check_time(since, "start time")?;
    if let Some(until) = query.until {
        check_range(since, until, "start time", "end time")?;
    }
    let raw: Vec<RawItem> = parse(
        &call(
            "search",
            json!({ "text": query.text, "startTime": since, "endTime": query.until, "maxResults": query.limit.min(i32::MAX as u32) }),
        )
        .await?,
    )?;
    let mut items = raw.into_iter().map(RawItem::item).collect::<Result<Vec<_>>>()?;
    items.sort_by(|a, b| b.last_visit.total_cmp(&a.last_visit));
    Ok(items)
}

pub async fn visits(url: &str) -> Result<Vec<Visit>> {
    check_url(url)?;
    let raw: Vec<RawVisit> = parse(&call("visits", json!({ "url": url })).await?)?;
    let mut visits = raw.into_iter().map(RawVisit::visit).collect::<Result<Vec<_>>>()?;
    visits.sort_by(|a, b| a.time.total_cmp(&b.time));
    Ok(visits)
}

pub async fn add(url: &str) -> Result<()> {
    check_addable(url)?;
    call("add", json!({ "url": url })).await.map(|_| ())
}

pub async fn delete(url: &str) -> Result<()> {
    check_url(url)?;
    call("delete", json!({ "url": url })).await.map(|_| ())
}

pub async fn delete_range(start: f64, end: f64) -> Result<()> {
    check_range(start, end, "start", "end")?;
    call("delete_range", json!({ "startTime": start, "endTime": end })).await.map(|_| ())
}

pub async fn delete_all() -> Result<()> {
    call("delete_all", json!({})).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_and_firefox_items_read_the_same() {
        let chrome: RawItem = serde_json::from_str(r#"{"id":"1","lastVisitTime":1794475597515.045,"title":"","typedCount":1,"url":"https://a.com/","visitCount":2}"#).unwrap();
        let firefox: RawItem = serde_json::from_str(r#"{"id":"lwt3zQginzIP","url":"https://a.com/","title":null,"lastVisitTime":1794475607878,"visitCount":2}"#).unwrap();
        for item in [chrome.item().unwrap(), firefox.item().unwrap()] {
            assert_eq!(item.url, "https://a.com/");
            assert_eq!(item.title, "");
            assert_eq!(item.visit_count, 2);
        }
        let bad: RawItem = serde_json::from_str(r#"{"url":"https://a.com/","visitCount":1.5,"lastVisitTime":1}"#).unwrap();
        assert!(bad.item().is_err());
    }

    #[test]
    fn visits_read_referrers_and_transitions() {
        let visit: RawVisit = serde_json::from_str(r#"{"id":"1","isLocal":true,"referringVisitId":"0","transition":"typed","visitId":"1","visitTime":5}"#).unwrap();
        let visit = visit.visit().unwrap();
        assert_eq!(visit.referrer, None);
        assert_eq!(visit.transition, Transition::Typed);
        let linked: RawVisit = serde_json::from_str(r#"{"visitId":"9","referringVisitId":"4","transition":"brand_new","visitTime":5}"#).unwrap();
        let linked = linked.visit().unwrap();
        assert_eq!(linked.referrer.as_deref(), Some("4"));
        assert_eq!(linked.transition, Transition::Other("brand_new".to_string()));
    }

    #[test]
    fn urls_and_times_are_checked() {
        for good in ["https://a.com/", "http://a.com", "ftp://a.com/x", "wss://a.com/x", "ws://a.com", "file:///tmp/x", "HTTPS://A.COM/"] {
            assert!(check_addable(good).is_ok(), "{good}");
        }
        for bad in ["", "a.com", "https://a.com/a b", "javascript:void(0)", "about:blank", "data:text/plain,hi", "blob:https://a.com/x", "chrome-extension://abc/x.html", "https:a.com", "https://", "https:///x", "https://?q"] {
            assert!(check_addable(bad).is_err(), "{bad:?}");
        }
        assert!(check_url("about:blank").is_ok());
        assert!(check_url("nope").is_err());
        assert!(check_range(1.0, 1.0, "start", "end").is_ok());
        assert!(check_range(2.0, 1.0, "start", "end").is_err());
        assert!(check_range(-1.0, 1.0, "start", "end").is_err());
        assert!(check_range(0.0, f64::INFINITY, "start", "end").is_err());
        assert!(check_range(f64::NAN, 1.0, "start", "end").is_err());
    }
}
