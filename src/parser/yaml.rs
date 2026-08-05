//! YAML parser — converts between YAML text and `core::Value`.
//!
//! Uses `serde_yaml::Value` as the intermediate representation. This (unlike
//! `serde_json::Value`) can hold NaN/Infinity floats, which YAML represents
//! natively as `.nan`/`.inf`/`-.inf` — routing through JSON's `Value` would
//! silently lose them in both directions.

use crate::core::Value;

/// Parse a YAML string into a `Value` tree.
pub fn parse_yaml(input: &str) -> color_eyre::Result<Value> {
    let yaml_val: serde_yaml::Value = serde_yaml::from_str(input)?;
    Ok(yaml_val_to_value(yaml_val))
}

/// Serialize a `Value` tree into a YAML string.
pub fn serialize_yaml(value: &Value) -> color_eyre::Result<String> {
    // Built directly as a `serde_yaml::Value` (not routed through
    // `serde_json::Value`) so that NaN/Infinity floats — which YAML can
    // represent natively as `.nan`/`.inf`/`-.inf` but JSON cannot — survive
    // instead of silently being coerced to `0.0`.
    let yaml_val = value_to_yaml_val(value);
    let mut buf = Vec::new();
    serde_yaml::to_writer(&mut buf, &yaml_val)?;
    let s = String::from_utf8(buf)
        .map_err(|e| color_eyre::eyre::eyre!("YAML output is not UTF-8: {e}"))?;
    // Ensure trailing newline
    if !s.ends_with('\n') {
        Ok(s + "\n")
    } else {
        Ok(s)
    }
}

/// Convert a `serde_yaml::Value` into our `Value`.
fn yaml_val_to_value(yv: serde_yaml::Value) -> Value {
    match yv {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(b) => Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        serde_yaml::Value::String(s) => Value::String(s),
        serde_yaml::Value::Sequence(arr) => {
            Value::Array(arr.into_iter().map(yaml_val_to_value).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let ordered = map
                .into_iter()
                .map(|(k, v)| (yaml_key_to_string(k), yaml_val_to_value(v)))
                .collect();
            Value::Object(ordered)
        }
        // `!Tag value` — we don't model tags, so keep the underlying value.
        serde_yaml::Value::Tagged(tagged) => yaml_val_to_value(tagged.value),
    }
}

/// Stringify a YAML mapping key. Our `Value::Object` only supports string
/// keys, so non-string keys (e.g. `123:` or `true:`) are converted to their
/// scalar text form.
fn yaml_key_to_string(key: serde_yaml::Value) -> String {
    match key {
        serde_yaml::Value::String(s) => s,
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Null => "null".to_string(),
        other => format!("{other:?}"),
    }
}

/// Convert our `Value` into a `serde_yaml::Value`.
fn value_to_yaml_val(value: &Value) -> serde_yaml::Value {
    match value {
        Value::Null => serde_yaml::Value::Null,
        Value::Bool(b) => serde_yaml::Value::Bool(*b),
        Value::Int(i) => serde_yaml::Value::Number((*i).into()),
        Value::Float(f) => serde_yaml::Value::Number((*f).into()),
        Value::String(s) => serde_yaml::Value::String(s.clone()),
        Value::Array(arr) => {
            serde_yaml::Value::Sequence(arr.iter().map(value_to_yaml_val).collect())
        }
        Value::Object(map) => {
            let mut m = serde_yaml::Mapping::new();
            for (k, v) in map {
                m.insert(serde_yaml::Value::String(k.clone()), value_to_yaml_val(v));
            }
            serde_yaml::Value::Mapping(m)
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
    fn serialize_nan_and_infinity_preserved() {
        let value = Value::Object(indexmap::IndexMap::from([
            ("a".to_string(), Value::float(f64::NAN)),
            ("b".to_string(), Value::float(f64::INFINITY)),
            ("c".to_string(), Value::float(f64::NEG_INFINITY)),
        ]));
        let output = serialize_yaml(&value).unwrap();
        let round_tripped = parse_yaml(&output).unwrap();
        let get = |k: &str| match round_tripped.get(&vec![k.into()]) {
            Some(Value::Float(f)) => *f,
            other => panic!("expected float for {k}, got {other:?}"),
        };
        assert!(get("a").is_nan(), "NaN not preserved:\n{output}");
        assert_eq!(
            get("b"),
            f64::INFINITY,
            "+Infinity not preserved:\n{output}"
        );
        assert_eq!(
            get("c"),
            f64::NEG_INFINITY,
            "-Infinity not preserved:\n{output}"
        );
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
