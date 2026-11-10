use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::js_bridge;

pub const MAX_BUTTONS: usize = 2;
pub const MAX_ID_BYTES: usize = 500;

const ID_SEPARATOR: char = '\u{1f}';
const CLEARED_HOOK: &str = "__oxichrome_notification_cleared";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Priority {
    #[default]
    Default,
    High,
    Max,
}

impl Priority {
    fn value(self) -> i8 {
        match self {
            Priority::Default => 0,
            Priority::High => 1,
            Priority::Max => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub title: String,
    pub message: String,
}

impl ListItem {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        ListItem {
            title: title.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Basic,
    Image(String),
    List(Vec<ListItem>),
    Progress(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    kind: Kind,
    title: String,
    message: String,
    icon: String,
    context_message: Option<String>,
    event_time: Option<u64>,
    priority: Priority,
    buttons: Vec<String>,
    require_interaction: bool,
    silent: bool,
}

impl Notification {
    fn new(
        kind: Kind,
        icon: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Notification {
            kind,
            title: title.into(),
            message: message.into(),
            icon: icon.into(),
            context_message: None,
            event_time: None,
            priority: Priority::Default,
            buttons: Vec::new(),
            require_interaction: false,
            silent: false,
        }
    }

    pub fn basic(
        icon: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Notification::new(Kind::Basic, icon, title, message)
    }

    pub fn image(
        icon: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
        image: impl Into<String>,
    ) -> Self {
        Notification::new(Kind::Image(image.into()), icon, title, message)
    }

    pub fn list(
        icon: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
        items: impl IntoIterator<Item = ListItem>,
    ) -> Self {
        Notification::new(
            Kind::List(items.into_iter().collect()),
            icon,
            title,
            message,
        )
    }

    pub fn progress(
        icon: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
        percent: u8,
    ) -> Self {
        Notification::new(Kind::Progress(percent), icon, title, message)
    }

    pub fn context_message(self, text: impl Into<String>) -> Self {
        Notification {
            context_message: Some(text.into()),
            ..self
        }
    }

    pub fn event_time(self, time_ms: u64) -> Self {
        Notification {
            event_time: Some(time_ms),
            ..self
        }
    }

    pub fn priority(self, priority: Priority) -> Self {
        Notification { priority, ..self }
    }

    pub fn button(mut self, title: impl Into<String>) -> Self {
        self.buttons.push(title.into());
        self
    }

    pub fn require_interaction(self) -> Self {
        Notification {
            require_interaction: true,
            ..self
        }
    }

    pub fn silent(self) -> Self {
        Notification {
            silent: true,
            ..self
        }
    }

    fn validate(&self) -> Result<()> {
        if let Kind::Progress(percent) = self.kind {
            if percent > 100 {
                return Err(invalid(format!(
                    "notification progress must be at most 100, got {percent}"
                )));
            }
        }
        if let Kind::List(items) = &self.kind {
            if items.is_empty() {
                return Err(invalid(
                    "a list notification needs at least one item".into(),
                ));
            }
        }
        if self.buttons.len() > MAX_BUTTONS {
            return Err(invalid(format!(
                "a notification can have at most {MAX_BUTTONS} buttons, got {}",
                self.buttons.len()
            )));
        }
        Ok(())
    }

    fn images(&self) -> Vec<&str> {
        let mut images = vec![self.icon.as_str()];
        if let Kind::Image(image) = &self.kind {
            images.push(image);
        }
        images
    }

    fn to_options(&self, chrome_extras: bool, resolve: impl Fn(&str) -> String) -> Options {
        let (kind, image_url, items, progress) = match &self.kind {
            Kind::Basic => ("basic", None, None, None),
            Kind::Image(image) => ("image", Some(resolve(image)), None, None),
            Kind::List(items) => (
                "list",
                None,
                Some(
                    items
                        .iter()
                        .map(|item| Item {
                            title: item.title.clone(),
                            message: item.message.clone(),
                        })
                        .collect(),
                ),
                None,
            ),
            Kind::Progress(percent) => ("progress", None, None, Some(*percent)),
        };
        Options {
            kind,
            title: self.title.clone(),
            message: self.message.clone(),
            icon_url: resolve(&self.icon),
            image_url,
            items,
            progress,
            context_message: self.context_message.clone(),
            event_time: self.event_time.map(|time| time as f64),
            priority: self.priority.value(),
            buttons: (chrome_extras && !self.buttons.is_empty()).then(|| {
                self.buttons
                    .iter()
                    .map(|title| Button {
                        title: title.clone(),
                    })
                    .collect()
            }),
            require_interaction: (chrome_extras && self.require_interaction).then_some(true),
            silent: (chrome_extras && self.silent).then_some(true),
        }
    }
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Options {
    #[serde(rename = "type")]
    kind: &'static str,
    title: String,
    message: String,
    icon_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<Vec<Item>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    progress: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_time: Option<f64>,
    priority: i8,
    #[serde(skip_serializing_if = "Option::is_none")]
    buttons: Option<Vec<Button>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    require_interaction: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    silent: Option<bool>,
}

#[derive(Debug, PartialEq, Serialize)]
struct Item {
    title: String,
    message: String,
}

#[derive(Debug, PartialEq, Serialize)]
struct Button {
    title: String,
}

fn invalid(message: String) -> OxichromeError {
    OxichromeError::InvalidArgument(message)
}

fn check_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(invalid("notification id must not be empty".into()));
    }
    if id.len() > MAX_ID_BYTES {
        return Err(invalid(format!(
            "notification id must be at most {MAX_ID_BYTES} bytes, got {}",
            id.len()
        )));
    }
    if id.contains(ID_SEPARATOR) {
        return Err(invalid(
            "notification id must not contain the U+001F control character".into(),
        ));
    }
    Ok(())
}

fn user_id(browser_id: &str) -> &str {
    browser_id
        .split_once(ID_SEPARATOR)
        .map_or(browser_id, |(id, _)| id)
}

fn resolve_url(path: &str) -> String {
    if path.starts_with("data:") || path.contains("://") {
        path.to_string()
    } else {
        js_bridge::chrome_runtime_get_url(path.trim_start_matches('/'))
    }
}

fn global_fn(name: &str) -> Result<js_sys::Function> {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str(name))?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| OxichromeError::UnexpectedValue(format!("{name} is not available")))
}

async fn check_image(path: &str) -> Result<()> {
    let url = resolve_url(path);
    load_image(&url).await.map_err(|reason| {
        invalid(format!(
            "notification image {path:?} could not be loaded as an image: {reason}"
        ))
    })
}

async fn load_image(url: &str) -> Result<()> {
    let response = JsFuture::from(js_sys::Promise::from(
        global_fn("fetch")?.call1(&JsValue::NULL, &JsValue::from_str(url))?,
    ))
    .await?;
    let ok = js_sys::Reflect::get(&response, &JsValue::from_str("ok"))?;
    if ok.as_bool() != Some(true) {
        let status = js_sys::Reflect::get(&response, &JsValue::from_str("status"))?;
        return Err(OxichromeError::UnexpectedValue(format!(
            "HTTP {}",
            status.as_f64().unwrap_or(0.0)
        )));
    }
    let blob_fn = js_sys::Reflect::get(&response, &JsValue::from_str("blob"))?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| OxichromeError::UnexpectedValue("response has no body".into()))?;
    let blob = JsFuture::from(js_sys::Promise::from(blob_fn.call0(&response)?)).await?;
    JsFuture::from(js_sys::Promise::from(
        global_fn("createImageBitmap")?.call1(&JsValue::NULL, &blob)?,
    ))
    .await?;
    Ok(())
}

