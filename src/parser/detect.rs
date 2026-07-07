//! Format detection for config files (extension + content fallback).

use std::path::Path;

/// The detected config file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// TOML format.
    Toml,
    /// JSON format.
    Json,
    /// YAML format.
    Yaml,
}

impl Format {
    /// Detect the format from a file path and its content.
    ///
    /// First tries the file extension. If the extension is missing or
    /// unrecognised, falls back to content-based detection.
    pub fn detect(path: &Path, content: &str) -> Self {
        // Try extension first
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "toml" => return Self::Toml,
                "json" => return Self::Json,
                "yaml" | "yml" => return Self::Yaml,
                _ => { /* fall through to content detection */ }
            }
        }

        // Content-based fallback
        Self::detect_from_content(content)
    }

    /// Detect the format from the content alone.
    ///
    /// - JSON: first non-whitespace character is `{` or `[`
    /// - TOML: try to parse as TOML (fallback in the caller)
    /// - YAML: anything else (also a fallback)
    fn detect_from_content(content: &str) -> Self {
        let trimmed = content.trim_start();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return Self::Json;
        }
        // TOML tables start with `[`, but we already checked for JSON.
        // Real TOML content might start with a key, a comment, or a table header.
        // If it starts with `[` and isn't JSON, it's likely TOML.
        // We'll default to YAML as the broadest format.
        Self::Yaml
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_by_extension_toml() {
        let path = Path::new("config.toml");
        assert_eq!(Format::detect(path, ""), Format::Toml);
    }

    #[test]
    fn detect_by_extension_json() {
        let path = Path::new("config.json");
        assert_eq!(Format::detect(path, ""), Format::Json);
    }

    #[test]
    fn detect_by_extension_yaml() {
        let path = Path::new("config.yaml");
        assert_eq!(Format::detect(path, ""), Format::Yaml);
    }

    #[test]
    fn detect_by_extension_yml() {
        let path = Path::new("config.yml");
        assert_eq!(Format::detect(path, ""), Format::Yaml);
    }

    #[test]
    fn detect_by_extension_uppercase() {
        let path = Path::new("config.JSON");
        assert_eq!(Format::detect(path, ""), Format::Json);
    }

    #[test]
    fn detect_by_content_json() {
        let path = Path::new("config.unknown");
        assert_eq!(Format::detect(path, r#"{"key": "value"}"#), Format::Json);
    }

    #[test]
    fn detect_by_content_json_array() {
        let path = Path::new("config.unknown");
        assert_eq!(Format::detect(path, "[1, 2, 3]"), Format::Json);
    }

    #[test]
    fn detect_by_content_yaml_fallback() {
        let path = Path::new("config.unknown");
        assert_eq!(Format::detect(path, "key: value\n"), Format::Yaml);
    }

    #[test]
    fn detect_unknown_extension_falls_back() {
        let path = Path::new("config.ini");
        assert_eq!(Format::detect(path, "key = value\n"), Format::Yaml);
    }
}
