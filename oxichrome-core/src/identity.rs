//! Signing the user in with OAuth, in Chrome and Firefox.
//!
//! Needs `Identity` in `permissions`. [`authorize`] opens the provider's sign-in page,
//! waits for it to send the user back to [`redirect_url`], and checks the answer: the
//! `state` it made up matches and, for the code flow, it keeps the PKCE verifier to send
//! with the code. Register [`redirect_url`] with the provider as the redirect URI. It is
//! `https://<id>.chromiumapp.org/` in Chrome and `https://<hash>.extensions.allizom.org/`
//! in Firefox; give the extension a `chrome_key` and a `gecko_id` so neither changes.
//!
//! ```ignore
//! use identity::{Authorize, Authorized};
//!
//! let signed_in = identity::authorize(
//!     Authorize::code("https://accounts.example.com/authorize", "my-client-id").scope("profile").pkce(),
//! ).await?;
//! if let Authorized::Code { code, code_verifier, redirect_uri } = signed_in {
//!     // trade `code` (with `code_verifier` and `redirect_uri`) for tokens at the token endpoint
//! }
//! ```
//!
//! Chrome also signs in the Google account the browser uses, with [`google_token`]; that
//! needs `oauth2 = { client_id = "...", scopes = [...] }` in `#[oxichrome::extension]`.

use std::collections::BTreeMap;

use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::JsError;
use crate::js_bridge;

/// Why signing in didn't finish.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AuthError {
    /// The sign-in page wants the user, but the flow wasn't interactive. Try again with
    /// `interactive(true)`, from a click. Firefox also says this when the page doesn't load.
    #[error("signing in needs the user; run it again with `interactive(true)`")]
    NeedsUser,
    /// The user closed the sign-in window or said no.
    #[error("the user closed the sign-in window or refused")]
    Cancelled,
    /// The sign-in page couldn't be loaded (Chrome only; Firefox reports `NeedsUser`).
    #[error("the sign-in page couldn't be loaded")]
    PageFailed,
    /// The provider sent the user back with an OAuth error, like `access_denied`.
    #[error("the provider refused: {error}{}", description.as_deref().map(|d| format!(" ({d})")).unwrap_or_default())]
    Refused { error: String, description: Option<String> },
    /// The answer's `state` isn't the one sent, so it isn't the answer to this sign-in.
    #[error("the answer's `state` doesn't match the one sent")]
    StateMismatch,
    /// The answer lacks something the flow needs, like `code` or `access_token`.
    #[error("the answer has no `{0}`")]
    Missing(&'static str),
    /// The browser doesn't have this (Firefox has no Google sign-in, for instance).
    #[error("{0}")]
    Unavailable(String),
    /// Anything else the browser said.
    #[error("{0}")]
    Browser(String),
}

fn classify(message: &str) -> AuthError {
    match message {
        "oxichrome:no-permission" => {
            AuthError::Unavailable("the identity API isn't available; add `Identity` to `permissions` in #[oxichrome::extension]".into())
        }
        _ if message.contains("User interaction required") || message.contains("Requires user interaction") => AuthError::NeedsUser,
        _ if message.contains("did not approve") || message.contains("cancelled or denied") || message.contains("User cancelled") => AuthError::Cancelled,
        _ if message.contains("could not be loaded") => AuthError::PageFailed,
        _ => AuthError::Browser(message.to_string()),
    }
}

fn browser_error(error: JsValue) -> AuthError {
    classify(&JsError::new(error).to_string())
}

/// Where the provider must send the user back to, with `path` after the slash (often empty).
pub fn redirect_url(path: &str) -> Result<String, AuthError> {
    js_bridge::identity_redirect_url(path.trim_start_matches('/')).map_err(browser_error)
}

