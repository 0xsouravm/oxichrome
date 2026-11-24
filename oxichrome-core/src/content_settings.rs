//! Choosing, per site, whether pages may run JavaScript, show images, pop up windows, use
//! the camera and so on. Chrome only, with `ContentSettings` in `permissions`.
//!
//! ```ignore
//! use oxichrome::content_settings::{self, Setting};
//!
//! content_settings::JAVASCRIPT.set("https://*.example.com/*", Setting::Block).await?;
//! let now = content_settings::JAVASCRIPT.get("https://www.example.com/").await?;
//! ```

use serde::{Deserialize, Serialize};

use crate::call::{self, Only};
use crate::error::{OxichromeError, Result};

/// What a site may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Setting {
    Allow,
    Block,
    /// Ask the user each time.
    Ask,
    /// Cookies only: keep them until the browser closes.
    SessionOnly,
}

impl Setting {
    fn name(self) -> &'static str {
        match self {
            Setting::Allow => "allow",
            Setting::Block => "block",
            Setting::Ask => "ask",
            Setting::SessionOnly => "session_only",
        }
    }
}

/// One kind of content setting, like [`JAVASCRIPT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kind {
    name: &'static str,
    allowed: &'static [Setting],
}

use Setting::{Allow, Ask, Block, SessionOnly};

pub const AUTOMATIC_DOWNLOADS: Kind = Kind { name: "automaticDownloads", allowed: &[Allow, Block, Ask] };
pub const AUTO_VERIFY: Kind = Kind { name: "autoVerify", allowed: &[Allow, Block] };
pub const CAMERA: Kind = Kind { name: "camera", allowed: &[Allow, Block, Ask] };
pub const CLIPBOARD: Kind = Kind { name: "clipboard", allowed: &[Allow, Block, Ask] };
pub const COOKIES: Kind = Kind { name: "cookies", allowed: &[Allow, Block, SessionOnly] };
pub const IMAGES: Kind = Kind { name: "images", allowed: &[Allow, Block] };
pub const JAVASCRIPT: Kind = Kind { name: "javascript", allowed: &[Allow, Block] };
pub const LOCATION: Kind = Kind { name: "location", allowed: &[Allow, Block, Ask] };
pub const MICROPHONE: Kind = Kind { name: "microphone", allowed: &[Allow, Block, Ask] };
pub const NOTIFICATIONS: Kind = Kind { name: "notifications", allowed: &[Allow, Block, Ask] };
pub const POPUPS: Kind = Kind { name: "popups", allowed: &[Allow, Block] };
pub const SOUND: Kind = Kind { name: "sound", allowed: &[Allow, Block] };

/// Whether a setting applies to normal windows only, or to the current incognito session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    #[default]
    Regular,
    IncognitoSession,
}

impl Scope {
    fn name(self) -> &'static str {
        match self {
            Scope::Regular => "regular",
            Scope::IncognitoSession => "incognito_session_only",
        }
    }
}

#[derive(Deserialize)]
struct Got {
    setting: Setting,
}

impl Kind {
    /// The name Chrome gives it, like `javascript`.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The settings it takes.
    pub fn allowed(&self) -> &'static [Setting] {
        self.allowed
    }

    async fn run(&self, method: &str, details: serde_json::Value) -> Result<wasm_bindgen::JsValue> {
        call::only(Only::Chrome, "contentSettings", "Firefox lets only the user change these, in its settings")?;
        call::call(&format!("contentSettings.{}", self.name), method, &[crate::json::to_js(&details)?], "ContentSettings").await
    }

    /// What applies to `url` (a page URL like `https://example.com/`) now.
    pub async fn get(&self, url: &str) -> Result<Setting> {
        let got = self.run("get", serde_json::json!({ "primaryUrl": url })).await?;
        Ok(crate::json::from_js::<Got>(&got)?.setting)
    }

    /// Sets it for pages matching `pattern`, a match pattern like `https://*.example.com/*`
    /// or `<all_urls>`.
    pub async fn set(&self, pattern: &str, setting: Setting) -> Result<()> {
        self.set_in(pattern, setting, Scope::Regular).await
    }

    pub async fn set_in(&self, pattern: &str, setting: Setting, scope: Scope) -> Result<()> {
        if !self.allowed.contains(&setting) {
            let allowed: Vec<_> = self.allowed.iter().map(|setting| format!("{setting:?}")).collect();
            return Err(OxichromeError::InvalidArgument(format!("`{}` takes {}, not {setting:?}", self.name, allowed.join(", "))));
        }
        self.run("set", serde_json::json!({ "primaryPattern": pattern, "setting": setting.name(), "scope": scope.name() })).await.map(|_| ())
    }

    /// Removes every rule this extension set for this kind.
    pub async fn clear(&self) -> Result<()> {
        self.run("clear", serde_json::json!({ "scope": Scope::Regular.name() })).await.map(|_| ())
    }
}
