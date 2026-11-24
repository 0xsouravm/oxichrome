//! The browser's privacy settings: read them, change them for as long as the extension is
//! installed, and give them back.
//!
//! Needs `Privacy` in `permissions`. Each setting is a constant here, typed by its value.
//! Some exist in both browsers and some in one; [`Setting::available`] says which, and
//! using one the browser doesn't have is an error that names it.
//!
//! ```ignore
//! use privacy::{IpHandling, NETWORK_PREDICTION, WEBRTC_IP_HANDLING};
//!
//! NETWORK_PREDICTION.set(false).await?;
//! WEBRTC_IP_HANDLING.set(IpHandling::DisableNonProxiedUdp).await?;
//! let now = WEBRTC_IP_HANDLING.get().await?;
//! assert_eq!(now.control, privacy::Control::ControlledByThisExtension);
//! WEBRTC_IP_HANDLING.clear().await?; // back to what the user had
//! ```

use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;
pub use crate::runtime::Listener;

/// Who decides a setting's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    /// A policy or the browser fixes it; no extension can change it.
    NotControllable,
    /// Another extension set it and outranks this one.
    ControlledByOtherExtensions,
    /// This extension may set it; nobody has.
    ControllableByThisExtension,
    /// This extension set it.
    ControlledByThisExtension,
}

impl Control {
    fn parse(level: &str) -> Control {
        match level {
            "controlled_by_this_extension" => Control::ControlledByThisExtension,
            "controllable_by_this_extension" => Control::ControllableByThisExtension,
            "controlled_by_other_extensions" => Control::ControlledByOtherExtensions,
            _ => Control::NotControllable,
        }
    }
}

/// A setting's value and who decides it.
#[derive(Debug, Clone, PartialEq)]
pub struct State<T> {
    pub value: T,
    pub control: Control,
}

/// Which browsers have a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Browsers {
    Both,
    Chrome,
    Firefox,
}

/// One privacy setting with values of type `T`.
pub struct Setting<T> {
    group: &'static str,
    key: &'static str,
    ident: &'static str,
    browsers: Browsers,
    read_only: bool,
    value: PhantomData<fn() -> T>,
}

impl<T> std::fmt::Debug for Setting<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "privacy::{}", self.ident)
    }
}

const fn setting<T>(group: &'static str, key: &'static str, ident: &'static str, browsers: Browsers, read_only: bool) -> Setting<T> {
    Setting { group, key, ident, browsers, read_only, value: PhantomData }
}

impl<T: Serialize + DeserializeOwned> Setting<T> {
    /// The browser's name for it, like `network.networkPredictionEnabled`.
    pub fn name(&self) -> String {
        format!("{}.{}", self.group, self.key)
    }

    /// Whether this browser has the setting.
    pub fn available(&self) -> bool {
        match self.browsers {
            Browsers::Both => true,
            Browsers::Chrome => !js_bridge::is_firefox(),
            Browsers::Firefox => js_bridge::is_firefox(),
        }
    }

    fn here(&self) -> Result<()> {
        if self.available() {
            return Ok(());
        }
        let (missing, has) = if js_bridge::is_firefox() { ("Firefox", "Chrome") } else { ("Chrome", "Firefox") };
        Err(OxichromeError::InvalidArgument(format!(
            "{missing} doesn't have `privacy::{}` (only {has} does); check `available()` first",
            self.ident
        )))
    }

    /// The value now and who decides it.
    pub async fn get(&self) -> Result<State<T>> {
        self.here()?;
        let promise = js_bridge::privacy_call("get", self.group, self.key, &JsValue::UNDEFINED).map_err(browser_error)?;
        read_state(&JsFuture::from(promise).await.map_err(browser_error)?)
    }

    /// Sets it for as long as the extension is installed, or until [`clear`](Self::clear).
    /// Fails when a policy, the browser or another extension decides the setting.
    pub async fn set(&self, value: T) -> Result<()> {
        self.here()?;
        if self.read_only {
            return Err(OxichromeError::InvalidArgument(format!(
                "`privacy::{}` can only be read; Firefox doesn't let extensions change it",
                self.ident
            )));
        }
        let value = crate::json::to_js(&value)?;
        let promise = js_bridge::privacy_call("set", self.group, self.key, &value).map_err(browser_error)?;
        JsFuture::from(promise).await.map_err(browser_error)?;
        let now = self.get().await?;
        if now.control != Control::ControlledByThisExtension {
            return Err(OxichromeError::InvalidArgument(format!(
                "`privacy::{}` didn't change: {}",
                self.ident,
                match now.control {
                    Control::ControlledByOtherExtensions => "another extension decides it",
                    _ => "a policy or the browser decides it",
                }
            )));
        }
        Ok(())
    }

