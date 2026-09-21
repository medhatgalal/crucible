use serde::Serialize;
use serde_json::Value;

/// UTF-8 JSON, sorted object keys, no insignificant whitespace.
pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let v = serde_json::to_value(value)?;
    serde_json::to_string(&sort_value(v))
}

fn sort_value(v: Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            let mut map = map;
            for k in keys {
                if let Some(val) = map.remove(&k) {
                    out.insert(k, sort_value(val));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_value).collect()),
        other => other,
    }
}
