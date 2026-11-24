//! Changing Firefox's colors and header image while the extension runs. Firefox only, with
//! `Theme` in `permissions`. (A Chrome theme is a separate kind of extension, set once in
//! its manifest.)

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

use crate::call::{self, Only};
use crate::error::{OxichromeError, Result};
pub use crate::runtime::Listener;

/// A theme: colors by Firefox's names (`frame`, `toolbar`, `tab_background_text`, ...),
/// images and properties. See <https://developer.mozilla.org/docs/Mozilla/Add-ons/WebExtensions/manifest.json/theme>.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, serde_json::Value>,
}

impl Theme {
    pub fn new() -> Self {
        Self::default()
    }

    /// Any color by Firefox's name for it, as CSS (`#0b0c0e`, `rgb(...)`, a name).
    pub fn color(mut self, part: impl Into<String>, color: impl Into<String>) -> Self {
        self.colors.insert(part.into(), color.into());
        self
    }

    /// The title bar and tab strip.
    pub fn frame(self, color: impl Into<String>) -> Self {
        self.color("frame", color)
    }

    /// The text of tabs in the tab strip.
    pub fn tab_text(self, color: impl Into<String>) -> Self {
        self.color("tab_background_text", color)
    }

    /// The toolbar and the selected tab.
    pub fn toolbar(self, color: impl Into<String>) -> Self {
        self.color("toolbar", color)
    }

    pub fn toolbar_text(self, color: impl Into<String>) -> Self {
        self.color("toolbar_text", color)
    }

    /// The address bar.
    pub fn field(self, color: impl Into<String>) -> Self {
        self.color("toolbar_field", color)
    }

    pub fn field_text(self, color: impl Into<String>) -> Self {
        self.color("toolbar_field_text", color)
    }

    /// The header image, a path in the extension.
    pub fn header_image(mut self, path: impl Into<String>) -> Self {
        self.images.insert("theme_frame".into(), serde_json::Value::String(path.into()));
        self
    }
}

/// What `getCurrent` gives: every color Firefox knows, `null` when unset, and colors
/// that may be `[r, g, b]` arrays.
#[derive(Deserialize, Default)]
struct RawTheme {
    #[serde(default)]
    colors: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(default)]
    images: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(default)]
    properties: Option<BTreeMap<String, serde_json::Value>>,
}

fn css(color: &serde_json::Value) -> Option<String> {
    match color {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Array(parts) => {
            let parts: Vec<String> = parts.iter().map(|part| part.to_string()).collect();
            match parts.len() {
                3 => Some(format!("rgb({})", parts.join(", "))),
                4 => Some(format!("rgba({})", parts.join(", "))),
                _ => None,
            }
        }
        _ => None,
    }
}

impl From<RawTheme> for Theme {
    fn from(raw: RawTheme) -> Self {
        let set = |map: Option<BTreeMap<String, serde_json::Value>>| -> BTreeMap<String, serde_json::Value> {
            map.unwrap_or_default().into_iter().filter(|(_, value)| !value.is_null()).collect()
        };
        Theme {
            colors: raw.colors.unwrap_or_default().iter().filter_map(|(part, color)| Some((part.clone(), css(color)?))).collect(),
            images: set(raw.images),
            properties: set(raw.properties),
        }
    }
}

fn read(value: &JsValue) -> Result<Theme> {
    if value.is_null() || value.is_undefined() {
        return Ok(Theme::default());
    }
    Ok(crate::json::from_js::<RawTheme>(value)?.into())
}

fn firefox() -> Result<()> {
    call::only(Only::Firefox, "theme", "a Chrome theme is its own extension, set in the manifest")
}

/// The theme in `window_id`, or the default one with `None`.
pub async fn current(window_id: Option<i32>) -> Result<Theme> {
    firefox()?;
    let args = window_id.map(JsValue::from).into_iter().collect::<Vec<_>>();
    read(&call::call("theme", "getCurrent", &args, "Theme").await?)
}

/// Uses `theme` in `window_id`, or in every window with `None`.
pub async fn set(window_id: Option<i32>, theme: &Theme) -> Result<()> {
    firefox()?;
    if theme.colors.is_empty() && theme.images.is_empty() && theme.properties.is_empty() {
        return Err(OxichromeError::InvalidArgument("this theme sets nothing; use `reset` to go back to the user's theme".into()));
    }
    let mut args: Vec<JsValue> = window_id.map(JsValue::from).into_iter().collect();
    args.push(crate::json::to_js(theme)?);
    call::call("theme", "update", &args, "Theme").await.map(|_| ())
}

/// Goes back to the user's own theme, in `window_id` or everywhere.
pub async fn reset(window_id: Option<i32>) -> Result<()> {
    firefox()?;
    let args = window_id.map(JsValue::from).into_iter().collect::<Vec<_>>();
    call::call("theme", "reset", &args, "Theme").await.map(|_| ())
}

/// Runs whenever the theme changes (from this extension or another), with the new theme
/// and the window it is for (`None` for all of them).
pub fn on_updated(handler: impl Fn(Theme, Option<i32>) + 'static) -> Result<Listener> {
    firefox()?;
    let callback = wasm_bindgen::closure::Closure::<dyn Fn(JsValue)>::new(move |info: JsValue| {
        let theme = js_sys::Reflect::get(&info, &"theme".into()).unwrap_or(JsValue::UNDEFINED);
        let window = js_sys::Reflect::get(&info, &"windowId".into()).ok().and_then(|id| id.as_f64()).map(|id| id as i32);
        handler(read(&theme).unwrap_or_default(), window);
    })
    .into_js_value();
    Listener::add("theme", "onUpdated", &callback)
        .ok_or_else(|| OxichromeError::InvalidArgument("`theme` isn't available; add `Theme` to `permissions` in #[oxichrome::extension]".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firefox_themes_are_read() {
        let raw: RawTheme = serde_json::from_value(serde_json::json!({
            "colors": { "frame": "#ff0000", "toolbar": [0, 255, 0], "popup": [1, 2, 3, 0.5], "tab_line": null },
            "images": null,
        }))
        .unwrap();
        let theme = Theme::from(raw);
        assert_eq!(theme.colors.get("frame").map(String::as_str), Some("#ff0000"));
        assert_eq!(theme.colors.get("toolbar").map(String::as_str), Some("rgb(0, 255, 0)"));
        assert_eq!(theme.colors.get("popup").map(String::as_str), Some("rgba(1, 2, 3, 0.5)"));
        assert!(!theme.colors.contains_key("tab_line") && theme.images.is_empty());
    }
}
