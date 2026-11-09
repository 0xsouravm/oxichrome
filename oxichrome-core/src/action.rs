use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::js_bridge;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Details<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    popup: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tab_id: Option<i32>,
}

fn to_js(details: &Details<'_>) -> Result<JsValue> {
    Ok(serde_wasm_bindgen::to_value(details)?)
}

async fn await_unit(promise: js_sys::Promise) -> Result<()> {
    JsFuture::from(promise).await?;
    Ok(())
}

async fn await_string(promise: js_sys::Promise) -> Result<String> {
    let value = JsFuture::from(promise).await?;
    value
        .as_string()
        .ok_or_else(|| OxichromeError::UnexpectedValue(format!("expected a string, got {value:?}")))
}
pub async fn set_badge_text(text: &str, tab_id: Option<i32>) -> Result<()> {
    let details = Details {
        text: Some(text),
        tab_id,
        ..Default::default()
    };
    await_unit(js_bridge::chrome_action_set_badge_text(&to_js(&details)?)).await
}

pub async fn get_badge_text(tab_id: Option<i32>) -> Result<String> {
    let details = Details {
        tab_id,
        ..Default::default()
    };
    await_string(js_bridge::chrome_action_get_badge_text(&to_js(&details)?)).await
}
pub async fn set_badge_background_color(color: &str, tab_id: Option<i32>) -> Result<()> {
    let details = Details {
        color: Some(color),
        tab_id,
        ..Default::default()
    };
    await_unit(js_bridge::chrome_action_set_badge_background_color(&to_js(
        &details,
    )?))
    .await
}
pub async fn set_badge_text_color(color: &str, tab_id: Option<i32>) -> Result<()> {
    let details = Details {
        color: Some(color),
        tab_id,
        ..Default::default()
    };
    await_unit(js_bridge::chrome_action_set_badge_text_color(&to_js(
        &details,
    )?))
    .await
}
pub async fn set_title(title: &str, tab_id: Option<i32>) -> Result<()> {
    let details = Details {
        title: Some(title),
        tab_id,
        ..Default::default()
    };
    await_unit(js_bridge::chrome_action_set_title(&to_js(&details)?)).await
}

pub async fn get_title(tab_id: Option<i32>) -> Result<String> {
    let details = Details {
        tab_id,
        ..Default::default()
    };
    await_string(js_bridge::chrome_action_get_title(&to_js(&details)?)).await
}
pub async fn set_icon(path: &str, tab_id: Option<i32>) -> Result<()> {
    let details = Details {
        path: Some(path),
        tab_id,
        ..Default::default()
    };
    await_unit(js_bridge::chrome_action_set_icon(&to_js(&details)?)).await
}
pub async fn set_popup(popup: &str, tab_id: Option<i32>) -> Result<()> {
    let details = Details {
        popup: Some(popup),
        tab_id,
        ..Default::default()
    };
    await_unit(js_bridge::chrome_action_set_popup(&to_js(&details)?)).await
}

pub async fn get_popup(tab_id: Option<i32>) -> Result<String> {
    let details = Details {
        tab_id,
        ..Default::default()
    };
    await_string(js_bridge::chrome_action_get_popup(&to_js(&details)?)).await
}

pub async fn enable(tab_id: Option<i32>) -> Result<()> {
    await_unit(js_bridge::chrome_action_enable(tab_id)).await
}

pub async fn disable(tab_id: Option<i32>) -> Result<()> {
    await_unit(js_bridge::chrome_action_disable(tab_id)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_uses_the_browser_key_names() {
        let details = Details {
            text: Some("1"),
            tab_id: Some(7),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&details).unwrap(),
            serde_json::json!({ "text": "1", "tabId": 7 })
        );
    }

    #[test]
    fn details_keeps_empty_text_so_the_badge_can_be_cleared() {
        let details = Details {
            text: Some(""),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&details).unwrap(),
            serde_json::json!({ "text": "" })
        );
    }

    #[test]
    fn details_omits_unset_tab_id() {
        assert_eq!(
            serde_json::to_value(Details::default()).unwrap(),
            serde_json::json!({})
        );
    }
}
