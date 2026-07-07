//! Inline validation for config values.
//!
//! Checks common constraints such as port ranges, number bounds,
//! URL format, and IP address format. Validation runs on the selected
//! node and results are shown in the property panel.
//!
//! This module has ZERO ratatui or crossterm imports.

use confui::core::{Path, PathSegment, Value};

/// The severity of a validation message.
#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    /// A hard error: the value is invalid.
    Error,
    /// A soft warning: the value is unusual but not necessarily wrong.
    Warning,
}

/// A single validation message produced for a value.
#[derive(Debug, Clone)]
pub struct ValidationMessage {
    /// How serious this message is.
    pub severity: Severity,
    /// Human-readable explanation.
    pub message: String,
}

/// Run all applicable validators on `value` at the given `path`.
pub fn validate_value(path: &Path, value: &Value) -> Vec<ValidationMessage> {
    let mut messages = Vec::new();

    let name = path
        .last()
        .map(|seg| match seg {
            PathSegment::Key(k) => k.as_str(),
            PathSegment::Index(_) => "",
        })
        .unwrap_or("");

    match value {
        Value::Int(i) => validate_int(name, *i, &mut messages),
        Value::Float(f) => validate_float(name, *f, &mut messages),
        Value::String(s) => validate_string(name, s, &mut messages),
        _ => {}
    }

    messages
}

/// Validate an integer value based on its key name.
fn validate_int(name: &str, value: i64, messages: &mut Vec<ValidationMessage>) {
    let lower = name.to_lowercase();

    // Port validation
    if lower.contains("port") {
        if !(1..=65535).contains(&value) {
            messages.push(ValidationMessage {
                severity: Severity::Error,
                message: format!("Port must be between 1 and 65535, got {value}"),
            });
        } else if value < 1024 {
            messages.push(ValidationMessage {
                severity: Severity::Warning,
                message: format!("Port {value} is a well-known port (0-1023); may require root"),
            });
        }
    }

    // Timeout / TTL / lifetime validation (must be positive)
    if (lower.contains("timeout") || lower.contains("ttl") || lower.contains("lifetime"))
        && value <= 0
    {
        messages.push(ValidationMessage {
            severity: Severity::Error,
            message: format!("{name} must be positive, got {value}"),
        });
    }

    // Count / limit / size / max / length validation (must be >= 0)
    if (lower.contains("count")
        || lower.contains("limit")
        || lower.contains("size")
        || lower.contains("max")
        || lower.contains("length"))
        && value < 0
    {
        messages.push(ValidationMessage {
            severity: Severity::Error,
            message: format!("{name} must be non-negative, got {value}"),
        });
    }
}

/// Validate a float value based on its key name.
fn validate_float(name: &str, value: f64, messages: &mut Vec<ValidationMessage>) {
    if value.is_nan() {
        messages.push(ValidationMessage {
            severity: Severity::Error,
            message: "Value is NaN (not a number)".into(),
        });
    }
    if value.is_infinite() {
        messages.push(ValidationMessage {
            severity: Severity::Error,
            message: format!(
                "Value is {} (infinite)",
                if value.is_sign_positive() {
                    "+inf"
                } else {
                    "-inf"
                }
            ),
        });
    }

    let lower = name.to_lowercase();

    // Timeout / rate / probability must be non-negative
    if (lower.contains("timeout") || lower.contains("rate") || lower.contains("probability"))
        && !value.is_nan()
        && value < 0.0
    {
        messages.push(ValidationMessage {
            severity: Severity::Error,
            message: format!("{name} must be non-negative, got {value}"),
        });
    }

    // Ratio / probability should be in [0, 1]
    if (lower.contains("ratio") || lower.contains("probability"))
        && !value.is_nan()
        && !(0.0..=1.0).contains(&value)
    {
        messages.push(ValidationMessage {
            severity: Severity::Warning,
            message: format!("{name} is a ratio/probability but value {value} is outside [0, 1]"),
        });
    }
}

