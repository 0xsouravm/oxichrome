use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    pub url: String,
    pub title: String,
}

#[derive(Deserialize)]
struct RawSite {
    url: Option<String>,
    title: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    if message == "oxichrome:no-permission" {
        return OxichromeError::InvalidArgument(
            "the top sites API isn't available; add `TopSites` to `permissions` in #[oxichrome::extension] (if it's optional, request it first)".to_string(),
        );
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

fn sites(raw: Vec<RawSite>) -> Vec<Site> {
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for site in raw {
        if site.kind.as_deref().is_some_and(|kind| kind != "url") {
            continue;
        }
        let Some(url) = site.url.filter(|url| !url.is_empty()) else { continue };
        if seen.contains(&url) {
            continue;
        }
        seen.push(url.clone());
        out.push(Site { url, title: site.title.unwrap_or_default() });
    }
    out
}

pub async fn get() -> Result<Vec<Site>> {
    let promise = js_bridge::top_sites_get().map_err(browser_error)?;
    let value = JsFuture::from(promise).await.map_err(browser_error)?;
    Ok(sites(crate::json::from_js(&value)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_and_firefox_sites_read_the_same() {
        let chrome: Vec<RawSite> = serde_json::from_str(r#"[{"title":"Web Store","url":"https://chrome.google.com/webstore"},{"title":"a","url":"https://a.com/"}]"#).unwrap();
        let firefox: Vec<RawSite> = serde_json::from_str(r#"[{"type":"url","url":"https://a.com/","title":"","favicon":null},{"type":"search","url":"https://s.com/","title":"s"},{"type":"url","url":"https://a.com/","title":"dup"},{"type":"url","url":"","title":"empty"},{"type":"url","url":"https://b.com/","title":null}]"#).unwrap();
        assert_eq!(sites(chrome)[1], Site { url: "https://a.com/".into(), title: "a".into() });
        let firefox = sites(firefox);
        assert_eq!(firefox.iter().map(|s| s.url.as_str()).collect::<Vec<_>>(), ["https://a.com/", "https://b.com/"]);
        assert_eq!(firefox[1].title, "");
    }
}
