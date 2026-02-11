use wasm_bindgen::JsValue;

#[derive(Debug)]
pub struct JsError {
    message: String,
}

impl JsError {
    pub fn new(val: JsValue) -> Self {
        let message = js_error_message(&val);
        JsError { message }
    }
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for JsError {}

fn js_error_message(val: &JsValue) -> String {
    if let Some(s) = val.as_string() {
        return s;
    }
    let message = js_sys::Reflect::get(val, &JsValue::from_str("message"));
    if let Ok(msg) = message {
        if let Some(s) = msg.as_string() {
            return s;
        }
    }
    format!("{val:?}")
}

#[derive(Debug, thiserror::Error)]
pub enum OxichromeError {
    #[error("JS error: {0}")]
    Js(#[from] JsError),

    #[error("serde error: {0}")]
    Serde(String),

    #[error("unexpected value: {0}")]
    UnexpectedValue(String),
}

impl From<JsValue> for OxichromeError {
    fn from(val: JsValue) -> Self {
        OxichromeError::Js(JsError::new(val))
    }
}

impl From<serde_wasm_bindgen::Error> for OxichromeError {
    fn from(err: serde_wasm_bindgen::Error) -> Self {
        OxichromeError::Serde(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, OxichromeError>;
