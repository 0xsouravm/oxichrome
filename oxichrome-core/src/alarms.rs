use std::time::Duration;

use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::event::FromEventArg;
use crate::js_bridge;

pub const MAX_TIME_MS: u64 = 8_640_000_000_000_000;

pub const MIN_PERIOD: Duration = Duration::from_secs(1);

pub const MAX_NAME_BYTES: usize = 1024;

pub const MAX_ALARMS: usize = 500;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "BrowserAlarm")]
pub struct Alarm {
    pub name: String,
    pub scheduled_time: u64,
    pub period: Option<Duration>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserAlarm {
    name: String,
    scheduled_time: f64,
    period_in_minutes: Option<f64>,
}

impl From<BrowserAlarm> for Alarm {
    fn from(alarm: BrowserAlarm) -> Self {
        Alarm {
            name: alarm.name,
            scheduled_time: alarm.scheduled_time.round() as u64,
            period: alarm
                .period_in_minutes
                .map(|minutes| Duration::from_millis((minutes * 60_000.0).round() as u64)),
        }
    }
}

impl FromEventArg for Alarm {
    fn from_event_arg(value: wasm_bindgen::JsValue) -> Result<Self> {
        Ok(serde_wasm_bindgen::from_value(value)?)
    }
}

impl crate::event::EventArg<crate::event::events::AlarmsOnAlarm, 1> for Alarm {}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Start {
    After(Duration),
    At(u64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlarmOptions {
    start: Start,
    period: Option<Duration>,
}

impl AlarmOptions {
    pub fn after(delay: Duration) -> Self {
        AlarmOptions {
            start: Start::After(delay),
            period: None,
        }
    }

    pub fn at(time_ms: u64) -> Self {
        AlarmOptions {
            start: Start::At(time_ms),
            period: None,
        }
    }

    pub fn then_every(self, period: Duration) -> Self {
        AlarmOptions {
            period: Some(period),
            ..self
        }
    }

    fn to_create_info(self, now_ms: f64) -> Result<CreateInfo> {
        let mut info = CreateInfo {
            delay_in_minutes: None,
            when: None,
            period_in_minutes: None,
        };
        match self.start {
            Start::After(delay) => {
                if now_ms + delay.as_millis() as f64 > MAX_TIME_MS as f64 {
                    return Err(invalid(format!(
                        "alarm delay of {delay:?} ends after the latest time a browser can schedule"
                    )));
                }
                info.delay_in_minutes = Some(minutes(delay));
            }
            Start::At(time_ms) => {
                if time_ms > MAX_TIME_MS {
                    return Err(invalid(format!(
                        "alarm time must be at most {MAX_TIME_MS} ms since the epoch, got {time_ms}"
                    )));
                }
                info.when = Some(time_ms as f64);
            }
        }
        if let Some(period) = self.period {
            if period < MIN_PERIOD {
                return Err(invalid(format!(
                    "alarm period must be at least {MIN_PERIOD:?}, got {period:?}"
                )));
            }
            if period.as_millis() > MAX_TIME_MS as u128 {
                return Err(invalid(format!(
                    "alarm period must be at most {MAX_TIME_MS} ms, got {period:?}"
                )));
            }
            info.period_in_minutes = Some(minutes(period));
        }
        Ok(info)
    }
}

fn minutes(duration: Duration) -> f64 {
    duration.as_secs_f64() / 60.0
}

fn invalid(message: String) -> OxichromeError {
    OxichromeError::InvalidArgument(message)
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    delay_in_minutes: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    when: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    period_in_minutes: Option<f64>,
}

fn check_name(name: &str) -> Result<()> {
    if name.len() > MAX_NAME_BYTES {
        return Err(invalid(format!(
            "alarm name must be at most {MAX_NAME_BYTES} bytes of UTF-8, got {}",
            name.len()
        )));
    }
    Ok(())
}

pub async fn create(name: &str, options: AlarmOptions) -> Result<()> {
    check_name(name)?;
    let info = options.to_create_info(js_sys::Date::now())?;
    let count = get_all().await?.len();
    if count >= MAX_ALARMS {
        return Err(OxichromeError::LimitExceeded(format!(
            "an extension can have at most {MAX_ALARMS} alarms, and {count} exist"
        )));
    }
    let js_options = serde_wasm_bindgen::to_value(&info)?;
    let returned = js_bridge::chrome_alarms_create(name, &js_options)?;
    JsFuture::from(js_sys::Promise::resolve(&returned)).await?;
    Ok(())
}

pub async fn create_if_missing(name: &str, options: AlarmOptions) -> Result<bool> {
    check_name(name)?;
    options.to_create_info(js_sys::Date::now())?;
    if get(name).await?.is_some() {
        return Ok(false);
    }
    create(name, options).await?;
    Ok(true)
}

pub async fn get(name: &str) -> Result<Option<Alarm>> {
    let value = JsFuture::from(js_bridge::chrome_alarms_get(name)?).await?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    Ok(Some(serde_wasm_bindgen::from_value(value)?))
}

pub async fn get_all() -> Result<Vec<Alarm>> {
    let value = JsFuture::from(js_bridge::chrome_alarms_get_all()?).await?;
    Ok(serde_wasm_bindgen::from_value(value)?)
}

pub async fn clear(name: &str) -> Result<bool> {
    let value = JsFuture::from(js_bridge::chrome_alarms_clear(name)?).await?;
    Ok(value.as_bool().unwrap_or(false))
}

pub async fn clear_all() -> Result<()> {
    JsFuture::from(js_bridge::chrome_alarms_clear_all()?).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: f64 = 1_800_000_000_000.0;

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    fn json(options: AlarmOptions) -> serde_json::Value {
        serde_json::to_value(options.to_create_info(NOW).unwrap()).unwrap()
    }

    fn error(options: AlarmOptions) -> String {
        match options.to_create_info(NOW) {
            Err(OxichromeError::InvalidArgument(message)) => message,
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn after_sends_only_a_delay_in_minutes() {
        assert_eq!(
            json(AlarmOptions::after(secs(90))),
            serde_json::json!({ "delayInMinutes": 1.5 })
        );
        assert_eq!(
            json(AlarmOptions::after(Duration::from_millis(1500))),
            serde_json::json!({ "delayInMinutes": 0.025 })
        );
    }

    #[test]
    fn at_sends_only_a_time() {
        assert_eq!(
            json(AlarmOptions::at(1000)),
            serde_json::json!({ "when": 1000.0 })
        );
    }

    #[test]
    fn then_every_adds_a_period_in_minutes() {
        assert_eq!(
            json(AlarmOptions::after(secs(60)).then_every(secs(300))),
            serde_json::json!({ "delayInMinutes": 1.0, "periodInMinutes": 5.0 })
        );
        assert_eq!(
            json(AlarmOptions::at(1000).then_every(secs(30))),
            serde_json::json!({ "when": 1000.0, "periodInMinutes": 0.5 })
        );
    }

    #[test]
    fn last_period_wins() {
        assert_eq!(
            json(
                AlarmOptions::after(secs(60))
                    .then_every(secs(300))
                    .then_every(secs(120))
            ),
            serde_json::json!({ "delayInMinutes": 1.0, "periodInMinutes": 2.0 })
        );
    }

    #[test]
    fn zero_delay_and_past_times_are_allowed() {
        assert_eq!(
            json(AlarmOptions::after(Duration::ZERO)),
            serde_json::json!({ "delayInMinutes": 0.0 })
        );
        assert_eq!(
            json(AlarmOptions::at(0)),
            serde_json::json!({ "when": 0.0 })
        );
    }

    #[test]
    fn one_second_period_is_allowed() {
        assert_eq!(
            json(AlarmOptions::after(secs(1)).then_every(MIN_PERIOD)),
            serde_json::json!({ "delayInMinutes": 1.0 / 60.0, "periodInMinutes": 1.0 / 60.0 })
        );
    }

    #[test]
    fn periods_under_one_second_are_rejected() {
        for period in [
            Duration::ZERO,
            Duration::from_millis(999),
            Duration::from_nanos(1),
        ] {
            let message = error(AlarmOptions::after(secs(60)).then_every(period));
            assert!(
                message.starts_with("alarm period must be at least 1s"),
                "{message}"
            );
        }
    }

    #[test]
    fn times_past_the_js_date_limit_are_rejected() {
        assert_eq!(
            json(AlarmOptions::at(MAX_TIME_MS)),
            serde_json::json!({ "when": MAX_TIME_MS as f64 })
        );
        for time_ms in [MAX_TIME_MS + 1, u64::MAX] {
            assert_eq!(
                error(AlarmOptions::at(time_ms)),
                format!(
                    "alarm time must be at most {MAX_TIME_MS} ms since the epoch, got {time_ms}"
                )
            );
        }
    }

    #[test]
    fn delays_ending_past_the_js_date_limit_are_rejected() {
        let longest = Duration::from_millis(MAX_TIME_MS - NOW as u64);
        assert!(AlarmOptions::after(longest).to_create_info(NOW).is_ok());
        for delay in [longest + Duration::from_millis(1), Duration::MAX] {
            assert!(error(AlarmOptions::after(delay)).starts_with("alarm delay of"));
        }
    }

    #[test]
    fn periods_past_the_js_date_limit_are_rejected() {
        let longest = Duration::from_millis(MAX_TIME_MS);
        assert!(AlarmOptions::after(secs(60))
            .then_every(longest)
            .to_create_info(NOW)
            .is_ok());
        for period in [longest + Duration::from_millis(1), Duration::MAX] {
            assert!(error(AlarmOptions::after(secs(60)).then_every(period))
                .starts_with("alarm period must be at most"));
        }
    }

    fn alarm(value: serde_json::Value) -> Alarm {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn names_up_to_1024_bytes_are_allowed() {
        assert!(check_name("").is_ok());
        assert!(check_name(&"x".repeat(1024)).is_ok());
        assert!(check_name(&"\u{e9}".repeat(512)).is_ok());
        assert!(check_name(&"\u{1F600}".repeat(256)).is_ok());
    }

    #[test]
    fn names_over_1024_bytes_are_rejected() {
        for name in [
            "x".repeat(1025),
            "\u{e9}".repeat(512) + "x",
            "\u{4E2D}".repeat(342),
            "\u{1F600}".repeat(257),
        ] {
            match check_name(&name) {
                Err(OxichromeError::InvalidArgument(message)) => assert_eq!(
                    message,
                    format!(
                        "alarm name must be at most 1024 bytes of UTF-8, got {}",
                        name.len()
                    )
                ),
                other => panic!("expected InvalidArgument, got {other:?}"),
            }
        }
    }

    #[test]
    fn alarm_reads_the_browser_key_names() {
        let alarm = alarm(serde_json::json!({
            "name": "tick",
            "scheduledTime": 1000.0,
            "periodInMinutes": 2.0
        }));
        assert_eq!(
            alarm,
            Alarm {
                name: "tick".into(),
                scheduled_time: 1000,
                period: Some(secs(120)),
            }
        );
    }

    #[test]
    fn alarm_without_a_period_is_one_shot() {
        for value in [
            serde_json::json!({ "name": "once", "scheduledTime": 1000.0 }),
            serde_json::json!({ "name": "once", "scheduledTime": 1000.0, "periodInMinutes": null }),
        ] {
            assert_eq!(alarm(value).period, None);
        }
    }

    #[test]
    fn alarm_rounds_to_the_nearest_millisecond() {
        let alarm = alarm(serde_json::json!({
            "name": "tick",
            "scheduledTime": 1794280489501.62,
            "periodInMinutes": 1.0 / 60.0
        }));
        assert_eq!(alarm.scheduled_time, 1794280489502);
        assert_eq!(alarm.period, Some(secs(1)));
    }

    #[test]
    fn periods_round_trip_through_minutes() {
        for ms in [
            1000,
            1001,
            1500,
            12_345,
            59_999,
            300_000,
            86_400_000,
            MAX_TIME_MS,
        ] {
            let period = Duration::from_millis(ms);
            let info = AlarmOptions::after(secs(60))
                .then_every(period)
                .to_create_info(NOW)
                .unwrap();
            let read = alarm(serde_json::json!({
                "name": "p",
                "scheduledTime": 0.0,
                "periodInMinutes": info.period_in_minutes
            }));
            assert_eq!(read.period, Some(period), "{ms} ms");
        }
    }

    #[test]
    fn out_of_range_browser_values_do_not_panic() {
        let alarm = alarm(serde_json::json!({
            "name": "raw",
            "scheduledTime": -5.0,
            "periodInMinutes": -1.0
        }));
        assert_eq!(alarm.scheduled_time, 0);
        assert_eq!(alarm.period, Some(Duration::ZERO));
        let huge = super::Alarm::from(BrowserAlarm {
            name: "huge".into(),
            scheduled_time: 1e300,
            period_in_minutes: Some(f64::INFINITY),
        });
        assert_eq!(huge.scheduled_time, u64::MAX);
        assert_eq!(huge.period, Some(Duration::from_millis(u64::MAX)));
    }
}
