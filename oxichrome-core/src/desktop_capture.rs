//! Asking the user for a screen, window or tab to record. Chrome only, with
//! `DesktopCapture` in `permissions`.
//!
//! [`choose`] shows Chrome's picker and waits for the user; [`cancel`] closes it. Open the
//! chosen stream with [`open`] in an extension page or offscreen document. Firefox has no
//! such API; there, pages ask the user with `getDisplayMedia`.

use std::cell::Cell;

use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::js_bridge;
pub use crate::tab_capture::Media;

/// What the picker offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Source {
    Screen,
    Window,
    Tab,
    /// The sound too, where the system allows it.
    Audio,
}

impl Source {
    fn name(self) -> &'static str {
        match self {
            Source::Screen => "screen",
            Source::Window => "window",
            Source::Tab => "tab",
            Source::Audio => "audio",
        }
    }
}

/// What the user picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub stream_id: String,
    /// Whether [`open`] can ask for the sound too.
    pub can_request_audio: bool,
}

thread_local! {
    static OPEN: Cell<Option<i32>> = const { Cell::new(None) };
}

fn chrome_only(what: &str) -> Result<()> {
    if js_bridge::is_firefox() {
        return Err(OxichromeError::InvalidArgument(format!(
            "Firefox has no desktop capture, so `desktop_capture::{what}` is Chrome only; let a page ask the user with `getDisplayMedia` there"
        )));
    }
    Ok(())
}

/// Shows Chrome's picker with these sources, for `tab` (whose page will use the stream;
/// `None` for the extension itself). `Ok(None)` when the user (or [`cancel`]) closes it.
pub async fn choose(sources: &[Source], tab: Option<i32>) -> Result<Option<Chosen>> {
    chrome_only("choose")?;
    if !sources.iter().any(|source| *source != Source::Audio) {
        return Err(OxichromeError::InvalidArgument("offer at least one of `Screen`, `Window` or `Tab`".into()));
    }
    let names = js_sys::Array::from_iter(sources.iter().map(|source| JsValue::from_str(source.name())));
    let request = js_bridge::desktop_capture_choose(&names, tab).map_err(|error| crate::tab_capture::capture_error(error, "DesktopCapture"))?;
    let request = JsFuture::from(request).await.map_err(|error| crate::tab_capture::capture_error(error, "DesktopCapture"))?;
    let id = js_sys::Reflect::get(&request, &"id".into()).ok().and_then(|id| id.as_f64()).map(|id| id as i32);
    OPEN.with(|open| open.set(id));
    let done = js_sys::Reflect::get(&request, &"done".into()).map_err(OxichromeError::from)?;
    let answer = JsFuture::from(js_sys::Promise::from(done)).await;
    OPEN.with(|open| if open.get() == id { open.set(None) });
    let answer = answer?;
    let stream_id = js_sys::Reflect::get(&answer, &"streamId".into()).ok().and_then(|id| id.as_string()).unwrap_or_default();
    if stream_id.is_empty() {
        return Ok(None);
    }
    let can_request_audio = js_sys::Reflect::get(&answer, &"canRequestAudioTrack".into()).ok().and_then(|value| value.as_bool()).unwrap_or(false);
    Ok(Some(Chosen { stream_id, can_request_audio }))
}

/// Closes the picker [`choose`] opened, if it is still open.
pub fn cancel() -> Result<()> {
    chrome_only("cancel")?;
    if let Some(id) = OPEN.with(|open| open.take()) {
        js_bridge::desktop_capture_cancel(id).map_err(|error| crate::tab_capture::capture_error(error, "DesktopCapture"))?;
    }
    Ok(())
}

/// Opens what the user chose. Stop its tracks to end the capture.
pub async fn open(chosen: &Chosen, media: Media) -> Result<web_sys::MediaStream> {
    chrome_only("open")?;
    if media.audio && !chosen.can_request_audio {
        return Err(OxichromeError::InvalidArgument("the user's choice has no sound to record; offer `Source::Audio` and check `can_request_audio`".into()));
    }
    crate::tab_capture::open_stream("desktop", &chosen.stream_id, media).await
}
