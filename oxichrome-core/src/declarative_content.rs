//! Turning the toolbar button on only on pages it is for, with no access to the pages.
//!
//! Chrome only, with `DeclarativeContent` in `permissions`. Turn the button off everywhere
//! with `action::disable(None)`, then give Chrome [`Rule`]s for where to show it; Chrome
//! checks them itself as pages load, and keeps them until the extension changes them.
//! Firefox has no such API, so [`set_rules`] fails there; use `tabs` events with
//! `action::enable` instead.
//!
//! ```ignore
//! use oxichrome::declarative_content::{self, Page, Rule};
//!
//! action::disable(None).await?;
//! declarative_content::set_rules(vec![
//!     Rule::show_action().when(Page::new().host_suffix(".github.com").css("article.markdown-body")),
//! ]).await?;
//! ```

use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::js_bridge;

/// What a page's URL has to look like. Every part given has to match.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_equals: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_contains: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_contains: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_contains: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_prefix: Option<String>,
    /// An RE2 regular expression for the whole URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_matches: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub schemes: Vec<String>,
}

/// One kind of page: its URL, elements it has and whether it is bookmarked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    #[serde(skip_serializing_if = "Option::is_none")]
    page_url: Option<UrlFilter>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    css: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_bookmarked: Option<bool>,
}

impl Page {
    pub fn new() -> Self {
        Self::default()
    }

    fn url_part(mut self, set: impl FnOnce(&mut UrlFilter)) -> Self {
        set(self.page_url.get_or_insert_with(UrlFilter::default));
        self
    }

    /// The host is exactly this, like `example.com`.
    pub fn host(self, host: impl Into<String>) -> Self {
        self.url_part(|url| url.host_equals = Some(host.into()))
    }

    /// The host ends with this, like `.example.com` for its subdomains.
    pub fn host_suffix(self, suffix: impl Into<String>) -> Self {
        self.url_part(|url| url.host_suffix = Some(suffix.into()))
    }

    pub fn path_prefix(self, prefix: impl Into<String>) -> Self {
        self.url_part(|url| url.path_prefix = Some(prefix.into()))
    }

    pub fn url_prefix(self, prefix: impl Into<String>) -> Self {
        self.url_part(|url| url.url_prefix = Some(prefix.into()))
    }

    /// The URL matches this RE2 regular expression.
    pub fn url_matches(self, pattern: impl Into<String>) -> Self {
        self.url_part(|url| url.url_matches = Some(pattern.into()))
    }

    pub fn scheme(self, scheme: impl Into<String>) -> Self {
        self.url_part(|url| url.schemes.push(scheme.into()))
    }

    /// Any other URL filter.
    pub fn url(mut self, filter: UrlFilter) -> Self {
        self.page_url = Some(filter);
        self
    }

    /// The page has an element this CSS selector finds. Chrome takes compound selectors
    /// only (like `video` or `a.download[href]`), not ones with spaces or `>`.
    pub fn css(mut self, selector: impl Into<String>) -> Self {
        self.css.push(selector.into());
        self
    }

    /// The page is (or isn't) bookmarked; needs `Bookmarks` in `permissions`.
    pub fn bookmarked(mut self, bookmarked: bool) -> Self {
        self.is_bookmarked = Some(bookmarked);
        self
    }

    fn check(&self) -> Result<()> {
        for selector in &self.css {
            if selector.trim().is_empty() || selector.contains([' ', '>', '+', '~', ',']) {
                return Err(OxichromeError::InvalidArgument(format!(
                    "Chrome matches compound CSS selectors only, so `{selector}` can't be used; drop the spaces, `>`, `+`, `~` and `,`, or add one `css` per selector"
                )));
            }
        }
        if self.page_url.is_none() && self.css.is_empty() && self.is_bookmarked.is_none() {
            return Err(OxichromeError::InvalidArgument("this `Page` matches every page; give it a URL, a CSS selector or `bookmarked`".into()));
        }
        Ok(())
    }
}

/// Show the toolbar button on pages that match any of the rule's [`Page`]s.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pages: Vec<Page>,
}

impl Rule {
    pub fn show_action() -> Self {
        Self::default()
    }

    pub fn when(mut self, page: Page) -> Self {
        self.pages.push(page);
        self
    }
}

fn chrome_only() -> Result<()> {
    if js_bridge::is_firefox() {
        return Err(OxichromeError::InvalidArgument(
            "Firefox has no `declarativeContent`; turn the button on from `tabs` events with `action::enable` instead".into(),
        ));
    }
    Ok(())
}

fn no_permission(error: wasm_bindgen::JsValue) -> OxichromeError {
    if crate::error::JsError::new(error.clone()).to_string() == "oxichrome:no-permission" {
        return OxichromeError::InvalidArgument("`declarativeContent` isn't available; add `DeclarativeContent` to `permissions` in #[oxichrome::extension]".into());
    }
    error.into()
}

/// Replaces the extension's rules with these.
pub async fn set_rules(rules: Vec<Rule>) -> Result<()> {
    chrome_only()?;
    for rule in &rules {
        if rule.pages.is_empty() {
            return Err(OxichromeError::InvalidArgument("a rule needs at least one `when(Page)`".into()));
        }
        rule.pages.iter().try_for_each(Page::check)?;
    }
    let rules = crate::json::to_js(&rules)?;
    JsFuture::from(js_bridge::declarative_content_set(&rules).map_err(no_permission)?).await.map_err(no_permission)?;
    Ok(())
}

/// Removes every rule, so the button stays as `action::enable`/`disable` left it.
pub async fn clear() -> Result<()> {
    set_rules(Vec::new()).await
}

/// The rules Chrome has now.
pub async fn rules() -> Result<Vec<Rule>> {
    chrome_only()?;
    let value = JsFuture::from(js_bridge::declarative_content_get().map_err(no_permission)?).await.map_err(no_permission)?;
    crate::json::from_js(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_checked() {
        assert!(Page::new().host("a.com").path_prefix("/x").css("video").check().is_ok());
        assert!(Page::new().check().is_err());
        assert!(Page::new().css("div video").check().is_err());
        assert!(Page::new().css("a > b").check().is_err());
        let json = serde_json::to_value(Rule::show_action().when(Page::new().host_suffix(".a.com").scheme("https"))).unwrap();
        assert_eq!(json, serde_json::json!({ "pages": [{ "pageUrl": { "hostSuffix": ".a.com", "schemes": ["https"] } }] }));
    }
}
