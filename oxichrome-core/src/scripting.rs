use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use std::result::Result as StdResult;
use crate::js_bridge;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    tab_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    all_frames: bool,
}

impl Target {
    pub fn tab(tab_id: i32) -> Self {
        Target {
            tab_id,
            frame_ids: None,
            all_frames: false,
        }
    }

    pub fn all_frames(mut self) -> Self {
        self.frame_ids = None;
        self.all_frames = true;
        self
    }

    pub fn frames(mut self, frame_ids: &[i64]) -> Self {
        self.frame_ids = Some(frame_ids.to_vec());
        self.all_frames = false;
        self
    }
}

impl From<i32> for Target {
    fn from(tab_id: i32) -> Self {
        Target::tab(tab_id)
    }
}

#[derive(Debug)]
pub struct FrameResult<R> {
    pub frame_id: i64,
    pub result: Result<R>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFrame {
    frame_id: i64,
    ok: Option<String>,
    err: Option<String>,
}

pub struct Script<A, R> {
    name: &'static str,
    export: &'static str,
    package: &'static str,
    types: PhantomData<fn(A) -> R>,
}

impl<A, R> Clone for Script<A, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, R> Copy for Script<A, R> {}

impl<A, R> std::fmt::Debug for Script<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Script").field(&self.name).finish()
    }
}

fn js_error(message: String) -> OxichromeError {
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    if let Some(message) = message.strip_prefix("[oxichrome] ") {
        return OxichromeError::InvalidArgument(message.to_string());
    }
    let lower = message.to_ascii_lowercase();
    if lower.contains("cannot be scripted") || lower.contains("cannot access a ") {
        return OxichromeError::InvalidArgument(format!(
            "{message} (browser pages and the extension store are closed to every extension)"
        ));
    }
    if lower.contains("permission") || lower.contains("cannot access") {
        return OxichromeError::InvalidArgument(format!(
            "{message} (the extension needs a `host_permissions` pattern matching this page, or `ActiveTab` with the call made right after a click on the toolbar button, a menu item or a shortcut)"
        ));
    }
    js_error(message)
}

impl<A: Serialize, R: DeserializeOwned> Script<A, R> {
    #[doc(hidden)]
    pub const fn __new(name: &'static str, export: &'static str, package: &'static str) -> Self {
        Script {
            name,
            export,
            package,
            types: PhantomData,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub async fn run(&self, tab_id: i32, arg: A) -> Result<R> {
        let mut frames = self.run_in(Target::tab(tab_id), arg).await?;
        match frames.pop() {
            Some(frame) => frame.result,
            None => Err(OxichromeError::InvalidArgument(format!(
                "`{}` didn't run: browser pages and the extension store are closed to every extension",
                self.name
            ))),
        }
    }

    pub async fn run_in(&self, target: Target, arg: A) -> Result<Vec<FrameResult<R>>> {
        let arg = serde_json::to_string(&arg)
            .map_err(|err| OxichromeError::Serde(err.to_string()))?;
        let module = format!("wasm/{}.js", self.package.replace('-', "_"));
        let target_js = crate::json::to_js(&target)?;
        let promise = js_bridge::run_script(&target_js, &module, self.export, &arg)?;
        let raw = JsFuture::from(promise).await.map_err(browser_error)?;
        let frames: Vec<RawFrame> = crate::json::from_js(&raw)?;
        if frames.is_empty() {
            return Err(OxichromeError::InvalidArgument(format!(
                "`{}` didn't run: browser pages and the extension store are closed to every extension",
                self.name
            )));
        }
        Ok(frames
            .into_iter()
            .map(|frame| FrameResult {
                frame_id: frame.frame_id,
                result: match (frame.ok, frame.err) {
                    (_, Some(err)) => Err(js_error(format!(
                        "`{}` failed in frame {}: {err}",
                        self.name, frame.frame_id
                    ))),
                    (Some(json), None) => serde_json::from_str(&json)
                        .map_err(|err| OxichromeError::Serde(err.to_string())),
                    (None, None) => Err(js_error(format!(
                        "`{}` returned nothing in frame {}",
                        self.name, frame.frame_id
                    ))),
                },
            })
            .collect())
    }
}

pub async fn insert_css(target: impl Into<Target>, css: &str) -> Result<()> {
    let target = crate::json::to_js(&target.into())?;
    JsFuture::from(js_bridge::insert_css(&target, css)?)
        .await
        .map_err(browser_error)?;
    Ok(())
}

pub async fn remove_css(target: impl Into<Target>, css: &str) -> Result<()> {
    let target = crate::json::to_js(&target.into())?;
    JsFuture::from(js_bridge::remove_css(&target, css)?)
        .await
        .map_err(browser_error)?;
    Ok(())
}

fn report_panics() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            js_bridge::report_script_panic(&format!("it panicked: {info}"));
            previous(info);
        }));
    });
}

#[doc(hidden)]
pub async fn __run_export<A: DeserializeOwned, R: Serialize, F, Fut>(name: &str, arg: JsValue, f: F) -> StdResult<JsValue, JsValue>
where
    F: FnOnce(A) -> Fut,
    Fut: std::future::Future<Output = R>,
{
    report_panics();
    let text = arg.as_string().unwrap_or_else(|| "null".to_string());
    let arg: A = serde_json::from_str(&text).map_err(|err| {
        JsValue::from(js_sys::Error::new(&format!(
            "[oxichrome] `{name}` couldn't read its argument: {err}"
        )))
    })?;
    let value = f(arg).await;
    let json = serde_json::to_string(&value).map_err(|err| {
        JsValue::from(js_sys::Error::new(&format!(
            "[oxichrome] `{name}` couldn't send back its result: {err}"
        )))
    })?;
    Ok(JsValue::from_str(&json))
}