/// Opens `url` and returns the URL the provider sent the user back to (on [`redirect_url`]).
/// [`authorize`] does this and reads the answer for you; use this for other flows.
pub async fn launch(url: &str, interactive: bool) -> Result<String, AuthError> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AuthError::Browser(format!("`{url}` isn't an http or https URL")));
    }
    let promise = js_bridge::identity_launch(url, interactive).map_err(browser_error)?;
    let value = JsFuture::from(promise).await.map_err(browser_error)?;
    value.as_string().ok_or(AuthError::Missing("redirect URL"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Response {
    Code { pkce: bool },
    Token,
}

/// An OAuth sign-in for [`authorize`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorize {
    endpoint: String,
    client_id: String,
    response: Response,
    scopes: Vec<String>,
    params: Vec<(String, String)>,
    path: String,
    interactive: bool,
}

impl Authorize {
    /// The authorization code flow: the answer is a code to trade for tokens.
    pub fn code(endpoint: impl Into<String>, client_id: impl Into<String>) -> Self {
        Self::new(endpoint.into(), client_id.into(), Response::Code { pkce: false })
    }

    /// The implicit flow: the answer is the access token itself. Prefer [`code`](Self::code)
    /// with [`pkce`](Self::pkce) where the provider allows it.
    pub fn token(endpoint: impl Into<String>, client_id: impl Into<String>) -> Self {
        Self::new(endpoint.into(), client_id.into(), Response::Token)
    }

    fn new(endpoint: String, client_id: String, response: Response) -> Self {
        Authorize { endpoint, client_id, response, scopes: Vec::new(), params: Vec::new(), path: String::new(), interactive: true }
    }

    /// Asks for this scope too.
    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scopes.push(scope.into());
        self
    }

    /// Sends a PKCE challenge (S256), and hands back the verifier with the code.
    pub fn pkce(mut self) -> Self {
        if let Response::Code { pkce } = &mut self.response {
            *pkce = true;
        }
        self
    }

    /// Adds a query parameter the provider wants, like `prompt=consent`.
    pub fn param(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.push((name.into(), value.into()));
        self
    }

    /// Sends the user back to [`redirect_url`] with this path.
    pub fn redirect_path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// `false` to finish only if the provider needs nobody (the user is signed in and has
    /// agreed before); then [`AuthError::NeedsUser`] means try again interactively.
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }
}

/// What a finished sign-in gave back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authorized {
    /// Trade `code` for tokens at the provider's token endpoint, sending `redirect_uri`
    /// and, with PKCE, `code_verifier`.
    Code { code: String, code_verifier: Option<String>, redirect_uri: String },
    Token { access_token: String, token_type: Option<String>, expires_in: Option<u64>, scope: Option<String> },
}

/// The parameters of a redirect: the query, then the fragment (where tokens come).
fn answer(redirect: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let without_fragment = redirect.split('#').next().unwrap_or_default();
    let query = without_fragment.split_once('?').map(|(_, query)| query).unwrap_or_default();
    let fragment = redirect.split_once('#').map(|(_, fragment)| fragment).unwrap_or_default();
    for part in query.split('&').chain(fragment.split('&')).filter(|part| !part.is_empty()) {
        let (name, value) = part.split_once('=').unwrap_or((part, ""));
        let decode = |text: &str| percent_decode(&text.replace('+', " "));
        found.entry(decode(name)).or_insert_with(|| decode(value));
    }
    found
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn read_answer(response: Response, params: &BTreeMap<String, String>, state: &str, verifier: Option<String>, redirect_uri: String) -> Result<Authorized, AuthError> {
    if let Some(error) = params.get("error") {
        return Err(AuthError::Refused { error: error.clone(), description: params.get("error_description").cloned() });
    }
    if params.get("state").map(String::as_str) != Some(state) {
        return Err(AuthError::StateMismatch);
    }
    match response {
        Response::Code { .. } => Ok(Authorized::Code {
            code: params.get("code").cloned().ok_or(AuthError::Missing("code"))?,
            code_verifier: verifier,
            redirect_uri,
        }),
        Response::Token => Ok(Authorized::Token {
            access_token: params.get("access_token").cloned().ok_or(AuthError::Missing("access_token"))?,
            token_type: params.get("token_type").cloned(),
            expires_in: params.get("expires_in").and_then(|seconds| seconds.parse().ok()),
            scope: params.get("scope").cloned(),
        }),
    }
}

/// Signs the user in with the provider and returns its checked answer.
pub async fn authorize(sign_in: Authorize) -> Result<Authorized, AuthError> {
    if !(sign_in.endpoint.starts_with("https://") || sign_in.endpoint.starts_with("http://")) {
        return Err(AuthError::Browser(format!("`{}` isn't an http or https URL", sign_in.endpoint)));
    }
    let redirect_uri = redirect_url(&sign_in.path)?;
    let state = js_bridge::identity_random();
    let mut params: Vec<(String, String)> = vec![
        ("response_type".into(), if sign_in.response == Response::Token { "token".into() } else { "code".into() }),
        ("client_id".into(), sign_in.client_id.clone()),
        ("redirect_uri".into(), redirect_uri.clone()),
        ("state".into(), state.clone()),
    ];
    if !sign_in.scopes.is_empty() {
        params.push(("scope".into(), sign_in.scopes.join(" ")));
    }
    let mut verifier = None;
    if sign_in.response == (Response::Code { pkce: true }) {
        let pair = JsFuture::from(js_bridge::identity_pkce()).await.map_err(browser_error)?;
        let pair: js_sys::Array = pair.into();
        verifier = pair.get(0).as_string();
        params.push(("code_challenge".into(), pair.get(1).as_string().unwrap_or_default()));
        params.push(("code_challenge_method".into(), "S256".into()));
    }
    params.extend(sign_in.params.iter().cloned());
    let encoded: Vec<String> = params
        .iter()
        .map(|(name, value)| format!("{}={}", js_sys::encode_uri_component(name), js_sys::encode_uri_component(value)))
        .collect();
    let joiner = if sign_in.endpoint.contains('?') { '&' } else { '?' };
    let url = format!("{}{joiner}{}", sign_in.endpoint, encoded.join("&"));
    let redirect = launch(&url, sign_in.interactive).await?;
    read_answer(sign_in.response, &answer(&redirect), &state, verifier, redirect_uri)
}

/// A Google access token from Chrome's own sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleToken {
    pub token: String,
    /// The scopes granted, when Chrome says.
    pub granted_scopes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawToken {
    token: Option<String>,
    #[serde(default)]
    granted_scopes: Vec<String>,
}

fn chrome_only(what: &str) -> Result<(), AuthError> {
    if js_bridge::is_firefox() {
        return Err(AuthError::Unavailable(format!("Firefox has no Google sign-in, so `identity::{what}` is Chrome only; use `authorize` there")));
    }
    Ok(())
}

/// A token for the Google account signed in to Chrome, for the `oauth2` scopes in
/// `#[oxichrome::extension]` (or `scopes`, when given). Chrome only.
pub async fn google_token(interactive: bool, scopes: &[&str]) -> Result<GoogleToken, AuthError> {
    chrome_only("google_token")?;
    let scopes = js_sys::Array::from_iter(scopes.iter().map(|scope| JsValue::from_str(scope)));
    let promise = js_bridge::identity_google_token(interactive, &scopes).map_err(browser_error)?;
    let value = JsFuture::from(promise).await.map_err(browser_error)?;
    let raw: RawToken = if value.is_string() {
        RawToken { token: value.as_string(), granted_scopes: Vec::new() }
    } else {
        crate::json::from_js(&value).map_err(|err| AuthError::Browser(err.to_string()))?
    };
    Ok(GoogleToken { token: raw.token.ok_or(AuthError::Missing("token"))?, granted_scopes: raw.granted_scopes })
}

/// Forgets a token from [`google_token`], so the next call asks Google again. Chrome only.
pub async fn remove_google_token(token: &str) -> Result<(), AuthError> {
    chrome_only("remove_google_token")?;
    let promise = js_bridge::identity_remove_google_token(token).map_err(browser_error)?;
    JsFuture::from(promise).await.map_err(browser_error)?;
    Ok(())
}

/// The Google account signed in to Chrome. Both are empty when nobody is signed in, and
/// `email` is empty without `IdentityEmail` in `permissions`. Chrome only.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Profile {
    pub email: String,
    pub id: String,
}

