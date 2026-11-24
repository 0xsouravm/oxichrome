//! Chrome's built-in AI: a language model to prompt, a summarizer, a translator and a
//! language detector, all running on the user's computer.
//!
//! Chrome only, and only from extension pages (popup, side panel, options, offscreen), not
//! the background service worker. Firefox has none of it: there every `availability` is
//! [`Availability::Unavailable`] and every `create` fails saying so. A model may have to be
//! downloaded first ([`Availability::Downloadable`]); Chrome starts that only from a click,
//! so call `create` from a click handler and follow it with `on_download`.
//!
//! ```ignore
//! use oxichrome::ai::{self, Availability, Prompt};
//!
//! if ai::Session::availability(&Prompt::new()).await? != Availability::Unavailable {
//!     let session = ai::Session::create(Prompt::new().system("Answer in one sentence.")).await?;
//!     let answer = session.prompt("What is a tab group?").await?;
//! }
//! ```

use std::rc::Rc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::js_bridge;

/// Whether a model can be used here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Availability {
    /// Not on this browser, computer or for these options.
    Unavailable,
    /// Usable once downloaded; `create` (from a click) starts the download.
    Downloadable,
    Downloading,
    Available,
}

type Progress = Option<Rc<dyn Fn(f64)>>;

async fn availability(api: &str, options: JsValue) -> Result<Availability> {
    let value = JsFuture::from(js_bridge::ai_availability(api, &options)).await?;
    crate::json::from_js(&value)
}

async fn create(api: &str, options: JsValue, progress: &Progress) -> Result<JsValue> {
    let progress = progress.clone();
    let on_progress = Closure::<dyn Fn(f64)>::new(move |loaded: f64| {
        if let Some(handler) = &progress {
            handler(loaded);
        }
    });
    let created = JsFuture::from(js_bridge::ai_create(api, &options, on_progress.as_ref().unchecked_ref())).await;
    drop(on_progress);
    Ok(created?)
}

async fn call(object: &JsValue, method: &str, args: &[JsValue]) -> Result<JsValue> {
    let args = js_sys::Array::from_iter(args);
    Ok(JsFuture::from(js_bridge::ai_call(object, method, &args)).await?)
}

async fn text(object: &JsValue, method: &str, args: &[JsValue]) -> Result<String> {
    call(object, method, args).await?.as_string().ok_or_else(|| OxichromeError::UnexpectedValue(format!("`{method}` gave back no text")))
}

async fn stream(object: &JsValue, method: &str, args: &[JsValue], mut on_chunk: impl FnMut(&str) + 'static) -> Result<String> {
    let args = js_sys::Array::from_iter(args);
    let chunk = Closure::<dyn FnMut(String)>::new(move |piece: String| on_chunk(&piece));
    let full = JsFuture::from(js_bridge::ai_stream(object, method, &args, chunk.as_ref().unchecked_ref())).await;
    drop(chunk);
    Ok(full?.as_string().unwrap_or_default())
}

fn js<T: Serialize>(value: &T) -> Result<JsValue> {
    crate::json::to_js(value)
}

fn check_languages(languages: &[String]) -> Result<()> {
    for language in languages {
        let ok = !language.is_empty()
            && language.split('-').all(|part| !part.is_empty() && part.len() <= 8 && part.chars().all(|c| c.is_ascii_alphanumeric()));
        if !ok {
            return Err(OxichromeError::InvalidArgument(format!("`{language}` isn't a language tag like `en` or `pt-BR`")));
        }
    }
    Ok(())
}

// The language model

/// How to set up a [`Session`].
#[derive(Clone, Default)]
pub struct Prompt {
    options: PromptOptions,
    progress: Progress,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptOptions {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    initial_prompts: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    expected_inputs: Vec<Expected>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    expected_outputs: Vec<Expected>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Expected {
    r#type: &'static str,
    languages: Vec<String>,
}

/// Who said a line of the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Message {
    role: Role,
    content: String,
}

impl std::fmt::Debug for Prompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prompt").field("options", &self.options).finish()
    }
}

impl Prompt {
    pub fn new() -> Self {
        Self::default()
    }

