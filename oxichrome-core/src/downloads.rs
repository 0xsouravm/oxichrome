use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    InProgress,
    Paused,
    Complete,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum Interrupt {
    FileFailed,
    FileAccessDenied,
    FileNoSpace,
    FileNameTooLong,
    FileTooLarge,
    FileVirusInfected,
    FileTransientError,
    FileBlocked,
    FileSecurityCheckFailed,
    FileTooShort,
    FileHashMismatch,
    FileSameAsSource,
    NetworkFailed,
    NetworkTimeout,
    NetworkDisconnected,
    NetworkServerDown,
    NetworkInvalidRequest,
    ServerFailed,
    ServerNoRange,
    ServerBadContent,
    ServerUnauthorized,
    ServerCertProblem,
    ServerForbidden,
    ServerUnreachable,
    ServerContentLengthMismatch,
    ServerCrossOriginRedirect,
    UserCanceled,
    UserShutdown,
    Crash,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum Danger {
    Safe,
    Accepted,
    File,
    Url,
    Content,
    Uncommon,
    Host,
    Unwanted,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "Raw")]
#[non_exhaustive]
pub struct Download {
    pub id: i32,
    pub url: String,
    pub final_url: Option<String>,
    pub filename: Option<String>,
    pub mime: Option<String>,
    pub state: State,
    pub error: Option<Interrupt>,
    pub can_resume: bool,
    pub danger: Danger,
    pub bytes_received: u64,
    pub total_bytes: Option<u64>,
    pub file_size: Option<u64>,
    pub exists: bool,
    pub start_time: String,
    pub end_time: Option<String>,
    pub incognito: bool,
    pub referrer: Option<String>,
    pub by_extension_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    id: i32,
    url: String,
    final_url: Option<String>,
    filename: Option<String>,
    mime: Option<String>,
    state: String,
    paused: Option<bool>,
    can_resume: Option<bool>,
    error: Option<String>,
    danger: Option<Danger>,
    bytes_received: Option<f64>,
    total_bytes: Option<f64>,
    file_size: Option<f64>,
    exists: Option<bool>,
    start_time: Option<String>,
    end_time: Option<String>,
    incognito: Option<bool>,
    referrer: Option<String>,
    by_extension_id: Option<String>,
}

fn filled(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.is_empty())
}

fn size(value: Option<f64>) -> Option<u64> {
    value.filter(|value| *value >= 0.0).map(|value| value as u64)
}

impl From<Raw> for Download {
    fn from(raw: Raw) -> Self {
        let paused = raw.paused.unwrap_or(false);
        let can_resume = raw.can_resume.unwrap_or(false);
        let state = match raw.state.as_str() {
            "complete" => State::Complete,
            "in_progress" if paused => State::Paused,
            "interrupted" if paused && can_resume => State::Paused,
            "in_progress" => State::InProgress,
            _ => State::Interrupted,
        };
        let error = match state {
            State::Interrupted => Some(
                raw.error
                    .map(|error| serde_json::from_value(serde_json::Value::String(error)).unwrap_or(Interrupt::Other))
                    .unwrap_or(Interrupt::Other),
            ),
            _ => None,
        };
        let total_bytes = size(raw.total_bytes).filter(|total| *total > 0);
        let file_size = size(raw.file_size);
        let mut bytes_received = size(raw.bytes_received).unwrap_or(0);
        if state == State::Complete {
            bytes_received = file_size.filter(|size| *size > 0).or(total_bytes).unwrap_or(bytes_received);
        }
        Download {
            id: raw.id,
            url: raw.url,
            final_url: filled(raw.final_url),
            filename: filled(raw.filename),
            mime: filled(raw.mime),
            state,
            error,
            can_resume: state == State::Paused || can_resume,
            danger: raw.danger.unwrap_or(Danger::Safe),
            bytes_received,
            total_bytes,
            file_size,
            exists: raw.exists.unwrap_or(false),
            start_time: raw.start_time.unwrap_or_default(),
            end_time: filled(raw.end_time),
            incognito: raw.incognito.unwrap_or(false),
            referrer: filled(raw.referrer),
            by_extension_id: filled(raw.by_extension_id),
        }
    }
}

impl FromEventArg for Download {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        crate::json::from_js(&value)
    }
}

