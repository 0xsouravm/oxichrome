//! Finding text in a tab, like the find bar, and highlighting it. Firefox only, with `Find`
//! in `permissions`; call it from the background or an extension page, not a content script.

use serde::{Deserialize, Serialize};

use crate::call::{self, Only};
use crate::error::{OxichromeError, Result};

/// What to look for, and where.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Search {
    #[serde(skip)]
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tab_id: Option<i32>,
    case_sensitive: bool,
    entire_word: bool,
    match_diacritics: bool,
    include_rect_data: bool,
}

impl Search {
    pub fn new(text: impl Into<String>) -> Self {
        Search { text: text.into(), ..Default::default() }
    }

    /// In this tab; the active tab otherwise.
    pub fn in_tab(mut self, tab_id: i32) -> Self {
        self.tab_id = Some(tab_id);
        self
    }

    pub fn case_sensitive(mut self) -> Self {
        self.case_sensitive = true;
        self
    }

    pub fn whole_words(mut self) -> Self {
        self.entire_word = true;
        self
    }

    /// `é` doesn't match `e`.
    pub fn match_diacritics(mut self) -> Self {
        self.match_diacritics = true;
        self
    }

    /// Also give where each match is on the page.
    pub fn with_rects(mut self) -> Self {
        self.include_rect_data = true;
        self
    }
}

/// Where a match is, in page pixels.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Rect {
    pub top: f64,
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub count: u32,
    /// With [`Search::with_rects`]: each match's boxes (one per line it spans).
    pub rects: Vec<Vec<Rect>>,
}

#[derive(Deserialize)]
struct RawFound {
    count: u32,
    #[serde(default, rename = "rectData")]
    rect_data: Vec<RawRect>,
}

#[derive(Deserialize)]
struct RawRect {
    #[serde(default, rename = "rectsAndTexts")]
    rects_and_texts: RawRects,
}

#[derive(Deserialize, Default)]
struct RawRects {
    #[serde(default, rename = "rectList")]
    rect_list: Vec<Rect>,
}

fn firefox() -> Result<()> {
    call::only(Only::Firefox, "find", "use `scripting` to search the page there")
}

/// Finds every match; follow with [`highlight`] to show them.
pub async fn find(search: Search) -> Result<Found> {
    firefox()?;
    if search.text.is_empty() {
        return Err(OxichromeError::InvalidArgument("there is nothing to find".into()));
    }
    let options = crate::json::to_js(&search)?;
    let raw: RawFound = crate::json::from_js(&call::call("find", "find", &[search.text.as_str().into(), options], "Find").await?)?;
    Ok(Found { count: raw.count, rects: raw.rect_data.into_iter().map(|rect| rect.rects_and_texts.rect_list).collect() })
}

/// Highlights what the last [`find`] found in `tab_id`, scrolling to match number `index`
/// (from 0), or highlights them all with `None`.
pub async fn highlight(tab_id: Option<i32>, index: Option<u32>) -> Result<()> {
    firefox()?;
    let mut options = serde_json::json!({});
    if let Some(tab_id) = tab_id {
        options["tabId"] = tab_id.into();
    }
    if let Some(index) = index {
        options["rangeIndex"] = index.into();
    }
    call::call("find", "highlightResults", &[crate::json::to_js(&options)?], "Find").await.map(|_| ())
}

/// Removes the highlighting.
pub async fn clear(tab_id: Option<i32>) -> Result<()> {
    firefox()?;
    let args = match tab_id {
        Some(tab_id) => vec![tab_id.into()],
        None => Vec::new(),
    };
    call::call("find", "removeHighlighting", &args, "Find").await.map(|_| ())
}
