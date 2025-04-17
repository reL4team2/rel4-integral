use std::collections::HashMap;
use tera::{Result as TeraResult, Value};

pub(crate) fn format_hex(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    if let Some(num) = value.as_u64() {
        Ok(Value::String(format!("0x{:x}", num)))
    } else {
        Ok(Value::String(String::new()))
    }
}