/// Validate a string value based on its key name and content.
fn validate_string(name: &str, value: &str, messages: &mut Vec<ValidationMessage>) {
    let lower = name.to_lowercase();

    // URL validation
    if lower.contains("url")
        || lower.contains("uri")
        || lower.contains("endpoint")
        || lower.contains("host")
        || value.starts_with("http://")
        || value.starts_with("https://")
    {
        if !value.starts_with("http://") && !value.starts_with("https://") {
            // Might still be a valid hostname, just warn about unrecognized scheme
            if value.contains("://") {
                messages.push(ValidationMessage {
                    severity: Severity::Warning,
                    message: format!("Unrecognized URI scheme in '{value}'"),
                });
            }
        } else {
            // Has http(s) scheme, verify basic format
            let without_scheme = value
                .strip_prefix("http://")
                .or_else(|| value.strip_prefix("https://"))
                .unwrap_or("");
            if without_scheme.is_empty() {
                messages.push(ValidationMessage {
                    severity: Severity::Error,
                    message: format!("URL '{value}' has no host"),
                });
            } else if !without_scheme.contains('.')
                && !without_scheme.contains(':')
                && without_scheme != "localhost"
            {
                messages.push(ValidationMessage {
                    severity: Severity::Warning,
                    message: format!("URL '{value}' does not look like a valid host"),
                });
            }
        }
    }

    // IP address validation
    if (lower.contains("ip") || lower.contains("address") || lower == "host")
        && (value.contains('.') || value.contains(':'))
        && value.parse::<std::net::IpAddr>().is_err()
    {
        messages.push(ValidationMessage {
            severity: Severity::Warning,
            message: format!("'{value}' does not appear to be a valid IP address"),
        });
    }

    // Empty string check
    if value.is_empty() {
        messages.push(ValidationMessage {
            severity: Severity::Warning,
            message: "Value is an empty string".into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use confui::path;

    #[test]
    fn validate_port_valid() {
        let path = path!["server", "port"];
        let msgs = validate_value(&path, &Value::Int(8080));
        assert!(msgs.is_empty());
    }

    #[test]
    fn validate_port_out_of_range() {
        let path = path!["server", "port"];
        let msgs = validate_value(&path, &Value::Int(70000));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Error)));
    }

    #[test]
    fn validate_port_well_known_warning() {
        let path = path!["server", "port"];
        let msgs = validate_value(&path, &Value::Int(80));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Warning)));
    }

    #[test]
    fn validate_port_negative() {
        let path = path!["server", "port"];
        let msgs = validate_value(&path, &Value::Int(-1));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Error)));
    }

    #[test]
    fn validate_timeout_positive() {
        let path = path!["connection", "timeout"];
        let msgs = validate_value(&path, &Value::Int(30));
        assert!(msgs.is_empty());
    }

    #[test]
    fn validate_timeout_zero() {
        let path = path!["connection", "timeout"];
        let msgs = validate_value(&path, &Value::Int(0));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Error)));
    }

    #[test]
    fn validate_count_negative() {
        let path = path!["max_connections"];
        let msgs = validate_value(&path, &Value::Int(-5));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Error)));
    }

    #[test]
    fn validate_count_positive() {
        let path = path!["max_connections"];
        let msgs = validate_value(&path, &Value::Int(100));
        assert!(msgs.is_empty());
    }

    #[test]
    fn validate_nan() {
        let path = path!["ratio"];
        let msgs = validate_value(&path, &Value::Float(f64::NAN));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Error)));
    }

    #[test]
    fn validate_infinite() {
        let path = path!["value"];
        let msgs = validate_value(&path, &Value::Float(f64::INFINITY));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Error)));
    }

    #[test]
    fn validate_ratio_out_of_range() {
        let path = path!["threshold", "ratio"];
        let msgs = validate_value(&path, &Value::Float(1.5));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Warning)));
    }

    #[test]
    fn validate_url_valid() {
        let path = path!["api", "url"];
        let msgs = validate_value(&path, &Value::string("https://example.com/api"));
        assert!(msgs.is_empty());
    }

    #[test]
    fn validate_url_empty_host() {
        let path = path!["url"];
        let msgs = validate_value(&path, &Value::string("https://"));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Error)));
    }

    #[test]
    fn validate_ip_valid() {
        let path = path!["server", "host"];
        let msgs = validate_value(&path, &Value::string("192.168.1.1"));
        assert!(msgs.is_empty());
    }

    #[test]
    fn validate_ip_invalid() {
        let path = path!["server", "host"];
        let msgs = validate_value(&path, &Value::string("999.999.999.999"));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Warning)));
    }

    #[test]
    fn validate_ipv6_valid() {
        let path = path!["ip_address"];
        let msgs = validate_value(&path, &Value::string("::1"));
        assert!(msgs.is_empty());
    }

    #[test]
    fn validate_non_port_not_validated() {
        let path = path!["name"];
        let msgs = validate_value(&path, &Value::Int(42));
        assert!(msgs.is_empty());
    }

    #[test]
    fn validate_empty_string() {
        let path = path!["label"];
        let msgs = validate_value(&path, &Value::string(""));
        assert!(msgs.iter().any(|m| matches!(m.severity, Severity::Warning)));
    }

    #[test]
    fn validate_null_no_errors() {
        let path = path!["value"];
        let msgs = validate_value(&path, &Value::Null);
        assert!(msgs.is_empty());
    }

    #[test]
    fn validate_bool_no_errors() {
        let path = path!["enabled"];
        let msgs = validate_value(&path, &Value::Bool(true));
        assert!(msgs.is_empty());
    }
}
