//! Calling a promise-returning extension API by name, for the APIs that need no more than
//! that, with one wording for "this browser doesn't have it" and "add the permission".

use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::js_bridge;

/// Which browser an API is in, when it isn't in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Only {
    Chrome,
    Firefox,
}

/// Fails in the other browser, saying what to use there instead (`instead`, may be empty).
pub(crate) fn only(only: Only, api: &str, instead: &str) -> Result<()> {
    let (has, lacks) = match only {
        Only::Chrome if js_bridge::is_firefox() => ("Chrome", "Firefox"),
        Only::Firefox if !js_bridge::is_firefox() => ("Firefox", "Chrome"),
        _ => return Ok(()),
    };
    let instead = if instead.is_empty() { String::new() } else { format!("; {instead}") };
    Err(OxichromeError::InvalidArgument(format!("{lacks} has no `{api}` (only {has} does){instead}")))
}

/// `chrome.<path>.<method>(...args)`, awaited. `permission` names the `Permission` to add
/// when the API isn't there.
pub(crate) async fn call(path: &str, method: &str, args: &[JsValue], permission: &str) -> Result<JsValue> {
    let args = js_sys::Array::from_iter(args);
    let missing = || {
        OxichromeError::InvalidArgument(format!("`{path}` isn't available here; add `{permission}` to `permissions` in #[oxichrome::extension]"))
    };
    let promise = js_bridge::api_call(path, method, &args).map_err(|error| {
        if crate::error::JsError::new(error.clone()).to_string() == "oxichrome:no-permission" { missing() } else { error.into() }
    })?;
    Ok(JsFuture::from(promise).await?)
}
