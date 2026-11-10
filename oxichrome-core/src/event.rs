use wasm_bindgen::JsValue;

use crate::error::Result;

#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be an event handler argument",
    label = "unsupported argument type",
    note = "use `JsValue`, or a type oxichrome provides for this event such as `alarms::Alarm`"
)]
pub trait FromEventArg: Sized {
    fn from_event_arg(value: JsValue) -> Result<Self>;
}

impl FromEventArg for JsValue {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        Ok(value)
    }
}