fn api() -> Result<JsValue> {
    let chrome = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("chrome"))?;
    let api = js_sys::Reflect::get(&chrome, &JsValue::from_str("notifications"))?;
    if api.is_undefined() {
        return Err(OxichromeError::UnexpectedValue(
            "chrome.notifications is not available here; add the \"notifications\" permission and call it from an extension page or the background".into(),
        ));
    }
    Ok(api)
}

fn has_native_update(api: &JsValue) -> bool {
    js_sys::Reflect::get(api, &JsValue::from_str("update"))
        .map(|update| update.is_function())
        .unwrap_or(false)
}

async fn prepare(notification: &Notification) -> Result<(JsValue, bool)> {
    notification.validate()?;
    let api = api()?;
    for image in notification.images() {
        check_image(image).await?;
    }
    let chrome_extras = has_native_update(&api);
    let options = notification.to_options(chrome_extras, resolve_url);
    Ok((serde_wasm_bindgen::to_value(&options)?, chrome_extras))
}

fn new_id() -> Result<String> {
    let crypto = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))?;
    let random_uuid = js_sys::Reflect::get(&crypto, &JsValue::from_str("randomUUID"))?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| {
            OxichromeError::UnexpectedValue("crypto.randomUUID is not available".into())
        })?;
    random_uuid.call0(&crypto)?.as_string().ok_or_else(|| {
        OxichromeError::UnexpectedValue("crypto.randomUUID returned no string".into())
    })
}