    /// Gives the setting back to the user (or the next extension that set it).
    pub async fn clear(&self) -> Result<()> {
        self.here()?;
        let promise = js_bridge::privacy_call("clear", self.group, self.key, &JsValue::UNDEFINED).map_err(browser_error)?;
        JsFuture::from(promise).await.map_err(browser_error)?;
        Ok(())
    }

    /// Runs `handler` each time the value or who decides it changes, until removed.
    pub fn on_change(&self, handler: impl Fn(State<T>) + 'static) -> Result<Listener>
    where
        T: 'static,
    {
        self.here()?;
        let callback = Closure::<dyn Fn(JsValue)>::new(move |raw: JsValue| {
            if let Ok(state) = read_state(&raw) {
                handler(state);
            }
        })
        .into_js_value();
        Listener::from_remover(js_bridge::privacy_on_change(self.group, self.key, &callback)).ok_or_else(no_permission)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawState<T> {
    value: T,
    level_of_control: String,
}

pub(crate) fn control_of(level: &str) -> Control {
    Control::parse(level)
}

fn read_state<T: DeserializeOwned>(raw: &JsValue) -> Result<State<T>> {
    let raw: RawState<T> = crate::json::from_js(raw)?;
    Ok(State { value: raw.value, control: Control::parse(&raw.level_of_control) })
}

fn no_permission() -> OxichromeError {
    OxichromeError::InvalidArgument("the privacy API isn't available; add `Privacy` to `permissions` in #[oxichrome::extension]".into())
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = JsError::new(error).to_string();
    if message == "oxichrome:no-permission" {
        return no_permission();
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

/// What WebRTC may reveal about the user's network addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpHandling {
    /// The browser's default.
    Default,
    /// Use the default route, with public and private addresses.
    #[serde(rename = "default_public_and_private_interfaces")]
    PublicAndPrivate,
    /// Use the default route, with only the public address.
    #[serde(rename = "default_public_interface_only")]
    PublicOnly,
    /// Only through a proxy, or TCP when there's none.
    DisableNonProxiedUdp,
    /// Only through a proxy (Firefox only; Chrome refuses it).
    ProxyOnly,
}

/// When Firefox blocks trackers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackingProtection {
    Always,
    Never,
    /// Only in private windows (Firefox's default).
    PrivateBrowsing,
}

/// When Firefox loads pages over HTTPS only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpsOnly {
    Always,
    PrivateBrowsing,
    Never,
}

/// Which cookies Firefox accepts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CookieConfig {
    pub behavior: CookieBehavior,
    /// Keep cookies only until the browser closes.
    #[serde(default)]
    pub non_persistent_cookies: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CookieBehavior {
    AllowAll,
    RejectAll,
    RejectThirdParty,
    AllowVisited,
    RejectTrackers,
    /// Firefox's default: no tracker cookies, and other third-party cookies kept per site.
    RejectTrackersAndPartitionForeign,
}

macro_rules! settings {
    ($($(#[$doc:meta])* $ident:ident: $ty:ty = $group:literal . $key:literal, $browsers:ident $(, $read_only:ident)?;)*) => {
        $(
            $(#[$doc])*
            pub const $ident: Setting<$ty> = setting($group, $key, stringify!($ident), Browsers::$browsers, settings!(@ro $($read_only)?));
        )*
    };
    (@ro) => { false };
    (@ro read_only) => { true };
}

settings! {
    /// Preloading pages and looking up addresses before the user clicks.
    NETWORK_PREDICTION: bool = "network"."networkPredictionEnabled", Both;
    /// What WebRTC may reveal about the user's addresses.
    WEBRTC_IP_HANDLING: IpHandling = "network"."webRTCIPHandlingPolicy", Both;
    /// Offering to save passwords.
    PASSWORD_SAVING: bool = "services"."passwordSavingEnabled", Both;
    /// Sending `<a ping>` pings.
    HYPERLINK_AUDITING: bool = "websites"."hyperlinkAuditingEnabled", Both;
    /// Sending the `Referer` header.
    REFERRERS: bool = "websites"."referrersEnabled", Both;

    /// Suggestions from Google when a page doesn't load.
    ALTERNATE_ERROR_PAGES: bool = "services"."alternateErrorPagesEnabled", Chrome;
    /// Filling in addresses.
    AUTOFILL_ADDRESS: bool = "services"."autofillAddressEnabled", Chrome;
    /// Filling in payment cards.
    AUTOFILL_CREDIT_CARD: bool = "services"."autofillCreditCardEnabled", Chrome;
    SAFE_BROWSING: bool = "services"."safeBrowsingEnabled", Chrome;
    SAFE_BROWSING_EXTENDED_REPORTING: bool = "services"."safeBrowsingExtendedReportingEnabled", Chrome;
    /// Search suggestions in the address bar.
    SEARCH_SUGGEST: bool = "services"."searchSuggestEnabled", Chrome;
    /// Spell checking through Google.
    SPELLING_SERVICE: bool = "services"."spellingServiceEnabled", Chrome;
    /// Offering to translate pages.
    TRANSLATION_SERVICE: bool = "services"."translationServiceEnabled", Chrome;
    THIRD_PARTY_COOKIES: bool = "websites"."thirdPartyCookiesAllowed", Chrome;
    /// Sending `DNT: 1`.
    DO_NOT_TRACK: bool = "websites"."doNotTrackEnabled", Chrome;
    /// Privacy Sandbox ad measurement. Extensions may only turn it off.
    AD_MEASUREMENT: bool = "websites"."adMeasurementEnabled", Chrome;
    /// Privacy Sandbox interest groups (FLEDGE). Extensions may only turn it off.
    FLEDGE: bool = "websites"."fledgeEnabled", Chrome;
    /// Privacy Sandbox topics. Extensions may only turn it off.
    TOPICS: bool = "websites"."topicsEnabled", Chrome;

    /// WebRTC as a whole.
    PEER_CONNECTION: bool = "network"."peerConnectionEnabled", Firefox;
    RESIST_FINGERPRINTING: bool = "websites"."resistFingerprinting", Firefox;
    /// Keeping cookies and storage apart per first-party site. Firefox refuses it while
    /// `COOKIE_CONFIG` partitions foreign cookies.
    FIRST_PARTY_ISOLATE: bool = "websites"."firstPartyIsolate", Firefox;
    TRACKING_PROTECTION: TrackingProtection = "websites"."trackingProtectionMode", Firefox;
    COOKIE_CONFIG: CookieConfig = "websites"."cookieConfig", Firefox;
    /// Sending `Sec-GPC: 1`. The user decides it.
    GLOBAL_PRIVACY_CONTROL: bool = "network"."globalPrivacyControl", Firefox, read_only;
    /// The user decides it.
    HTTPS_ONLY: HttpsOnly = "network"."httpsOnlyMode", Firefox, read_only;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_use_the_browser_names() {
        assert_eq!(serde_json::to_string(&IpHandling::PublicAndPrivate).unwrap(), "\"default_public_and_private_interfaces\"");
        assert_eq!(serde_json::to_string(&IpHandling::DisableNonProxiedUdp).unwrap(), "\"disable_non_proxied_udp\"");
        assert_eq!(serde_json::from_str::<TrackingProtection>("\"private_browsing\"").unwrap(), TrackingProtection::PrivateBrowsing);
        let config: CookieConfig = serde_json::from_str(r#"{"behavior":"reject_trackers_and_partition_foreign","nonPersistentCookies":false}"#).unwrap();
        assert_eq!(config.behavior, CookieBehavior::RejectTrackersAndPartitionForeign);
        assert_eq!(NETWORK_PREDICTION.name(), "network.networkPredictionEnabled");
        assert_eq!(format!("{:?}", HTTPS_ONLY), "privacy::HTTPS_ONLY");
        const { assert!(HTTPS_ONLY.read_only && !COOKIE_CONFIG.read_only) };
    }

    #[test]
    fn control_levels() {
        assert_eq!(Control::parse("controlled_by_this_extension"), Control::ControlledByThisExtension);
        assert_eq!(Control::parse("controlled_by_other_extensions"), Control::ControlledByOtherExtensions);
        assert_eq!(Control::parse("not_controllable"), Control::NotControllable);
    }
}
