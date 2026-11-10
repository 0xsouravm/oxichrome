use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::JsValue;

use crate::error::{OxichromeError, Result};

pub(crate) const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

fn serde_error(err: serde_json::Error) -> OxichromeError {
    OxichromeError::Serde(err.to_string())
}

fn check_numbers(value: &Value) -> Result<()> {
    match value {
        Value::Number(number) => {
            let exact = match (number.as_u64(), number.as_i64()) {
                (Some(n), _) => n <= MAX_SAFE_INTEGER,
                (None, Some(n)) => n.unsigned_abs() <= MAX_SAFE_INTEGER,
                (None, None) => true,
            };
            if exact {
                Ok(())
            } else {
                Err(OxichromeError::InvalidArgument(format!(
                    "the number {number} can't be stored exactly: browsers keep numbers as 64-bit floats, which hold whole numbers only up to {MAX_SAFE_INTEGER}; store it as a string instead"
                )))
            }
        }
        Value::Array(items) => items.iter().try_for_each(check_numbers),
        Value::Object(map) => map.values().try_for_each(check_numbers),
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
    }
}

fn to_text<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    let text = serde_json::to_string(value).map_err(serde_error)?;
    check_numbers(&serde_json::from_str(&text).map_err(serde_error)?)?;
    Ok(text)
}

pub(crate) fn to_js<T: Serialize + ?Sized>(value: &T) -> Result<JsValue> {
    Ok(js_sys::JSON::parse(&to_text(value)?)?)
}

pub(crate) fn from_js<T: DeserializeOwned>(value: &JsValue) -> Result<T> {
    let text = if value.is_undefined() {
        "null".to_string()
    } else {
        js_sys::JSON::stringify(value)?
            .as_string()
            .unwrap_or_else(|| "null".to_string())
    };
    serde_json::from_str(&text).map_err(serde_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn integers_must_fit_a_browser_number() {
        assert!(to_text(&MAX_SAFE_INTEGER).is_ok());
        assert!(to_text(&-(MAX_SAFE_INTEGER as i64)).is_ok());
        assert!(to_text(&1.5e300).is_ok());
        for err in [
            to_text(&(MAX_SAFE_INTEGER + 1)).unwrap_err(),
            to_text(&u64::MAX).unwrap_err(),
            to_text(&i64::MIN).unwrap_err(),
            to_text(&vec![1u64, 1 << 60]).unwrap_err(),
            to_text(&BTreeMap::from([("k", vec![u64::MAX])])).unwrap_err(),
        ] {
            assert!(err.to_string().contains("store it as a string"), "{err}");
        }
    }

    #[test]
    fn maps_become_objects_with_string_keys() {
        let text = to_text(&BTreeMap::from([(7u32, true), (10, false)])).unwrap();
        assert_eq!(text, r#"{"7":true,"10":false}"#);
    }

    #[test]
    fn struct_fields_keep_their_order() {
        #[derive(Serialize)]
        struct Message {
            tag: &'static str,
            n: u32,
            nested: (u32, Option<u32>),
        }
        let text = to_text(&Message { tag: "st", n: 1, nested: (1, None) }).unwrap();
        assert_eq!(text, r#"{"tag":"st","n":1,"nested":[1,null]}"#);
    }
}
