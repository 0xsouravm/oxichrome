//! Looking up a host name's addresses. Firefox only, with `Dns` in `permissions`.

use serde::Deserialize;

use crate::call::{self, Only};
use crate::error::{OxichromeError, Result};

/// How to look the name up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Flag {
    BypassCache,
    /// Also find the canonical name (CNAME).
    CanonicalName,
    DisableIpv4,
    DisableIpv6,
    /// Don't use DNS over HTTPS.
    DisableTrr,
    /// Use only what is cached.
    Offline,
    PriorityLow,
    PriorityMedium,
}

impl Flag {
    fn name(self) -> &'static str {
        match self {
            Flag::BypassCache => "bypass_cache",
            Flag::CanonicalName => "canonical_name",
            Flag::DisableIpv4 => "disable_ipv4",
            Flag::DisableIpv6 => "disable_ipv6",
            Flag::DisableTrr => "disable_trr",
            Flag::Offline => "offline",
            Flag::PriorityLow => "priority_low",
            Flag::PriorityMedium => "priority_medium",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    pub addresses: Vec<String>,
    /// With [`Flag::CanonicalName`].
    #[serde(default)]
    pub canonical_name: Option<String>,
    /// Whether DNS over HTTPS answered.
    #[serde(rename = "isTRR", default)]
    pub is_trr: bool,
}

/// The addresses `host` resolves to. A name that doesn't exist is an error.
pub async fn resolve(host: &str, flags: &[Flag]) -> Result<Resolved> {
    call::only(Only::Firefox, "dns", "Chrome doesn't let extensions look up names")?;
    if host.is_empty() || host.contains(['/', ' ']) {
        return Err(OxichromeError::InvalidArgument(format!("`{host}` isn't a host name; give just the name, like `example.com`")));
    }
    if flags.contains(&Flag::DisableIpv4) && flags.contains(&Flag::DisableIpv6) {
        return Err(OxichromeError::InvalidArgument("turning off both IPv4 and IPv6 leaves nothing to look up".into()));
    }
    let flags = js_sys::Array::from_iter(flags.iter().map(|flag| wasm_bindgen::JsValue::from_str(flag.name())));
    let value = call::call("dns", "resolve", &[host.into(), flags.into()], "Dns").await?;
    crate::json::from_js(&value)
}
