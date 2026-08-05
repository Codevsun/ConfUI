//! Format detection, parse/serialize for TOML/JSON/YAML.
//!
//! This module is the only place that imports serde, toml_edit, etc.
//! The UI layer never knows what format the file was in.

mod detect;
mod json;
mod toml;
mod yaml;

pub use detect::Format;
pub use json::{parse_json, serialize_json};
pub use toml::{parse_toml, serialize_toml};
pub use yaml::{parse_yaml, serialize_yaml};

use std::path::Path;

use crate::core::Value;

/// Parse a config file's content into a `Value` tree.
///
/// The format is determined by the file path's extension (with content-based
/// fallback via [`Format::detect`]).
pub fn parse(path: &Path, content: &str) -> color_eyre::Result<Value> {
    let format = Format::detect(path, content);
    parse_with_format(content, format)
}

/// Parse config content using an explicitly provided format.
pub fn parse_with_format(content: &str, format: Format) -> color_eyre::Result<Value> {
    match format {
        Format::Json => json::parse_json(content),
        Format::Toml => toml::parse_toml(content),
        Format::Yaml => yaml::parse_yaml(content),
    }
}

/// Serialize a `Value` tree back into a config string.
///
/// The format is determined by the file path's extension.
pub fn serialize(path: &Path, value: &Value) -> color_eyre::Result<String> {
    let format = Format::detect(
        path, "", // empty content — rely on extension only
    );
    serialize_with_format(value, format)
}

/// Serialize using an explicitly provided format.
pub fn serialize_with_format(value: &Value, format: Format) -> color_eyre::Result<String> {
    match format {
        Format::Json => json::serialize_json(value),
        Format::Toml => toml::serialize_toml(value),
        Format::Yaml => yaml::serialize_yaml(value),
    }
}

/// Per-file editing state that lets [`serialize_for_edit`] preserve
/// comments/formatting for keys that haven't changed since the file was
/// opened or last saved. Only TOML carries extra state today; JSON has no
/// comments and `serde_yaml` re-serializes from scratch regardless.
pub enum EditState {
    Toml(toml_edit::DocumentMut),
    Other,
}

/// Parse a config file's content, also returning state that
/// [`serialize_for_edit`] can use to minimize formatting churn on save.
pub fn parse_for_edit(path: &Path, content: &str) -> color_eyre::Result<(Value, EditState)> {
    match Format::detect(path, content) {
        Format::Toml => {
            let (value, doc) = toml::parse_toml_for_edit(content)?;
            Ok((value, EditState::Toml(doc)))
        }
        Format::Json => Ok((json::parse_json(content)?, EditState::Other)),
        Format::Yaml => Ok((yaml::parse_yaml(content)?, EditState::Other)),
    }
}

/// Serialize `value` back into text, using `state` (from [`parse_for_edit`])
/// to preserve formatting where possible.
pub fn serialize_for_edit(
    path: &Path,
    value: &Value,
    state: &mut EditState,
) -> color_eyre::Result<String> {
    match (Format::detect(path, ""), state) {
        (Format::Toml, EditState::Toml(doc)) => toml::serialize_toml_into(doc, value),
        (Format::Toml, EditState::Other) => toml::serialize_toml(value),
        (Format::Json, _) => json::serialize_json(value),
        (Format::Yaml, _) => yaml::serialize_yaml(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("fixtures");
        p.push(name);
        p
    }

    #[test]
    fn parse_json_fixture() {
        let path = fixture_path("sample.json");
        let content = std::fs::read_to_string(&path).unwrap();
        let val = parse(&path, &content).unwrap();
        assert_eq!(val.type_name(), "object");
        assert!(val.get(&vec!["server".into()]).is_some());
        assert_eq!(
            val.get(&vec!["server".into(), "port".into()]),
            Some(&Value::int(8080))
        );
    }

    #[test]
    fn parse_toml_fixture() {
        let path = fixture_path("sample.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        let val = parse(&path, &content).unwrap();
        assert_eq!(val.type_name(), "object");
        assert!(val.get(&vec!["server".into()]).is_some());
        assert_eq!(
            val.get(&vec!["server".into(), "port".into()]),
            Some(&Value::int(8080))
        );
    }

    #[test]
    fn parse_yaml_fixture() {
        let path = fixture_path("sample.yaml");
        let content = std::fs::read_to_string(&path).unwrap();
        let val = parse(&path, &content).unwrap();
        assert_eq!(val.type_name(), "object");
        assert!(val.get(&vec!["server".into()]).is_some());
        assert_eq!(
            val.get(&vec!["server".into(), "port".into()]),
            Some(&Value::int(8080))
        );
    }

    #[test]
    fn round_trip_json_fixture() {
        let path = fixture_path("sample.json");
        let content = std::fs::read_to_string(&path).unwrap();
        let val1 = parse(&path, &content).unwrap();
        let serialized = serialize(&path, &val1).unwrap();
        let val2 = parse(&path, &serialized).unwrap();
        assert_eq!(val1, val2);
    }

    #[test]
    fn round_trip_toml_fixture() {
        let path = fixture_path("sample.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        let val1 = parse(&path, &content).unwrap();
        let serialized = serialize(&path, &val1).unwrap();
        let val2 = parse(&path, &serialized).unwrap();
        assert_eq!(val1, val2);
    }

    #[test]
    fn round_trip_yaml_fixture() {
        let path = fixture_path("sample.yaml");
        let content = std::fs::read_to_string(&path).unwrap();
        let val1 = parse(&path, &content).unwrap();
        let serialized = serialize(&path, &val1).unwrap();
        let val2 = parse(&path, &serialized).unwrap();
        assert_eq!(val1, val2);
    }

    #[test]
    fn detect_json_format_then_parse() {
        let path = Path::new("config.json");
        let content = r#"{"key": "value"}"#;
        assert_eq!(Format::detect(path, content), Format::Json);
        let val = parse(path, content).unwrap();
        assert_eq!(val.get(&vec!["key".into()]), Some(&Value::string("value")));
    }

    #[test]
    fn detect_yaml_format_then_parse() {
        let path = Path::new("config.yaml");
        let content = "key: value\n";
        let val = parse(path, content).unwrap();
        assert_eq!(val.get(&vec!["key".into()]), Some(&Value::string("value")));
    }

    #[test]
    fn detect_toml_format_then_parse() {
        let path = Path::new("config.toml");
        let content = r#"key = "value""#;
        let val = parse(path, content).unwrap();
        assert_eq!(val.get(&vec!["key".into()]), Some(&Value::string("value")));
    }
}