/// Who is signed in to Chrome. Chrome only.
pub async fn profile() -> Result<Profile, AuthError> {
    chrome_only("profile")?;
    let promise = js_bridge::identity_profile().map_err(browser_error)?;
    let value = JsFuture::from(promise).await.map_err(browser_error)?;
    crate::json::from_js(&value).map_err(|err| AuthError::Browser(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_read_from_query_and_fragment() {
        let code = answer("https://x.chromiumapp.org/cb?code=a%2Fb&state=s1");
        assert_eq!(read_answer(Response::Code { pkce: true }, &code, "s1", Some("v".into()), "r".into()), Ok(Authorized::Code { code: "a/b".into(), code_verifier: Some("v".into()), redirect_uri: "r".into() }));
        assert_eq!(read_answer(Response::Code { pkce: false }, &code, "other", None, "r".into()), Err(AuthError::StateMismatch));
        let token = answer("https://x.extensions.allizom.org/#access_token=t+1&token_type=Bearer&expires_in=3600&state=s&scope=a%20b");
        assert_eq!(
            read_answer(Response::Token, &token, "s", None, "r".into()),
            Ok(Authorized::Token { access_token: "t 1".into(), token_type: Some("Bearer".into()), expires_in: Some(3600), scope: Some("a b".into()) })
        );
        let refused = answer("https://x/?error=access_denied&error_description=No%20thanks&state=s");
        assert_eq!(read_answer(Response::Token, &refused, "s", None, "r".into()), Err(AuthError::Refused { error: "access_denied".into(), description: Some("No thanks".into()) }));
        assert_eq!(read_answer(Response::Token, &answer("https://x/?state=s"), "s", None, "r".into()), Err(AuthError::Missing("access_token")));
        assert_eq!(percent_decode("100%"), "100%");
    }

    #[test]
    fn browser_messages_read_the_same() {
        assert_eq!(classify("User interaction required. Try setting `abortOnLoadForNonInteractive`"), AuthError::NeedsUser);
        assert_eq!(classify("Requires user interaction"), AuthError::NeedsUser);
        assert_eq!(classify("The user did not approve access."), AuthError::Cancelled);
        assert_eq!(classify("User cancelled or denied access."), AuthError::Cancelled);
        assert_eq!(classify("Authorization page could not be loaded."), AuthError::PageFailed);
    }
}