impl EventArg<events::DownloadsOnCreated, 1> for Download {}
impl EventArg<events::DownloadsOnChanged, 1> for Download {}
impl EventArg<events::DownloadsOnErased, 1> for i32 {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Conflict {
    Uniquify,
    Overwrite,
    #[serde(rename = "prompt")]
    Ask,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    url: String,
    filename: Option<String>,
    conflict: Option<Conflict>,
    ask: bool,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

impl Request {
    pub fn new(url: impl Into<String>) -> Self {
        Request { url: url.into(), filename: None, conflict: None, ask: false, headers: Vec::new(), body: None }
    }

    pub fn filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    pub fn on_conflict(mut self, conflict: Conflict) -> Self {
        self.conflict = Some(conflict);
        self
    }

    pub fn ask_where_to_save(mut self) -> Self {
        self.ask = true;
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn post(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    fn check(&self) -> Result<()> {
        check_url(&self.url)?;
        if let Some(filename) = &self.filename {
            check_filename(filename)?;
        }
        for (name, value) in &self.headers {
            check_header(name, value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Save {
    filename: String,
    bytes: Vec<u8>,
    mime: Option<String>,
    conflict: Option<Conflict>,
    ask: bool,
}

impl Save {
    pub fn bytes(filename: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Save { filename: filename.into(), bytes: bytes.into(), mime: None, conflict: None, ask: false }
    }

    pub fn text(filename: impl Into<String>, text: impl Into<String>) -> Self {
        Save::bytes(filename, text.into().into_bytes())
    }

    pub fn mime(mut self, mime: impl Into<String>) -> Self {
        self.mime = Some(mime.into());
        self
    }

    pub fn on_conflict(mut self, conflict: Conflict) -> Self {
        self.conflict = Some(conflict);
        self
    }

    pub fn ask_where_to_save(mut self) -> Self {
        self.ask = true;
        self
    }

    fn check(&self) -> Result<String> {
        check_filename(&self.filename)?;
        let mime = match &self.mime {
            Some(mime) => {
                let valid = mime.split_once('/').is_some_and(|(kind, sub)| {
                    !kind.is_empty() && !sub.is_empty() && mime.chars().all(|c| c.is_ascii_graphic() || c == ' ') && !mime.contains(',')
                });
                if !valid {
                    return Err(OxichromeError::InvalidArgument(format!(
                        "`{mime}` isn't a MIME type; write it like `text/plain` or `application/json`"
                    )));
                }
                mime.clone()
            }
            None => guess_mime(&self.filename).to_string(),
        };
        Ok(mime)
    }
}

fn guess_mime(filename: &str) -> &'static str {
    let ext = filename.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "txt" | "log" => "text/plain;charset=utf-8",
        "md" => "text/markdown;charset=utf-8",
        "csv" => "text/csv;charset=utf-8",
        "json" => "application/json",
        "html" | "htm" => "text/html;charset=utf-8",
        "css" => "text/css",
        "js" => "text/javascript",
        "xml" => "application/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

fn check_url(url: &str) -> Result<()> {
    let scheme = url.split_once(':').map(|(scheme, _)| scheme.to_ascii_lowercase()).unwrap_or_default();
    let valid_scheme = scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !valid_scheme {
        return Err(OxichromeError::InvalidArgument(format!("`{url}` isn't a full URL; start it with `https://`")));
    }
    match scheme.as_str() {
        "http" | "https" => {
            let host = url[scheme.len() + 1..].strip_prefix("//").unwrap_or("");
            if host.is_empty() || host.starts_with('/') {
                return Err(OxichromeError::InvalidArgument(format!("`{url}` has no host; write it like `https://example.com/file.zip`")));
            }
            if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(OxichromeError::InvalidArgument(format!("`{url}` contains spaces or line breaks; encode them (a space is `%20`)")));
            }
            Ok(())
        }
        "data" | "blob" => Err(OxichromeError::InvalidArgument(format!(
            "`{scheme}:` URLs work in only one browser each; to save content you made, use `downloads::save(Save::bytes(...))`"
        ))),
        _ => Err(OxichromeError::InvalidArgument(format!(
            "`{url}` can't be downloaded; extensions can download `http` and `https` URLs in both browsers (to save content you made, use `downloads::save`)"
        ))),
    }
}

const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4",
    "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

fn check_filename(filename: &str) -> Result<()> {
    let bad = |why: String| Err(OxichromeError::InvalidArgument(format!("the file name `{filename}` {why}")));
    if filename.trim().is_empty() {
        return Err(OxichromeError::InvalidArgument("the file name is empty".to_string()));
    }
    let bytes = filename.as_bytes();
    let drive = bytes.len() > 1 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && (bytes.len() == 2 || matches!(bytes[2], b'/' | b'\\'));
    if filename.starts_with('/') || filename.starts_with('\\') || filename.starts_with('~') || drive {
        return bad("is an absolute path; give a path inside the downloads folder, like `reports/today.csv`".to_string());
    }
    for part in filename.split('/') {
        if part.is_empty() {
            return bad("has an empty folder name; remove the doubled or trailing `/`".to_string());
        }
        if part == "." || part == ".." {
            return bad("can't contain `.` or `..` folders; downloads stay inside the downloads folder".to_string());
        }
        if part.starts_with('.') {
            return bad(format!("has `{part}`, which starts with a dot; Chrome refuses hidden files"));
        }
        if part.ends_with('.') || part.ends_with(' ') || part.starts_with(' ') {
            return bad(format!("has `{part}`, which starts or ends with a space or ends with a dot; browsers refuse those"));
        }
        if let Some(c) = part.chars().find(|c| matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()) {
            return bad(format!("contains {c:?}, which file systems don't allow; use `/` only between folders"));
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_lowercase();
        if RESERVED.contains(&stem.as_str()) {
            return bad(format!("uses `{stem}`, a name Windows reserves; Chrome refuses it on every system"));
        }
        let lower = part.to_ascii_lowercase();
        if lower.ends_with(".lnk") || lower.ends_with(".local") {
            return bad("ends with `.lnk` or `.local`; Chrome refuses those".to_string());
        }
        if part.len() > 255 {
            return bad(format!("has a part {} bytes long; file systems allow 255", part.len()));
        }
    }
    Ok(())
}

const FORBIDDEN_HEADERS: &[&str] = &[
    "accept-charset",
    "accept-encoding",
    "access-control-request-headers",
    "access-control-request-method",
    "access-control-request-private-network",
    "connection",
    "content-length",
    "cookie",
    "cookie2",
    "date",
    "dnt",
    "expect",
    "host",
    "keep-alive",
    "origin",
    "referer",
    "set-cookie",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "user-agent",
    "via",
];

fn check_header(name: &str, value: &str) -> Result<()> {
    let token = |c: char| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c);
    if name.is_empty() || !name.chars().all(token) {
        return Err(OxichromeError::InvalidArgument(format!(
            "`{name}` isn't a header name; use letters, digits and `-`, like `X-Api-Key`"
        )));
    }
    let lower = name.to_ascii_lowercase();
    if FORBIDDEN_HEADERS.contains(&lower.as_str()) || lower.starts_with("sec-") || lower.starts_with("proxy-") {
        return Err(OxichromeError::InvalidArgument(format!(
            "the browser sets `{name}` itself and refuses to let extensions change it"
        )));
    }
    if value.chars().any(|c| matches!(c, '\r' | '\n' | '\0')) {
        return Err(OxichromeError::InvalidArgument(format!("the value of `{name}` has a line break; headers are one line")));
    }
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    text: Vec<String>,
    without: Vec<String>,
    filename: Option<String>,
    filename_regex: Option<String>,
    url: Option<String>,
    url_regex: Option<String>,
    mime: Option<String>,
    state: Option<State>,
    limit: Option<usize>,
    oldest_first: bool,
}

impl Query {
    pub fn new() -> Self {
        Query::default()
    }

    pub fn text(mut self, word: impl Into<String>) -> Self {
        self.text.push(word.into());
        self
    }

    pub fn without(mut self, word: impl Into<String>) -> Self {
        self.without.push(word.into());
        self
    }

    pub fn filename(mut self, path: impl Into<String>) -> Self {
        self.filename = Some(path.into());
        self
    }

    pub fn filename_regex(mut self, pattern: impl Into<String>) -> Self {
        self.filename_regex = Some(pattern.into());
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn url_regex(mut self, pattern: impl Into<String>) -> Self {
        self.url_regex = Some(pattern.into());
        self
    }

    pub fn mime(mut self, mime: impl Into<String>) -> Self {
        self.mime = Some(mime.into());
        self
    }

    pub fn state(mut self, state: State) -> Self {
        self.state = Some(state);
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn oldest_first(mut self) -> Self {
        self.oldest_first = true;
        self
    }

    fn check(&self) -> Result<()> {
        if self.limit == Some(0) {
            return Err(OxichromeError::InvalidArgument("the limit is 0, so nothing would match; leave it out to get every download".to_string()));
        }
        if let Some(word) = self.text.iter().chain(&self.without).find(|word| word.trim().is_empty()) {
            return Err(OxichromeError::InvalidArgument(format!("the search word `{word}` is empty")));
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawQuery<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i32>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    query: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename_regex: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url_regex: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime: Option<&'a str>,
    order_by: [&'static str; 1],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawOptions<'a> {
    url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conflict_action: Option<Conflict>,
    save_as: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    headers: Vec<RawHeader<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
}

#[derive(Serialize)]
struct RawHeader<'a> {
    name: &'a str,
    value: &'a str,
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    let lower = message.to_ascii_lowercase();
    if message == "oxichrome:no-permission" {
        return OxichromeError::InvalidArgument(
            "the downloads API isn't available; add `Downloads` to `permissions` in #[oxichrome::extension] (if it's optional, request it first)".to_string(),
        );
    }
    if message == "oxichrome:no-open-permission" || lower.contains("downloads.open") {
        return OxichromeError::InvalidArgument(
            "opening downloads needs `DownloadsOpen` in `permissions` in #[oxichrome::extension]".to_string(),
        );
    }
    if lower.contains("user gesture") || lower.contains("user input handler") {
        return OxichromeError::InvalidArgument(format!(
            "{message} (use the handler `downloads::open_on_click` returns, so the browser sees the click)"
        ));
    }
    if lower.contains("not_resumable") || lower.contains("not resumable") {
        return OxichromeError::InvalidArgument(
            "the server doesn't support resuming, so Firefox stopped the download when it was paused; start it again".to_string(),
        );
    }
    if lower.contains("canceled by the user") || lower.contains("download canceled") {
        return canceled();
    }
    if lower.contains("already deleted") || lower.contains("file doesn't exist") || lower.contains("file does not exist") {
        return gone();
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

async fn call<T: serde::de::DeserializeOwned>(method: &str, args: &[JsValue]) -> Result<T> {
    let list = js_sys::Array::new();
    for arg in args {
        list.push(arg);
    }
    let value = JsFuture::from(js_bridge::downloads_call(method, &list).map_err(browser_error)?).await.map_err(browser_error)?;
    if value.is_undefined() {
        return crate::json::from_js(&JsValue::NULL);
    }
    crate::json::from_js(&value)
}

async fn done(method: &str, args: &[JsValue]) -> Result<()> {
    let _: Option<serde::de::IgnoredAny> = call(method, args).await?;
    Ok(())
}

fn not_found(id: i32) -> OxichromeError {
    OxichromeError::InvalidArgument(format!("there's no download with id {id} (it was erased, or the id is wrong)"))
}

pub async fn download(request: Request) -> Result<i32> {
    request.check()?;
    let options = crate::json::to_js(&RawOptions {
        url: &request.url,
        filename: request.filename.as_deref(),
        conflict_action: request.conflict,
        save_as: request.ask,
        headers: request.headers.iter().map(|(name, value)| RawHeader { name, value }).collect(),
        method: request.body.as_ref().map(|_| "POST"),
        body: request.body.as_deref(),
    })?;
    let id = call("download", &[options]).await?;
    if request.ask { picked(id).await } else { Ok(id) }
}

async fn pause_briefly() {
    let wait = js_sys::Promise::new(&mut |resolve, _| {
        if let Ok(set_timeout) = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("setTimeout")) {
            let _ = js_sys::Function::from(set_timeout).call2(&JsValue::NULL, &resolve, &JsValue::from(100));
        }
    });
    let _ = JsFuture::from(wait).await;
}

async fn picked(id: i32) -> Result<i32> {
    loop {
        let download = get(id).await?;
        if download.filename.is_some() {
            return Ok(id);
        }
        match download.state {
            State::InProgress => pause_briefly().await,
            State::Interrupted if download.error == Some(Interrupt::UserCanceled) => {
                let _ = erase(id).await;
                return Err(canceled());
            }
            _ => return Ok(id),
        }
    }
}

fn gone() -> OxichromeError {
    OxichromeError::InvalidArgument(
        "the downloaded file is no longer on disk (it was deleted or moved), so there's nothing to open or show".to_string(),
    )
}

fn canceled() -> OxichromeError {
    OxichromeError::InvalidArgument("the download was canceled (the save dialog was closed)".to_string())
}

pub async fn save(save: Save) -> Result<i32> {
    let mime = save.check()?;
    let conflict = save.conflict.map(|conflict| match conflict {
        Conflict::Uniquify => "uniquify",
        Conflict::Overwrite => "overwrite",
        Conflict::Ask => "prompt",
    });
    let promise = js_bridge::downloads_save(&save.filename, save.bytes, &mime, conflict, save.ask).map_err(browser_error)?;
    let value = JsFuture::from(promise).await.map_err(browser_error)?;
    let id = crate::json::from_js(&value)?;
    if save.ask { picked(id).await } else { Ok(id) }
}

async fn find(id: i32) -> Result<Option<Download>> {
    let query = crate::json::to_js(&RawQuery {
        id: Some(id),
        query: &[],
        filename: None,
        filename_regex: None,
        url: None,
        url_regex: None,
        mime: None,
        order_by: ["-startTime"],
    })?;
    let found: Vec<Download> = call("search", &[query]).await?;
    Ok(found.into_iter().next())
}

pub async fn get(id: i32) -> Result<Download> {
    find(id).await?.ok_or_else(|| not_found(id))
}

pub async fn search(query: Query) -> Result<Vec<Download>> {
    query.check()?;
    let raw = crate::json::to_js(&RawQuery {
        id: None,
        query: &query.text,
        filename: query.filename.as_deref(),
        filename_regex: query.filename_regex.as_deref(),
        url: query.url.as_deref(),
        url_regex: query.url_regex.as_deref(),
        mime: query.mime.as_deref(),
        order_by: [if query.oldest_first { "startTime" } else { "-startTime" }],
    })?;
    let found: Vec<Download> = call("search", &[raw]).await?;
    let without: Vec<String> = query.without.iter().map(|word| word.to_lowercase()).collect();
    Ok(found
        .into_iter()
        .filter(|download| query.state.is_none_or(|state| download.state == state))
        .filter(|download| {
            let filename = download.filename.as_deref().unwrap_or("").to_lowercase();
            let url = download.url.to_lowercase();
            !without.iter().any(|word| filename.contains(word) || url.contains(word))
        })
        .take(query.limit.unwrap_or(usize::MAX))
        .collect())
}

pub async fn pause(id: i32) -> Result<()> {
    let download = get(id).await?;
    match download.state {
        State::Paused => Ok(()),
        State::InProgress => done("pause", &[id.into()]).await,
        state => Err(OxichromeError::InvalidArgument(format!("download {id} is {}, so there's nothing to pause", describe(state)))),
    }
}

pub async fn resume(id: i32) -> Result<()> {
    let download = get(id).await?;
    match download.state {
        State::InProgress => Ok(()),
        State::Paused => done("resume", &[id.into()]).await,
        state => Err(OxichromeError::InvalidArgument(format!(
            "download {id} is {}, so it can't be resumed; start a new download instead",
            describe(state)
        ))),
    }
}

pub async fn cancel(id: i32) -> Result<()> {
    let download = get(id).await?;
    match download.state {
        State::InProgress | State::Paused => done("cancel", &[id.into()]).await,
        State::Complete | State::Interrupted => Ok(()),
    }
}

pub async fn erase(id: i32) -> Result<()> {
    let query = crate::json::to_js(&serde_json::json!({ "id": id }))?;
    let _: Vec<i32> = call("erase", &[query]).await?;
    Ok(())
}

pub async fn erase_matching(query: Query) -> Result<Vec<i32>> {
    let mut erased = Vec::new();
    for download in search(query).await? {
        erase(download.id).await?;
        erased.push(download.id);
    }
    Ok(erased)
}

pub async fn remove_file(id: i32) -> Result<()> {
    let download = get(id).await?;
    if download.state != State::Complete {
        return Err(OxichromeError::InvalidArgument(format!(
            "download {id} is {}; only a finished download has a file to remove (use `cancel` to stop one)",
            describe(download.state)
        )));
    }
    done("removeFile", &[id.into()]).await
}

pub async fn show(id: i32) -> Result<()> {
    get(id).await?;
    done("show", &[id.into()]).await
}

pub async fn show_default_folder() -> Result<()> {
    done("showDefaultFolder", &[]).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSize {
    Small,
    Large,
}

pub async fn file_icon(id: i32, size: IconSize) -> Result<String> {
    get(id).await?;
    let options = crate::json::to_js(&serde_json::json!({ "size": match size { IconSize::Small => 16, IconSize::Large => 32 } }))?;
    let icon: Option<String> = call("getFileIcon", &[id.into(), options]).await?;
    icon.ok_or_else(|| OxichromeError::UnexpectedValue(format!("the browser has no icon for download {id}")))
}

fn describe(state: State) -> &'static str {
    match state {
        State::InProgress => "still downloading",
        State::Paused => "paused",
        State::Complete => "already finished",
        State::Interrupted => "stopped (canceled or failed)",
    }
}

pub fn open_on_click<E, F, Fut>(id: i32, then: F) -> impl FnMut(E) + 'static
where
    E: AsRef<web_sys::Event>,
    F: FnMut(Result<()>) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    let then = Rc::new(RefCell::new(then));
    move |event: E| {
        let started: Result<js_sys::Promise> = if !event.as_ref().is_trusted() {
            Err(OxichromeError::InvalidArgument(
                "only a real click or key press can open a download; this event was created by code".to_string(),
            ))
        } else {
            js_bridge::downloads_open(id).map_err(browser_error)
        };
        let then = then.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = match started {
                Ok(promise) => match JsFuture::from(promise).await {
                    Ok(_) => Ok(()),
                    Err(error) => {
                        let error = browser_error(error);
                        match find(id).await {
                            Ok(None) => Err(not_found(id)),
                            Ok(Some(download)) if download.state != State::Complete => Err(OxichromeError::InvalidArgument(format!(
                                "download {id} is {}; only a finished download can be opened",
                                describe(download.state)
                            ))),
                            Ok(Some(_)) if matches!(error, OxichromeError::Js(_)) => Err(gone()),
                            _ => Err(error),
                        }
                    }
                },
                Err(err) => Err(err),
            };
            let future = (then.borrow_mut())(result);
            future.await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(result: Result<()>) -> String {
        match result.unwrap_err() {
            OxichromeError::InvalidArgument(message) => message,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn filenames_follow_both_browsers() {
        for good in ["a.txt", "sub/dir/a.txt", "résumé ✓.txt", "100%.txt", "a#b.txt", "plain", "x.tar.gz"] {
            assert!(check_filename(good).is_ok(), "{good}");
        }
        for (bad, why) in [
            ("", "empty"),
            ("   ", "empty"),
            ("/tmp/a.txt", "absolute"),
            ("~/a.txt", "absolute"),
            ("C:/a.txt", "absolute"),
            ("../a.txt", "`..`"),
            ("a/./b.txt", "`..`"),
            ("a//b.txt", "empty folder"),
            ("dir/", "empty folder"),
            (".hidden", "dot"),
            ("a/.git/x", "dot"),
            ("dot.", "ends with"),
            ("a .txt ", "ends with"),
            ("a:b.txt", "':'"),
            ("a\\b.txt", "'\\\\'"),
            ("a\u{1}b", "'\\u{1}'"),
            ("a\"b", "'\"'"),
            ("con.txt", "Windows"),
            ("NUL", "Windows"),
            ("dir/lpt1.log", "Windows"),
            ("a.lnk", ".lnk"),
        ] {
            let message = err(check_filename(bad));
            assert!(message.contains(why), "{bad}: {message}");
        }
        assert!(err(check_filename(&"x".repeat(300))).contains("255"));
    }

    #[test]
    fn urls_and_headers_follow_both_browsers() {
        for good in ["https://example.com/a.zip", "http://127.0.0.1:8000/x?q=1"] {
            assert!(check_url(good).is_ok(), "{good}");
        }
        for (bad, why) in [
            ("file.txt", "full URL"),
            ("", "full URL"),
            ("https:/x", "no host"),
            ("https://", "no host"),
            ("https://a.com/a b", "spaces"),
            ("data:text/plain,hi", "downloads::save"),
            ("blob:https://a/1", "downloads::save"),
            ("file:///etc/hosts", "http"),
            ("javascript:1", "http"),
            ("about:blank", "http"),
            ("ftp://a.com/x", "http"),
        ] {
            let message = err(check_url(bad));
            assert!(message.contains(why), "{bad}: {message}");
        }
        assert!(check_header("X-Api-Key", "abc").is_ok());
        for name in ["Cookie", "referer", "User-Agent", "Origin", "Sec-Fetch-Mode", "Proxy-Authorization", "Host"] {
            assert!(err(check_header(name, "1")).contains("sets"), "{name}");
        }
        assert!(err(check_header("a b", "1")).contains("isn't a header name"));
        assert!(err(check_header("", "1")).contains("isn't a header name"));
        assert!(err(check_header("X-A", "a\nb")).contains("line break"));
    }

    #[test]
    fn save_needs_a_mime_type_that_works() {
        assert_eq!(Save::text("a.txt", "x").check().unwrap(), "text/plain;charset=utf-8");
        assert_eq!(Save::bytes("a.bin", vec![1]).check().unwrap(), "application/octet-stream");
        assert_eq!(Save::bytes("a.PNG", vec![1]).mime("image/png").check().unwrap(), "image/png");
        assert!(err(Save::bytes("a", vec![]).mime("png").check().map(|_| ())).contains("MIME"));
        assert!(err(Save::bytes("a", vec![]).mime("text/plain,x").check().map(|_| ())).contains("MIME"));
        assert!(err(Save::text("../a.txt", "x").check().map(|_| ())).contains("`..`"));
        assert!(err(Query::new().limit(0).check()).contains("limit"));
        assert!(err(Query::new().text(" ").check()).contains("empty"));
        assert!(err(Query::new().without("").check()).contains("empty"));
    }

    #[test]
    fn reads_both_browsers_the_same_way() {
        let chrome: Download = serde_json::from_str(r#"{"id":4,"url":"https://a/x","finalUrl":"https://b/x","filename":"/d/x","mime":"text/plain","state":"in_progress","paused":true,"canResume":true,"danger":"safe","bytesReceived":10,"totalBytes":100,"fileSize":-1,"exists":true,"startTime":"t","incognito":false,"referrer":""}"#).unwrap();
        let firefox: Download = serde_json::from_str(r#"{"id":4,"url":"https://a/x","filename":"/d/x","mime":"text/plain","state":"interrupted","error":"USER_CANCELED","paused":true,"canResume":true,"danger":"safe","bytesReceived":10,"totalBytes":100,"fileSize":0,"exists":false,"startTime":"t","endTime":null,"incognito":false,"cookieStoreId":"firefox-default"}"#).unwrap();
        assert_eq!((chrome.state, chrome.error, firefox.state, firefox.error), (State::Paused, None, State::Paused, None));
        assert_eq!((chrome.referrer, chrome.file_size, chrome.final_url.as_deref()), (None, None, Some("https://b/x")));
        let cancelled: Download = serde_json::from_str(r#"{"id":5,"url":"u","state":"interrupted","error":"USER_CANCELED","paused":true,"canResume":false,"totalBytes":-1}"#).unwrap();
        assert_eq!((cancelled.state, cancelled.error, cancelled.total_bytes), (State::Interrupted, Some(Interrupt::UserCanceled), None));
        let firefox_done: Download = serde_json::from_str(r#"{"id":6,"url":"u","state":"complete","bytesReceived":2555904,"totalBytes":2621440,"fileSize":2621440}"#).unwrap();
        assert_eq!(firefox_done.bytes_received, 2621440);
        let odd: Download = serde_json::from_str(r#"{"id":7,"url":"u","state":"interrupted","error":"SOMETHING_NEW","danger":"deepScannedSafe"}"#).unwrap();
        assert_eq!((odd.error, odd.danger), (Some(Interrupt::Other), Danger::Other));
    }
}
