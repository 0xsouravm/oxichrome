use js_sys::Promise;
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::error::{OxichromeError, Result};
use crate::js_bridge;

async fn read<T: DeserializeOwned>(promise: Promise, key: &str) -> Result<Option<T>> {
    let result = JsFuture::from(promise).await?;

    let val =
        js_sys::Reflect::get(&result, &JsValue::from_str(key)).map_err(OxichromeError::from)?;

    if val.is_undefined() || val.is_null() {
        return Ok(None);
    }

    Ok(Some(crate::json::from_js(&val)?))
}

fn items<T: Serialize>(key: &str, value: &T) -> Result<JsValue> {
    let obj = js_sys::Object::new();
    let js_val = crate::json::to_js(value)?;
    js_sys::Reflect::set(&obj, &JsValue::from_str(key), &js_val).map_err(OxichromeError::from)?;
    Ok(obj.into())
}

async fn wait(promise: Promise) -> Result<()> {
    JsFuture::from(promise).await?;
    Ok(())
}

pub async fn get<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
    read(
        js_bridge::chrome_storage_local_get(&JsValue::from_str(key))?,
        key,
    )
    .await
}

pub async fn set<T: Serialize>(key: &str, value: &T) -> Result<()> {
    wait(js_bridge::chrome_storage_local_set(&items(key, value)?)?).await
}

pub async fn remove(key: &str) -> Result<()> {
    wait(js_bridge::chrome_storage_local_remove(&JsValue::from_str(
        key,
    ))?)
    .await
}

pub mod sync {
    use super::*;

    pub async fn get<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
        read(
            js_bridge::chrome_storage_sync_get(&JsValue::from_str(key))?,
            key,
        )
        .await
    }

    pub async fn set<T: Serialize>(key: &str, value: &T) -> Result<()> {
        wait(js_bridge::chrome_storage_sync_set(&items(key, value)?)?).await
    }

    pub async fn remove(key: &str) -> Result<()> {
        wait(js_bridge::chrome_storage_sync_remove(&JsValue::from_str(
            key,
        ))?)
        .await
    }
}

pub mod session {
    use super::*;

    pub async fn get<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
        read(
            js_bridge::chrome_storage_session_get(&JsValue::from_str(key))?,
            key,
        )
        .await
    }

    pub async fn set<T: Serialize>(key: &str, value: &T) -> Result<()> {
        wait(js_bridge::chrome_storage_session_set(&items(key, value)?)?).await
    }

    pub async fn remove(key: &str) -> Result<()> {
        wait(js_bridge::chrome_storage_session_remove(
            &JsValue::from_str(key),
        )?)
        .await
    }
}

/// Settings an administrator pushes to the extension through browser policy. Read-only.
///
/// Describe them with a type that derives `serde::Deserialize` and
/// `oxichrome::ManagedSchema`, and name it in `#[oxichrome::extension(managed_storage = Policy)]`;
/// the Chrome build then ships the schema Chrome checks the policy against (and shows in
/// chrome://policy). Firefox reads the same settings from `policies.json` (under
/// `3rdparty.Extensions.<gecko_id>`) or a managed storage manifest, with no schema.
/// Make each field an `Option` or give the type `#[serde(default)]`: with no policy,
/// [`read`](managed::read) sees no settings at all.
///
/// Chrome can load policy a moment after it starts, and an administrator can change it any
/// time; either way the new settings arrive as an [`on_changed`] change with
/// [`Area::Managed`], so read them again there.
pub mod managed {
    use super::*;

    /// Marks a type `#[derive(oxichrome::ManagedSchema)]` made a schema for.
    pub trait ManagedSchema: DeserializeOwned {}

    async fn all() -> Result<JsValue> {
        match JsFuture::from(js_bridge::storage_managed_get(&JsValue::NULL)?).await {
            Ok(value) => Ok(value),
            // Firefox, when no policy or manifest names the extension
            Err(error) if crate::error::JsError::new(error.clone()).to_string().contains("Managed storage manifest not found") => {
                Ok(js_sys::Object::new().into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Every managed setting, as `T`.
    pub async fn read<T: ManagedSchema>() -> Result<T> {
        let value = all().await?;
        crate::json::from_js(&value).map_err(|error| {
            OxichromeError::UnexpectedValue(format!(
                "the managed settings don't fit `{}` ({error}); make its fields `Option`s or give it `#[serde(default)]`, since there may be no policy",
                std::any::type_name::<T>()
            ))
        })
    }

    /// One managed setting, or `None` when the policy doesn't set it.
    pub async fn get<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
        let value = all().await?;
        let value = js_sys::Reflect::get(&value, &JsValue::from_str(key)).map_err(OxichromeError::from)?;
        if value.is_undefined() || value.is_null() {
            return Ok(None);
        }
        Ok(Some(crate::json::from_js(&value)?))
    }
}

pub use crate::runtime::Listener;

/// The storage area a change happened in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Area {
    Local,
    Sync,
    Session,
    /// Set by an administrator's policy.
    Managed,
}

impl Area {
    fn parse(name: &str) -> Option<Area> {
        match name {
            "local" => Some(Area::Local),
            "sync" => Some(Area::Sync),
            "session" => Some(Area::Session),
            "managed" => Some(Area::Managed),
            _ => None,
        }
    }
}

impl crate::event::FromEventArg for Area {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        value
            .as_string()
            .and_then(|name| Area::parse(&name))
            .ok_or_else(|| OxichromeError::UnexpectedValue(format!("expected a storage area, got {value:?}")))
    }
}

impl crate::event::EventArg<crate::event::events::StorageOnChanged, 2> for Area {}

/// One key that changed, passed to the handler of [`on_changed`].
#[derive(Debug, Clone)]
pub struct Change {
    pub key: String,
    pub area: Area,
    old: JsValue,
    new: JsValue,
}

impl Change {
    /// The value now, or `None` when the key was removed.
    pub fn new_value<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        value(&self.new)
    }

    /// The value before, or `None` when the key is new.
    pub fn old_value<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        value(&self.old)
    }
}

fn value<T: DeserializeOwned>(raw: &JsValue) -> Result<Option<T>> {
    if raw.is_undefined() {
        return Ok(None);
    }
    Ok(Some(crate::json::from_js(raw)?))
}

/// Runs `handler` once for every key that changes in any storage area, in a popup, options
/// page, side panel, offscreen document or content script. Check `change.area` and
/// `change.key` for the ones you care about. The background uses
/// `#[oxichrome::on(storage::on_changed)]` instead.
pub fn on_changed(handler: impl Fn(Change) + 'static) -> Result<Listener> {
    let callback = wasm_bindgen::closure::Closure::<dyn Fn(JsValue, JsValue)>::new(move |changes: JsValue, area: JsValue| {
        let Some(area) = area.as_string().and_then(|name| Area::parse(&name)) else {
            return;
        };
        let Some(changes) = changes.dyn_ref::<js_sys::Object>() else {
            return;
        };
        for entry in js_sys::Object::entries(changes).iter() {
            let entry: js_sys::Array = entry.unchecked_into();
            let Some(key) = entry.get(0).as_string() else { continue };
            if key.starts_with("__oxichrome_") {
                continue;
            }
            let change = entry.get(1);
            let field = |name: &str| js_sys::Reflect::get(&change, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED);
            handler(Change { key, area, old: field("oldValue"), new: field("newValue") });
        }
    })
    .into_js_value();
    Listener::add("storage", "onChanged", &callback).ok_or_else(|| {
        OxichromeError::InvalidArgument("storage isn't available here; add `Storage` to `permissions`".into())
    })
}