async fn show(id: &str, options: &JsValue) -> Result<String> {
    let value = JsFuture::from(js_bridge::chrome_notifications_create(id, options)?).await?;
    value.as_string().ok_or_else(|| {
        OxichromeError::UnexpectedValue(format!("expected a notification id, got {value:?}"))
    })
}

async fn browser_ids() -> Result<Vec<String>> {
    api()?;
    let value = JsFuture::from(js_bridge::chrome_notifications_get_all()?).await?;
    if value.is_undefined() || value.is_null() {
        return Ok(Vec::new());
    }
    Ok(js_sys::Object::keys(value.unchecked_ref::<js_sys::Object>())
        .iter()
        .filter_map(|key| key.as_string())
        .collect())
}

async fn browser_ids_for(id: &str) -> Result<Vec<String>> {
    Ok(browser_ids()
        .await?
        .into_iter()
        .filter(|browser_id| user_id(browser_id) == id)
        .collect())
}

async fn announce_clear(browser_id: &str) {
    let hook = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str(CLEARED_HOOK))
        .ok()
        .and_then(|hook| hook.dyn_into::<js_sys::Function>().ok());
    if let Some(hook) = hook {
        let _ = hook.call1(&JsValue::NULL, &JsValue::from_str(browser_id));
        return;
    }
    let message = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &message,
        &JsValue::from_str(CLEARED_HOOK),
        &JsValue::from_str(browser_id),
    );
    if let Ok(promise) = js_bridge::chrome_runtime_send_message_checked(&message) {
        let _ = JsFuture::from(promise).await;
    }
}

async fn clear_browser_id(browser_id: &str) -> Result<bool> {
    announce_clear(browser_id).await;
    let value = JsFuture::from(js_bridge::chrome_notifications_clear(browser_id)?).await?;
    Ok(value.as_bool().unwrap_or(false))
}

async fn replace(id: &str, options: &JsValue, previous: Vec<String>) -> Result<()> {
    show(&format!("{id}{ID_SEPARATOR}{}", new_id()?), options).await?;
    for browser_id in previous {
        clear_browser_id(&browser_id).await?;
    }
    Ok(())
}

pub async fn create(notification: &Notification) -> Result<String> {
    let (options, _) = prepare(notification).await?;
    show(&new_id()?, &options).await
}

pub async fn create_with_id(id: &str, notification: &Notification) -> Result<String> {
    check_id(id)?;
    let (options, native) = prepare(notification).await?;
    if native {
        return show(id, &options).await;
    }
    replace(id, &options, browser_ids_for(id).await?).await?;
    Ok(id.to_string())
}

pub async fn update(id: &str, notification: &Notification) -> Result<bool> {
    check_id(id)?;
    let (options, native) = prepare(notification).await?;
    if native {
        let value = JsFuture::from(js_bridge::chrome_notifications_update(id, &options)?).await?;
        return Ok(value.as_bool().unwrap_or(false));
    }
    let previous = browser_ids_for(id).await?;
    if previous.is_empty() {
        return Ok(false);
    }
    replace(id, &options, previous).await?;
    Ok(true)
}

pub async fn clear(id: &str) -> Result<bool> {
    check_id(id)?;
    if has_native_update(&api()?) {
        let value = JsFuture::from(js_bridge::chrome_notifications_clear(id)?).await?;
        return Ok(value.as_bool().unwrap_or(false));
    }
    let mut cleared = false;
    for browser_id in browser_ids_for(id).await? {
        cleared |= clear_browser_id(&browser_id).await?;
    }
    Ok(cleared)
}