    /// Instructions the model follows for the whole session.
    pub fn system(mut self, text: impl Into<String>) -> Self {
        self.options.initial_prompts.retain(|message| message.role != Role::System);
        self.options.initial_prompts.insert(0, Message { role: Role::System, content: text.into() });
        self
    }

    /// An earlier turn, to start the conversation from (examples, or a saved chat).
    pub fn turn(mut self, role: Role, text: impl Into<String>) -> Self {
        self.options.initial_prompts.push(Message { role, content: text.into() });
        self
    }

    /// How random the answers are; set it with [`top_k`](Self::top_k). See [`Session::params`].
    pub fn temperature(mut self, temperature: f64) -> Self {
        self.options.temperature = Some(temperature);
        self
    }

    pub fn top_k(mut self, top_k: u32) -> Self {
        self.options.top_k = Some(top_k);
        self
    }

    /// The languages prompts will be in, like `["en", "ja"]`.
    pub fn input_languages<S: Into<String>>(mut self, languages: impl IntoIterator<Item = S>) -> Self {
        self.options.expected_inputs = vec![Expected { r#type: "text", languages: languages.into_iter().map(Into::into).collect() }];
        self
    }

    /// The languages answers should be in.
    pub fn output_languages<S: Into<String>>(mut self, languages: impl IntoIterator<Item = S>) -> Self {
        self.options.expected_outputs = vec![Expected { r#type: "text", languages: languages.into_iter().map(Into::into).collect() }];
        self
    }

    /// Runs with how much of the model has downloaded, from 0 to 1.
    pub fn on_download(mut self, handler: impl Fn(f64) + 'static) -> Self {
        self.progress = Some(Rc::new(handler));
        self
    }

    fn check(&self) -> Result<()> {
        if self.options.temperature.is_some() != self.options.top_k.is_some() {
            return Err(OxichromeError::InvalidArgument("Chrome takes `temperature` and `top_k` together; set both or neither".into()));
        }
        if self.options.temperature.is_some_and(|t| t.is_nan() || t < 0.0) {
            return Err(OxichromeError::InvalidArgument("the temperature can't be below 0".into()));
        }
        if self.options.top_k == Some(0) {
            return Err(OxichromeError::InvalidArgument("`top_k` is at least 1".into()));
        }
        let mut systems = self.options.initial_prompts.iter().filter(|message| message.role == Role::System);
        if systems.next().is_some() && systems.next().is_some() {
            return Err(OxichromeError::InvalidArgument("a session has one system prompt".into()));
        }
        for expected in self.options.expected_inputs.iter().chain(&self.options.expected_outputs) {
            check_languages(&expected.languages)?;
        }
        Ok(())
    }
}

/// The model's limits and defaults.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    pub default_top_k: u32,
    pub max_top_k: u32,
    pub default_temperature: f64,
    pub max_temperature: f64,
}

/// A conversation with the built-in language model. It remembers every prompt and answer
/// until its context fills ([`Session::usage`]); [`Session::fork`] copies it.
#[derive(Debug)]
pub struct Session {
    inner: JsValue,
}

impl Session {
    pub async fn availability(prompt: &Prompt) -> Result<Availability> {
        prompt.check()?;
        availability("LanguageModel", js(&prompt.options)?).await
    }

    /// The model's temperature and top-k limits, or `None` where there is no model.
    pub async fn params() -> Result<Option<Params>> {
        let value = JsFuture::from(js_bridge::ai_params()).await?;
        if value.is_null() || value.is_undefined() {
            return Ok(None);
        }
        Ok(Some(crate::json::from_js(&value)?))
    }

    pub async fn create(prompt: Prompt) -> Result<Session> {
        prompt.check()?;
        Ok(Session { inner: create("LanguageModel", js(&prompt.options)?, &prompt.progress).await? })
    }

    /// Asks, and returns the whole answer.
    pub async fn prompt(&self, text: &str) -> Result<String> {
        self::text(&self.inner, "prompt", &[text.into()]).await
    }

    /// Asks, runs `on_chunk` with each piece of the answer as it is written, and returns the
    /// whole answer.
    pub async fn prompt_streaming(&self, text: &str, on_chunk: impl FnMut(&str) + 'static) -> Result<String> {
        stream(&self.inner, "promptStreaming", &[text.into()], on_chunk).await
    }

    /// Asks for an answer that fits `schema` (a JSON schema), and reads it as `T`.
    pub async fn prompt_json<T: DeserializeOwned>(&self, text: &str, schema: &serde_json::Value) -> Result<T> {
        let options = js(&serde_json::json!({ "responseConstraint": schema }))?;
        let answer = self::text(&self.inner, "prompt", &[text.into(), options]).await?;
        serde_json::from_str(&answer).map_err(|error| OxichromeError::Serde(format!("the model's answer isn't the JSON asked for ({error}): {answer}")))
    }

    /// Adds to the conversation without asking for an answer.
    pub async fn append(&self, role: Role, text: &str) -> Result<()> {
        let message = js(&[Message { role, content: text.into() }])?;
        call(&self.inner, "append", &[message]).await.map(|_| ())
    }

    /// How much of the context is used, and how much there is, in tokens.
    pub fn usage(&self) -> (u64, u64) {
        let number = |name: &str| js_sys::Reflect::get(&self.inner, &name.into()).ok().and_then(|value| value.as_f64()).unwrap_or(0.0) as u64;
        (number("inputUsage"), number("inputQuota"))
    }

    /// How many tokens `text` would take.
    pub async fn measure(&self, text: &str) -> Result<u64> {
        Ok(call(&self.inner, "measureInputUsage", &[text.into()]).await?.as_f64().unwrap_or(0.0) as u64)
    }

    /// A copy of the session, conversation and all, to go a different way from here.
    pub async fn fork(&self) -> Result<Session> {
        Ok(Session { inner: call(&self.inner, "clone", &[]).await? })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        js_bridge::ai_destroy(&self.inner);
    }
}

// The summarizer

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SummaryKind {
    #[default]
    KeyPoints,
    Tldr,
    Teaser,
    Headline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    #[default]
    Markdown,
    PlainText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Length {
    Short,
    #[default]
    Medium,
    Long,
}

/// How to set up a [`Summarizer`].
#[derive(Clone, Default)]
pub struct Summary {
    options: SummaryOptions,
    progress: Progress,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SummaryOptions {
    r#type: SummaryKind,
    format: Format,
    length: Length,
    #[serde(skip_serializing_if = "Option::is_none")]
    shared_context: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    expected_input_languages: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_language: Option<String>,
}

impl std::fmt::Debug for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Summary").field("options", &self.options).finish()
    }
}

impl Summary {
    pub fn new(kind: SummaryKind) -> Self {
        Summary { options: SummaryOptions { r#type: kind, ..Default::default() }, progress: None }
    }

    pub fn format(mut self, format: Format) -> Self {
        self.options.format = format;
        self
    }

    pub fn length(mut self, length: Length) -> Self {
        self.options.length = length;
        self
    }

    /// Background that applies to every text, like "articles from a cooking blog".
    pub fn context(mut self, context: impl Into<String>) -> Self {
        self.options.shared_context = Some(context.into());
        self
    }

    pub fn input_languages<S: Into<String>>(mut self, languages: impl IntoIterator<Item = S>) -> Self {
        self.options.expected_input_languages = languages.into_iter().map(Into::into).collect();
        self
    }

    pub fn output_language(mut self, language: impl Into<String>) -> Self {
        self.options.output_language = Some(language.into());
        self
    }

    pub fn on_download(mut self, handler: impl Fn(f64) + 'static) -> Self {
        self.progress = Some(Rc::new(handler));
        self
    }

    fn check(&self) -> Result<()> {
        check_languages(&self.options.expected_input_languages)?;
        check_languages(self.options.output_language.as_slice())
    }
}

#[derive(Debug)]
pub struct Summarizer {
    inner: JsValue,
}

impl Summarizer {
    pub async fn availability(summary: &Summary) -> Result<Availability> {
        summary.check()?;
        availability("Summarizer", js(&summary.options)?).await
    }

    pub async fn create(summary: Summary) -> Result<Summarizer> {
        summary.check()?;
        Ok(Summarizer { inner: create("Summarizer", js(&summary.options)?, &summary.progress).await? })
    }

    pub async fn summarize(&self, text: &str) -> Result<String> {
        self::text(&self.inner, "summarize", &[text.into()]).await
    }

    pub async fn summarize_streaming(&self, text: &str, on_chunk: impl FnMut(&str) + 'static) -> Result<String> {
        stream(&self.inner, "summarizeStreaming", &[text.into()], on_chunk).await
    }
}

impl Drop for Summarizer {
    fn drop(&mut self) {
        js_bridge::ai_destroy(&self.inner);
    }
}

// The translator

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Pair {
    source_language: String,
    target_language: String,
}

fn pair(from: &str, to: &str) -> Result<Pair> {
    check_languages(&[from.to_string(), to.to_string()])?;
    Ok(Pair { source_language: from.into(), target_language: to.into() })
}

/// Translates from one language to another, like `Translator::create("en", "fr")`.
#[derive(Debug)]
pub struct Translator {
    inner: JsValue,
}

impl Translator {
    pub async fn availability(from: &str, to: &str) -> Result<Availability> {
        availability("Translator", js(&pair(from, to)?)?).await
    }

    pub async fn create(from: &str, to: &str) -> Result<Translator> {
        Self::create_with_progress(from, to, |_| {}).await
    }

    /// [`create`](Self::create), running `on_download` with how much of the language pack
    /// has downloaded, from 0 to 1.
    pub async fn create_with_progress(from: &str, to: &str, on_download: impl Fn(f64) + 'static) -> Result<Translator> {
        let progress: Progress = Some(Rc::new(on_download));
        Ok(Translator { inner: create("Translator", js(&pair(from, to)?)?, &progress).await? })
    }

    pub async fn translate(&self, text: &str) -> Result<String> {
        self::text(&self.inner, "translate", &[text.into()]).await
    }

    pub async fn translate_streaming(&self, text: &str, on_chunk: impl FnMut(&str) + 'static) -> Result<String> {
        stream(&self.inner, "translateStreaming", &[text.into()], on_chunk).await
    }
}

impl Drop for Translator {
    fn drop(&mut self) {
        js_bridge::ai_destroy(&self.inner);
    }
}

// The language detector

/// A language the text may be in, most likely first.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Detected {
    #[serde(rename = "detectedLanguage")]
    pub language: String,
    pub confidence: f64,
}

#[derive(Debug)]
pub struct LanguageDetector {
    inner: JsValue,
}

impl LanguageDetector {
    pub async fn availability() -> Result<Availability> {
        availability("LanguageDetector", JsValue::from(js_sys::Object::new())).await
    }

    pub async fn create() -> Result<LanguageDetector> {
        Ok(LanguageDetector { inner: create("LanguageDetector", JsValue::from(js_sys::Object::new()), &None).await? })
    }

    pub async fn detect(&self, text: &str) -> Result<Vec<Detected>> {
        let value = call(&self.inner, "detect", &[text.into()]).await?;
        crate::json::from_js(&value)
    }
}

impl Drop for LanguageDetector {
    fn drop(&mut self) {
        js_bridge::ai_destroy(&self.inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_are_checked() {
        assert!(Prompt::new().system("a").temperature(0.5).top_k(3).input_languages(["en", "pt-BR"]).check().is_ok());
        assert!(Prompt::new().temperature(0.5).check().is_err());
        assert!(Prompt::new().temperature(-1.0).top_k(1).check().is_err());
        assert!(Prompt::new().temperature(1.0).top_k(0).check().is_err());
        assert!(Prompt::new().input_languages(["english language"]).check().is_err());
        assert!(Prompt::new().turn(Role::System, "a").turn(Role::System, "b").check().is_err());
        assert_eq!(Prompt::new().system("a").system("b").options.initial_prompts.len(), 1);
        assert!(Summary::new(SummaryKind::Tldr).output_language("en").check().is_ok());
        assert!(Summary::new(SummaryKind::Tldr).output_language("").check().is_err());
        assert!(pair("en", "x y").is_err());
    }
}
