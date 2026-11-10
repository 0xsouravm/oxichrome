use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::Result;
use crate::event::{events, EventArg, FromEventArg};
use crate::js_bridge;

pub const SHORTCUT_LIMIT: usize = 4;

#[derive(Clone, Copy)]
pub struct Command {
    name: &'static str,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Command").field(&self.name).finish()
    }
}

#[derive(Deserialize)]
struct Registered {
    name: Option<String>,
    shortcut: Option<String>,
}

async fn shortcut_of(name: &str) -> Result<Option<String>> {
    let all = JsFuture::from(js_bridge::chrome_commands_get_all()?).await?;
    let all: Vec<Registered> = crate::json::from_js(&all)?;
    Ok(all
        .into_iter()
        .find(|command| command.name.as_deref() == Some(name))
        .and_then(|command| command.shortcut)
        .filter(|shortcut| !shortcut.is_empty()))
}

impl Command {
    #[doc(hidden)]
    pub const fn __new(name: &'static str) -> Self {
        Command { name }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub async fn shortcut(&self) -> Result<Option<String>> {
        shortcut_of(self.name).await
    }
}

pub async fn action_shortcut() -> Result<Option<String>> {
    shortcut_of("_execute_action").await
}

pub async fn open_shortcut_settings() -> Result<()> {
    JsFuture::from(js_bridge::open_shortcut_settings()?).await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Press {
    pub tab_id: Option<i32>,
    pub window_id: Option<i32>,
}

impl FromEventArg for Press {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        crate::json::from_js(&value)
    }
}

impl EventArg<events::CommandsOnCommand, 1> for Press {}
