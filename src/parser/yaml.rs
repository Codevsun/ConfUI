//! YAML parser — converts between YAML text and `core::Value`.
//!
//! Uses `serde_json::Value` as the intermediate representation since
//! `serde_yaml` 0.9 works directly with any Serde-compatible type.

use crate::core::Value;

/// Parse a YAML string into a `Value` tree.
pub fn parse_yaml(input: &str) -> color_eyre::Result<Value> {
    let json_val: serde_json::Value = serde_yaml::from_str(input)?;
    Ok(json_val_to_value(json_val))
}

/// Serialize a `Value` tree into a YAML string.
pub fn serialize_yaml(value: &Value) -> color_eyre::Result<String> {
    let json_val = value_to_json_val(value);
    let mut buf = Vec::new();
    serde_yaml::to_writer(&mut buf, &json_val)?;
    let s = String::from_utf8(buf)
        .map_err(|e| color_eyre::eyre::eyre!("YAML output is not UTF-8: {e}"))?;
    // Ensure trailing newline
    if !s.ends_with('\n') {
        Ok(s + "\n")
    } else {
        Ok(s)
    }
}

/// Convert a `serde_json::Value` into our `Value`.
fn json_val_to_value(jv: serde_json::Value) -> Value {
    match jv {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(arr) => {
            Value::Array(arr.into_iter().map(json_val_to_value).collect())
        }
        serde_json::Value::Object(map) => {
            let ordered = map
                .into_iter()
                .map(|(k, v)| (k, json_val_to_value(v)))
                .collect();
            Value::Object(ordered)
        }
    }
}

/// Convert our `Value` into a `serde_json::Value`.
fn value_to_json_val(value: &Value) -> serde_json::Value {
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
        Value::Array(arr) => serde_json::Value::Array(arr.iter().map(value_to_json_val).collect()),
        Value::Object(map) => {
            let mut m = serde_json::Map::new();
            for (k, v) in map {
                m.insert(k.clone(), value_to_json_val(v));
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
    fn parse_simple_yaml() {
        let input = "name: test\ncount: 42\n";
        let val = parse_yaml(input).unwrap();
        assert_eq!(val.get(&vec!["name".into()]), Some(&Value::string("test")));
        assert_eq!(val.get(&vec!["count".into()]), Some(&Value::int(42)));
    }

    #[test]
    fn parse_nested_yaml() {
        let input = "server:\n  host: 0.0.0.0\n  port: 8080\n";
        let val = parse_yaml(input).unwrap();
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
    fn parse_yaml_with_array() {
        let input = "items:\n  - 1\n  - 2\n  - 3\n";
        let val = parse_yaml(input).unwrap();
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
    fn parse_yaml_all_types() {
        let input = r#"
null_val: ~
bool_val: true
int_val: 42
float_val: 3.14
string_val: hello
array_val: [1, 2]
object_val:
  a: 1
"#;
        let val = parse_yaml(input).unwrap();
        assert_eq!(val.get(&vec!["null_val".into()]), Some(&Value::Null));
        assert_eq!(val.get(&vec!["bool_val".into()]), Some(&Value::Bool(true)));
        assert_eq!(val.get(&vec!["int_val".into()]), Some(&Value::int(42)));
        assert_eq!(
            val.get(&vec!["float_val".into()]),
            Some(&Value::float(3.14))
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
    fn round_trip_yaml() {
        let input = r#"
string: hello
int: 42
float: 3.14
bool: true
null: ~
array:
  - 1
  - 2
  - 3
object:
  key: val
"#;
        let val1 = parse_yaml(input).unwrap();
        let output = serialize_yaml(&val1).unwrap();
        let val2 = parse_yaml(&output).unwrap();
        assert_eq!(val1, val2);
    }

    #[test]
    fn round_trip_yaml_fixture() {
        let content = include_str!("../../tests/fixtures/sample.yaml");
        let val1 = parse_yaml(content).unwrap();
        let output = serialize_yaml(&val1).unwrap();
        let val2 = parse_yaml(&output).unwrap();
        assert_eq!(val1, val2);
    }

    #[test]
    fn parse_yaml_invalid_returns_error() {
        let result = parse_yaml(": invalid yaml ::");
        assert!(result.is_err());
    }
}
