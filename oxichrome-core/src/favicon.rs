//! Site icons from Chrome's cache. Chrome only, with `Favicon` in `permissions`.
//!
//! [`url`] gives an address an `<img>` or `fetch` in an extension page can load. For a
//! content script, also add `web_accessible = ["_favicon/*"]` to `#[oxichrome::extension]`;
//! that lets any site load the icons too, and so learn which sites the user has visited.
//! Firefox has no such cache; use [`Tab::fav_icon_url`](crate::tabs::Tab::fav_icon_url) there.
//!
//! ```ignore
//! let icon = favicon::url("https://rust-lang.org/", 32)?;
//! view! { <img src=icon width="16" height="16" /> }
//! ```

use crate::call::{self, Only};
use crate::error::{OxichromeError, Result};

/// The address of `page_url`'s icon, `size` pixels square. Chrome gives its default globe
/// for a page it has no icon for. Draw a 32 pixel icon at 16 CSS pixels for sharp screens.
pub fn url(page_url: &str, size: u32) -> Result<String> {
    call::only(Only::Chrome, "favicon", "use `Tab::fav_icon_url` there")?;
    if !page_url.contains("://") {
        return Err(OxichromeError::InvalidArgument(format!("`{page_url}` isn't a full URL; write it like `https://{page_url}/`")));
    }
    if size == 0 {
        return Err(OxichromeError::InvalidArgument("an icon size of 0 shows nothing; give the size in pixels, like 32".into()));
    }
    Ok(format!("{}?pageUrl={}&size={size}", crate::runtime::get_url("/_favicon/"), String::from(js_sys::encode_uri_component(page_url))))
}