pub async fn get_all() -> Result<Vec<String>> {
    let mut ids: Vec<String> = Vec::new();
    for browser_id in browser_ids().await? {
        let id = user_id(&browser_id);
        if !ids.iter().any(|existing| existing == id) {
            ids.push(id.to_string());
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(notification: &Notification, chrome_extras: bool) -> serde_json::Value {
        serde_json::to_value(notification.to_options(chrome_extras, |path| format!("ext:/{path}")))
            .unwrap()
    }

    #[test]
    fn basic_sends_only_the_basic_fields() {
        assert_eq!(
            json(&Notification::basic("icon.png", "T", "M"), true),
            serde_json::json!({ "type": "basic", "title": "T", "message": "M", "iconUrl": "ext:/icon.png", "priority": 0 })
        );
    }

    #[test]
    fn each_kind_sends_its_own_data() {
        assert_eq!(
            json(&Notification::image("icon.png", "T", "M", "pic.png"), true),
            serde_json::json!({ "type": "image", "title": "T", "message": "M", "iconUrl": "ext:/icon.png", "imageUrl": "ext:/pic.png", "priority": 0 })
        );
        assert_eq!(
            json(
                &Notification::list("icon.png", "T", "M", [ListItem::new("a", "b")]),
                true
            ),
            serde_json::json!({ "type": "list", "title": "T", "message": "M", "iconUrl": "ext:/icon.png", "items": [{ "title": "a", "message": "b" }], "priority": 0 })
        );
        assert_eq!(
            json(&Notification::progress("icon.png", "T", "M", 40), true),
            serde_json::json!({ "type": "progress", "title": "T", "message": "M", "iconUrl": "ext:/icon.png", "progress": 40, "priority": 0 })
        );
    }

    #[test]
    fn optional_fields_use_the_browser_key_names() {
        let notification = Notification::basic("icon.png", "T", "M")
            .context_message("ctx")
            .event_time(1000)
            .priority(Priority::High)
            .button("One")
            .button("Two")
            .require_interaction()
            .silent();
        assert_eq!(
            json(&notification, true),
            serde_json::json!({
                "type": "basic", "title": "T", "message": "M", "iconUrl": "ext:/icon.png",
                "contextMessage": "ctx", "eventTime": 1000.0, "priority": 1,
                "buttons": [{ "title": "One" }, { "title": "Two" }],
                "requireInteraction": true, "silent": true
            })
        );
    }

    #[test]
    fn chrome_only_fields_are_dropped_where_unsupported() {
        let notification = Notification::basic("icon.png", "T", "M")
            .button("One")
            .require_interaction()
            .silent()
            .priority(Priority::Max);
        assert_eq!(
            json(&notification, false),
            serde_json::json!({ "type": "basic", "title": "T", "message": "M", "iconUrl": "ext:/icon.png", "priority": 2 })
        );
    }

    #[test]
    fn priorities_map_to_the_browser_range() {
        for (priority, value) in [
            (Priority::Default, 0),
            (Priority::High, 1),
            (Priority::Max, 2),
        ] {
            assert_eq!(
                json(
                    &Notification::basic("icon.png", "T", "M").priority(priority),
                    true
                )["priority"],
                value
            );
        }
    }

    #[test]
    fn progress_over_100_is_rejected() {
        assert!(Notification::progress("icon.png", "T", "M", 0)
            .validate()
            .is_ok());
        assert!(Notification::progress("icon.png", "T", "M", 100)
            .validate()
            .is_ok());
        for percent in [101, 255] {
            match Notification::progress("icon.png", "T", "M", percent).validate() {
                Err(OxichromeError::InvalidArgument(message)) => assert_eq!(
                    message,
                    format!("notification progress must be at most 100, got {percent}")
                ),
                other => panic!("expected InvalidArgument, got {other:?}"),
            }
        }
    }

    #[test]
    fn more_than_two_buttons_are_rejected() {
        let two = Notification::basic("icon.png", "T", "M")
            .button("1")
            .button("2");
        assert!(two.validate().is_ok());
        assert!(matches!(
            two.button("3").validate(),
            Err(OxichromeError::InvalidArgument(_))
        ));
    }

    #[test]
    fn ids_are_checked() {
        assert!(check_id("a").is_ok());
        assert!(check_id(&"x".repeat(MAX_ID_BYTES)).is_ok());
        assert!(check_id(&"é".repeat(MAX_ID_BYTES / 2)).is_ok());
        for bad in [
            String::new(),
            "x".repeat(MAX_ID_BYTES + 1),
            "é".repeat(MAX_ID_BYTES / 2 + 1),
            "a\u{1f}b".to_string(),
        ] {
            assert!(
                matches!(check_id(&bad), Err(OxichromeError::InvalidArgument(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn browser_ids_decode_to_user_ids() {
        assert_eq!(user_id("plain"), "plain");
        assert_eq!(user_id("job\u{1f}0b9e"), "job");
        assert_eq!(user_id(""), "");
    }

    #[test]
    fn empty_lists_are_rejected() {
        assert!(matches!(
            Notification::list("icon.png", "T", "M", []).validate(),
            Err(OxichromeError::InvalidArgument(_))
        ));
        assert!(
            Notification::list("icon.png", "T", "M", [ListItem::new("a", "b")])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn images_lists_icon_and_image() {
        assert_eq!(
            Notification::basic("icon.png", "T", "M").images(),
            vec!["icon.png"]
        );
        assert_eq!(
            Notification::image("icon.png", "T", "M", "pic.png").images(),
            vec!["icon.png", "pic.png"]
        );
    }
}
