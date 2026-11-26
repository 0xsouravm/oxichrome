//! Keeping the computer or its screen awake. Chrome only, with `Power` in `permissions`.
//!
//! An extension holds at most one request: [`keep_awake`] again changes its level, and
//! [`release`] lets the computer sleep as usual. Chrome keeps the request while the service
//! worker sleeps and drops it when the extension is disabled, reloaded or removed.
//!
//! ```ignore
//! power::keep_awake(Level::Display)?;
//! let video = play().await;
//! power::release()?;
//! ```

use wasm_bindgen::JsValue;

use crate::call::{self, Only};
use crate::error::Result;

const INSTEAD: &str = "Firefox doesn't let extensions keep the computer awake";

/// How awake to keep it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    /// The computer stays on; the screen may still dim and turn off.
    System,
    /// The computer and its screen stay on.
    Display,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Level::System => "system",
            Level::Display => "display",
        }
    }
}

fn run(method: &str, args: &[JsValue]) -> Result<()> {
    call::only(Only::Chrome, "power", INSTEAD)?;
    call::start("power", method, args, "Power").map(|_| ())
}

/// Stops the computer from sleeping when idle, at `level`, until [`release`]. The user can
/// still put it to sleep, and closing a laptop's lid still does.
pub fn keep_awake(level: Level) -> Result<()> {
    run("requestKeepAwake", &[JsValue::from_str(level.name())])
}

/// Ends [`keep_awake`]. Does nothing when there was no request.
pub fn release() -> Result<()> {
    run("releaseKeepAwake", &[])
}
