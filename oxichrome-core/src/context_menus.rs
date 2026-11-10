use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

pub const ACTION_MENU_TOP_LEVEL_LIMIT: usize = 6;

pub struct Normal;
pub struct Checkbox;
pub struct Radio;
pub struct Submenu;

pub struct MenuItem<Kind> {
    id: &'static str,
    selection_only: bool,
    kind: PhantomData<Kind>,
}

impl<Kind> Clone for MenuItem<Kind> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Kind> Copy for MenuItem<Kind> {}

impl<Kind> std::fmt::Debug for MenuItem<Kind> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("MenuItem").field(&self.id).finish()
    }
}

#[derive(Default, Serialize)]
struct Update<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked: Option<bool>,
}

fn check_title(title: &str, selection_only: bool) -> Result<()> {
    if title.is_empty() {
        return Err(OxichromeError::InvalidArgument(
            "a menu item title can't be empty".to_string(),
        ));
    }
    if title.contains("%s") && !selection_only {
        return Err(OxichromeError::InvalidArgument(format!(
            "`%s` in the title `{title}` is only replaced for items shown on selected text (`contexts = [selection]`); elsewhere browsers show it as is"
        )));
    }
    Ok(())
}

impl<Kind> MenuItem<Kind> {
    #[doc(hidden)]
    pub const fn __new(id: &'static str, selection_only: bool) -> Self {
        MenuItem {
            id,
            selection_only,
            kind: PhantomData,
        }
    }

    pub fn id(&self) -> &'static str {
        self.id
    }

    async fn update(&self, update: &Update<'_>) -> Result<()> {
        let properties = crate::json::to_js(update)?;
        JsFuture::from(js_bridge::chrome_context_menus_update(self.id, &properties)?).await?;
        Ok(())
    }

    pub async fn set_title(&self, title: &str) -> Result<()> {
        check_title(title, self.selection_only)?;
        let title = title.replace('&', "&&");
        self.update(&Update {
            title: Some(&title),
            ..Default::default()
        })
        .await
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<()> {
        self.update(&Update {
            enabled: Some(enabled),
            ..Default::default()
        })
        .await
    }

    pub async fn set_visible(&self, visible: bool) -> Result<()> {
        self.update(&Update {
            visible: Some(visible),
            ..Default::default()
        })
        .await
    }
}

impl MenuItem<Checkbox> {
    pub async fn set_checked(&self, checked: bool) -> Result<()> {
        self.update(&Update {
            checked: Some(checked),
            ..Default::default()
        })
        .await
    }
}

impl MenuItem<Radio> {
    pub async fn select(&self) -> Result<()> {
        self.update(&Update {
            checked: Some(true),
            ..Default::default()
        })
        .await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Image,
    Video,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Click {
    pub page_url: Option<String>,
    pub frame_url: Option<String>,
    pub frame_id: Option<i64>,
    pub link_url: Option<String>,
    pub src_url: Option<String>,
    pub media_type: Option<MediaType>,
    pub selection_text: Option<String>,
    #[serde(default)]
    pub editable: bool,
    pub checked: Option<bool>,
    pub was_checked: Option<bool>,
    pub tab_id: Option<i32>,
    pub window_id: Option<i32>,
}

impl FromEventArg for Click {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        crate::json::from_js(&value)
    }
}

impl EventArg<events::ContextMenusOnClicked, 1> for Click {}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a submenu",
    label = "`parent` must name a struct marked `#[oxichrome::submenu(...)]`",
    note = "declare the parent as `#[oxichrome::submenu(title = \"...\")] struct Name;`"
)]
pub trait IsSubmenu {
    #[doc(hidden)]
    const CONTEXTS: u16;
}

#[doc(hidden)]
pub mod contexts {
    pub const PAGE: u16 = 1 << 0;
    pub const FRAME: u16 = 1 << 1;
    pub const SELECTION: u16 = 1 << 2;
    pub const LINK: u16 = 1 << 3;
    pub const EDITABLE: u16 = 1 << 4;
    pub const IMAGE: u16 = 1 << 5;
    pub const VIDEO: u16 = 1 << 6;
    pub const AUDIO: u16 = 1 << 7;
    pub const ACTION: u16 = 1 << 8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_follow_both_browsers() {
        assert!(check_title("Search for %s", true).is_ok());
        assert!(check_title("Open", false).is_ok());
        assert!(check_title("", false).is_err());
        let err = check_title("Search for %s", false).unwrap_err().to_string();
        assert!(err.contains("contexts = [selection]"), "{err}");
    }

    #[test]
    fn updates_send_only_what_changed() {
        let update = Update {
            checked: Some(false),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&update).unwrap(),
            serde_json::json!({ "checked": false })
        );
    }

    #[test]
    fn clicks_read_both_browsers() {
        let firefox: Click = serde_json::from_value(serde_json::json!({
            "button": 0, "editable": false, "frameId": 0, "linkText": "a link",
            "linkUrl": "http://127.0.0.1/page.html", "menuItemId": "lnk", "modifiers": [],
            "pageUrl": "http://127.0.0.1/menu.html", "parentMenuItemId": 0, "viewType": "tab",
            "mediaType": "image", "tabId": 1, "windowId": 3
        }))
        .unwrap();
        assert_eq!(firefox.link_url.as_deref(), Some("http://127.0.0.1/page.html"));
        assert_eq!(firefox.media_type, Some(MediaType::Image));
        assert_eq!(firefox.tab_id, Some(1));
        assert_eq!(firefox.checked, None);
        let minimal: Click = serde_json::from_value(serde_json::json!({ "menuItemId": "x" })).unwrap();
        assert!(!minimal.editable);
        assert_eq!(minimal.page_url, None);
    }
}
