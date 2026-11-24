//! Reading text aloud, in Chrome and Firefox.
//!
//! Chrome uses `chrome.tts` (needs `Tts` in `permissions`); Firefox has no `tts` API, so
//! there oxichrome speaks through the page's Web Speech synthesizer, which needs no
//! permission and works in the background page, popups and other extension pages.
//!
//! ```ignore
//! use oxichrome::tts::{self, Speech};
//!
//! tts::speak(Speech::new("Saved").lang("en-US").rate(1.2)).await?;
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::js_bridge;

/// The longest text Chrome speaks at once.
pub const MAX_TEXT: usize = 32_768;

type OnWord = Rc<RefCell<dyn FnMut(usize)>>;

/// Text to read aloud, and how.
#[derive(Clone, Default)]
pub struct Speech {
    text: String,
    options: Options,
    on_word: Option<OnWord>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Options {
    #[serde(skip_serializing_if = "Option::is_none")]
    voice_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pitch: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    volume: Option<f64>,
    enqueue: bool,
}

impl std::fmt::Debug for Speech {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Speech").field("text", &self.text).field("options", &self.options).field("on_word", &self.on_word.is_some()).finish()
    }
}

impl Speech {
    pub fn new(text: impl Into<String>) -> Self {
        Speech { text: text.into(), ..Default::default() }
    }

    /// A voice by name, from [`voices`].
    pub fn voice(mut self, name: impl Into<String>) -> Self {
        self.options.voice_name = Some(name.into());
        self
    }

    /// The language, like `en-US`; the browser picks a voice for it.
    pub fn lang(mut self, lang: impl Into<String>) -> Self {
        self.options.lang = Some(lang.into());
        self
    }

    /// From 0.1 to 10; 1 is normal speed.
    pub fn rate(mut self, rate: f64) -> Self {
        self.options.rate = Some(rate);
        self
    }

    /// From 0 to 2; 1 is the voice's own pitch.
    pub fn pitch(mut self, pitch: f64) -> Self {
        self.options.pitch = Some(pitch);
        self
    }

    /// From 0 to 1; 1 is the loudest.
    pub fn volume(mut self, volume: f64) -> Self {
        self.options.volume = Some(volume);
        self
    }

    /// Waits for what is already being said, instead of cutting it off.
    pub fn enqueue(mut self) -> Self {
        self.options.enqueue = true;
        self
    }

    /// Runs with the index in the text of each word as it is spoken, for highlighting.
    pub fn on_word(mut self, handler: impl FnMut(usize) + 'static) -> Self {
        self.on_word = Some(Rc::new(RefCell::new(handler)));
        self
    }

    fn check(&self) -> Result<()> {
        if self.text.trim().is_empty() {
            return Err(OxichromeError::InvalidArgument("there is no text to speak".into()));
        }
        if self.text.chars().count() > MAX_TEXT {
            return Err(OxichromeError::LimitExceeded(format!("Chrome speaks at most {MAX_TEXT} characters at once; split the text")));
        }
        let range = |name: &str, value: Option<f64>, low: f64, high: f64| match value {
            Some(value) if !(low..=high).contains(&value) => Err(OxichromeError::InvalidArgument(format!("the {name} is {value}; it goes from {low} to {high}"))),
            _ => Ok(()),
        };
        range("rate", self.options.rate, 0.1, 10.0)?;
        range("pitch", self.options.pitch, 0.0, 2.0)?;
        range("volume", self.options.volume, 0.0, 1.0)
    }
}

/// How a [`speak`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Finished {
    /// All of it was said.
    Spoken,
    /// It was cut off by [`stop`] or by speech that didn't [`enqueue`](Speech::enqueue).
    Interrupted,
    /// It was waiting its turn and was dropped before it started.
    Cancelled,
}

fn no_permission(error: JsValue) -> OxichromeError {
    let text = crate::error::JsError::new(error.clone()).to_string();
    if text == "oxichrome:no-permission" {
        OxichromeError::InvalidArgument("`chrome.tts` isn't available; add `Tts` to `permissions` in #[oxichrome::extension]".into())
    } else {
        error.into()
    }
}

/// Says `speech`, and returns once it is done, cut off or dropped.
pub async fn speak(speech: Speech) -> Result<Finished> {
    speech.check()?;
    let options = crate::json::to_js(&speech.options)?;
    let on_word = speech.on_word.clone();
    let word = Closure::<dyn FnMut(f64)>::new(move |index: f64| {
        if let Some(handler) = &on_word {
            (handler.borrow_mut())(index as usize);
        }
    });
    let promise = js_bridge::tts_speak(&speech.text, &options, word.as_ref().unchecked_ref()).map_err(no_permission)?;
    let ended = JsFuture::from(promise).await;
    drop(word);
    match ended?.as_string().as_deref() {
        Some("end") => Ok(Finished::Spoken),
        Some("interrupted") => Ok(Finished::Interrupted),
        Some("cancelled") => Ok(Finished::Cancelled),
        other => Err(OxichromeError::UnexpectedValue(format!("speech ended with `{other:?}`"))),
    }
}

/// Stops speaking and drops anything waiting.
pub fn stop() -> Result<()> {
    js_bridge::tts_call("stop").map(|_| ()).map_err(no_permission)
}

/// Pauses speech, to carry on with [`resume`].
pub fn pause() -> Result<()> {
    js_bridge::tts_call("pause").map(|_| ()).map_err(no_permission)
}

pub fn resume() -> Result<()> {
    js_bridge::tts_call("resume").map(|_| ()).map_err(no_permission)
}

/// Whether something is being said.
pub async fn is_speaking() -> Result<bool> {
    let value = JsFuture::from(js_sys::Promise::resolve(&js_bridge::tts_call("isSpeaking").map_err(no_permission)?)).await?;
    Ok(value.as_bool().unwrap_or(false))
}

/// A voice to [`Speech::voice`] by name.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    #[serde(rename = "voiceName")]
    pub name: String,
    #[serde(default)]
    pub lang: Option<String>,
    /// Speaks through an online service rather than on this computer.
    #[serde(default)]
    pub remote: bool,
}

/// The voices this browser can speak with.
pub async fn voices() -> Result<Vec<Voice>> {
    let value = JsFuture::from(js_sys::Promise::resolve(&js_bridge::tts_call("getVoices").map_err(no_permission)?)).await?;
    crate::json::from_js(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_is_checked() {
        assert!(Speech::new("hi").rate(2.0).pitch(0.0).volume(1.0).check().is_ok());
        assert!(Speech::new("  ").check().is_err());
        assert!(Speech::new("hi").rate(0.0).check().is_err());
        assert!(Speech::new("hi").pitch(2.5).check().is_err());
        assert!(Speech::new("hi").volume(-0.1).check().is_err());
        assert!(matches!(Speech::new("a".repeat(MAX_TEXT + 1)).check(), Err(OxichromeError::LimitExceeded(_))));
    }
}
