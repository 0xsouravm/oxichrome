use std::time::Duration;

use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

pub const MIN_INTERVAL: Duration = Duration::from_secs(15);
pub const MAX_INTERVAL: Duration = Duration::from_secs(i32::MAX as u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Active,
    Idle,
    Locked,
}

impl State {
    fn parse(text: &str) -> Result<Self> {
        match text {
            "active" => Ok(State::Active),
            "idle" => Ok(State::Idle),
            "locked" => Ok(State::Locked),
            other => Err(OxichromeError::UnexpectedValue(format!("unknown idle state `{other}`"))),
        }
    }
}

impl FromEventArg for State {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        match value.as_string() {
            Some(text) => State::parse(&text),
            None => Err(OxichromeError::UnexpectedValue(format!("expected an idle state, got {value:?}"))),
        }
    }
}

impl EventArg<events::IdleOnStateChanged, 1> for State {}

fn seconds(interval: Duration) -> Result<i32> {
    if interval.subsec_nanos() != 0 {
        return Err(OxichromeError::InvalidArgument(format!(
            "the idle interval {interval:?} must be whole seconds"
        )));
    }
    if interval < MIN_INTERVAL {
        return Err(OxichromeError::InvalidArgument(format!(
            "the idle interval {interval:?} is too short; both browsers need at least 15 seconds"
        )));
    }
    if interval > MAX_INTERVAL {
        return Err(OxichromeError::InvalidArgument(format!(
            "the idle interval {interval:?} is too long; Chrome takes at most {} seconds",
            i32::MAX
        )));
    }
    Ok(interval.as_secs() as i32)
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    if message == "oxichrome:no-permission" {
        return OxichromeError::InvalidArgument(
            "the idle API isn't available; add `Idle` to `permissions` in #[oxichrome::extension] (if it's optional, request it first)".to_string(),
        );
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

pub async fn state(idle_after: Duration) -> Result<State> {
    let promise = js_bridge::idle_call("queryState", seconds(idle_after)?).map_err(browser_error)?;
    let value = JsFuture::from(promise).await.map_err(browser_error)?;
    State::from_event_arg(value)
}

pub async fn set_detection_interval(idle_after: Duration) -> Result<()> {
    let promise = js_bridge::idle_call("setDetectionInterval", seconds(idle_after)?).map_err(browser_error)?;
    JsFuture::from(promise).await.map_err(browser_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_follow_both_browsers() {
        assert_eq!(seconds(Duration::from_secs(15)).unwrap(), 15);
        assert_eq!(seconds(MAX_INTERVAL).unwrap(), i32::MAX);
        for (bad, why) in [
            (Duration::from_secs(14), "at least 15"),
            (Duration::ZERO, "at least 15"),
            (Duration::from_millis(15_500), "whole seconds"),
            (MAX_INTERVAL + Duration::from_secs(1), "at most"),
        ] {
            match seconds(bad).unwrap_err() {
                OxichromeError::InvalidArgument(message) => assert!(message.contains(why), "{bad:?}: {message}"),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn states_parse() {
        assert_eq!(State::parse("active").unwrap(), State::Active);
        assert_eq!(State::parse("idle").unwrap(), State::Idle);
        assert_eq!(State::parse("locked").unwrap(), State::Locked);
        assert!(State::parse("away").is_err());
    }
}
