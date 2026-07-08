//! Example plugin for Cargo.toml files.
//!
//! Provides documentation and validation for common Cargo.toml keys.

use crate::plugins::Plugin;
use crate::validation::{Severity, ValidationMessage};
use confui::core::{Path, PathSegment, Value};

/// Plugin for `Cargo.toml` files.
#[derive(Debug)]
#[allow(dead_code)]
pub struct CargoTomlPlugin;

#[allow(dead_code)]
impl CargoTomlPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Plugin for CargoTomlPlugin {
    fn name(&self) -> &str {
        "Cargo.toml"
    }

    fn description(&self) -> &str {
        "Rust package manifest — docs and validation for common keys"
    }

    fn matches_file(&self, file_name: &str) -> bool {
        file_name == "Cargo.toml"
    }

    fn docs_for(&self, path: &Path) -> Option<String> {
        let key = last_key(path)?;
        let doc = match key {
            "name" => "Package name. Must be a valid Rust identifier (kebab-case).\n\
                       Used as the crate name on crates.io."
                .into(),
            "version" => "Semantic version (e.g. \"0.1.0\"). Should follow semver \
                          for Rust crates."
                .into(),
            "edition" => "Rust edition. Valid values: 2015, 2018, 2021, 2024.\n\
                          Determines which Rust language features are available."
                .into(),
            "authors" => "List of package authors, typically as \"Name <email>\".".into(),
            "description" => "A short description of the package.\n\
                              Displayed on crates.io and in `cargo search`."
                .into(),
            "license" => "SPDX license identifier (e.g. \"MIT\", \"Apache-2.0\").".into(),
            "repository" => "URL to the package's source repository.".into(),
            "homepage" => "URL to the package's homepage.".into(),
            "documentation" => "URL to the package's documentation.\n\
                                Defaults to docs.rs if not set."
                .into(),
            "readme" => "Path to the README file (relative to Cargo.toml).\n\
                         Defaults to \"README.md\"."
                .into(),
            "keywords" => "List of keywords for crates.io search.".into(),
            "categories" => "List of category labels for crates.io.".into(),
            "publish" => "Whether the package can be published to crates.io.\n\
                          Set to `false` to prevent accidental publishing."
                .into(),
            "resolver" => "Dependency resolver version (\"1\" or \"2\").\n\
                           Edition 2021 defaults to \"2\"."
                .into(),
            _ => return None,
        };
        Some(doc)
    }

    fn validate(&self, path: &Path, value: &Value) -> Vec<ValidationMessage> {
        let mut msgs = Vec::new();
        let key = match last_key(path) {
            Some(k) => k,
            None => return msgs,
        };
        match (key, value) {
            ("edition", Value::String(ed)) => {
                let valid = ["2015", "2018", "2021", "2024"];
                if !valid.contains(&ed.as_str()) {
                    msgs.push(ValidationMessage {
                        severity: Severity::Warning,
                        message: format!(
                            "Unknown Rust edition '{ed}'. Valid values: {}",
                            valid.join(", ")
                        ),
                    });
                }
            }
            ("version", Value::String(v)) => {
                // Simple semver check: at least major.minor.patch
                let parts: Vec<&str> = v.split('.').collect();
                if parts.len() < 3 {
                    msgs.push(ValidationMessage {
                        severity: Severity::Warning,
                        message: format!(
                            "Version '{v}' does not follow semver (expected major.minor.patch)"
                        ),
                    });
                }
            }
            ("name", Value::String(n)) => {
                if n.is_empty() {
                    msgs.push(ValidationMessage {
                        severity: Severity::Error,
                        message: "Package name must not be empty".into(),
                    });
                } else if !n
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                {
                    msgs.push(ValidationMessage {
                        severity: Severity::Error,
                        message: format!(
                            "Package name '{n}' contains invalid characters. \
                                          Use only letters, numbers, hyphens, and underscores"
                        ),
                    });
                }
            }
            _ => {}
        }
        msgs
    }
}

/// Extract the last key segment from a path, if it's a `Key` segment.
fn last_key(path: &Path) -> Option<&str> {
    match path.last()? {
        PathSegment::Key(k) => Some(k.as_str()),
        PathSegment::Index(_) => None,
    }
}
