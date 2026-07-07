//! JSON parser — converts between JSON text and `core::Value`.

use crate::core::Value;

/// Parse a JSON string into a `Value` tree.
pub fn parse_json(input: &str) -> color_eyre::Result<Value> {
    let json_val: serde_json::Value = serde_json::from_str(input)?;
    Ok(json_to_value(json_val))
}

/// Serialize a `Value` tree into a pretty-printed JSON string.
pub fn serialize_json(value: &Value) -> color_eyre::Result<String> {
    let json_val = value_to_json(value);
    let s = serde_json::to_string_pretty(&json_val)?;
    Ok(s + "\n")
}

/// Convert a `serde_json::Value` into our `Value`.
fn json_to_value(jv: serde_json::Value) -> Value {
    match jv {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                // Very large or special numbers; store as float
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(arr) => Value::Array(arr.into_iter().map(json_to_value).collect()),
        serde_json::Value::Object(map) => {
            let ordered = map
                .into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect();
            Value::Object(ordered)
        }
    }
}

/// Convert our `Value` into a `serde_json::Value`.
fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
        Value::Float(f) => {
            if let Some(n) = serde_json::Number::from_f64(*f) {
                serde_json::Value::Number(n)
            } else {
                serde_json::Value::Number(serde_json::Number::from_f64(0.0).unwrap())
            }
        }
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Array(arr) => serde_json::Value::Array(arr.iter().map(value_to_json).collect()),
        Value::Object(map) => {
            let mut m = serde_json::Map::new();
            for (k, v) in map {
                m.insert(k.clone(), value_to_json(v));
            }
            serde_json::Value::Object(m)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant)]
    use super::*;

    #[test]
    fn parse_simple_json() {
        let input = r#"{"name": "test", "count": 42}"#;
        let val = parse_json(input).unwrap();
        assert_eq!(val.get(&vec!["name".into()]), Some(&Value::string("test")));
        assert_eq!(val.get(&vec!["count".into()]), Some(&Value::int(42)));
    }

    #[test]
    fn parse_nested_json() {
        let input = r#"{"server": {"host": "0.0.0.0", "port": 8080}}"#;
        let val = parse_json(input).unwrap();
        assert_eq!(
            val.get(&vec!["server".into(), "host".into()]),
            Some(&Value::string("0.0.0.0"))
        );
        assert_eq!(
            val.get(&vec!["server".into(), "port".into()]),
            Some(&Value::int(8080))
        );
    }

    #[test]
    fn parse_json_with_array() {
        let input = r#"{"items": [1, 2, 3]}"#;
        let val = parse_json(input).unwrap();
        assert_eq!(
            val.get(&vec!["items".into(), 0usize.into()]),
            Some(&Value::int(1))
        );
        assert_eq!(
            val.get(&vec!["items".into(), 2usize.into()]),
            Some(&Value::int(3))
        );
    }

    #[test]
    fn parse_json_all_types() {
        let input = r#"{
            "null_val": null,
            "bool_val": true,
            "int_val": 42,
            "float_val": 3.14,
            "string_val": "hello",
            "array_val": [1, 2],
            "object_val": {"a": 1}
        }"#;
        let val = parse_json(input).unwrap();
        assert_eq!(val.get(&vec!["null_val".into()]), Some(&Value::Null));
        assert_eq!(val.get(&vec!["bool_val".into()]), Some(&Value::Bool(true)));
        assert_eq!(val.get(&vec!["int_val".into()]), Some(&Value::int(42)));
        assert_eq!(
            val.get(&vec!["float_val".into()]),
            Some(&Value::float(3.14))
        );
        assert_eq!(
            val.get(&vec!["string_val".into()]),
            Some(&Value::string("hello"))
        );
        assert_eq!(
            val.get(&vec!["array_val".into(), 0usize.into()]),
            Some(&Value::int(1))
        );
        assert_eq!(
            val.get(&vec!["object_val".into(), "a".into()]),
            Some(&Value::int(1))
        );
    }

    #[test]
    fn round_trip_json() {
        let input = r#"{
            "string": "hello",
            "int": 42,
            "float": 3.14,
            "bool": true,
            "null": null,
            "array": [1, 2, 3],
            "object": {"key": "val"}
        }"#;
        let val1 = parse_json(input).unwrap();
        let output = serialize_json(&val1).unwrap();
        let val2 = parse_json(&output).unwrap();
        assert_eq!(val1, val2);
    }

    #[test]
    fn round_trip_json_fixture() {
        let content = include_str!("../../tests/fixtures/sample.json");
        let val1 = parse_json(content).unwrap();
        let output = serialize_json(&val1).unwrap();
        let val2 = parse_json(&output).unwrap();
        assert_eq!(val1, val2);
    }

    #[test]
    fn parse_json_invalid_returns_error() {
        let result = parse_json("not valid json");
        assert!(result.is_err());
    }
}
