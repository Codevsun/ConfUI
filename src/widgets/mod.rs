//! Reusable TUI components — tree view logic and visible line computation.
//!
//! This module contains the tree model logic (no ratatui imports).
//! The actual rendering is done in `crate::ui`.

use confui::core::{Path, PathSegment, Value};

/// A single visible line in the tree sidebar.
#[derive(Debug, Clone)]
pub struct TreeLine {
    /// Indentation depth (0 = root).
    pub depth: usize,
    /// The key label (e.g., "server", "[0]").
    pub key: String,
    /// A short summary of the value (e.g., "8080", "\"hello\"", "...").
    pub value_summary: String,
    /// Whether this node's children are currently shown.
    pub is_expanded: bool,
    /// Whether this is a leaf node (string, int, float, bool, null).
    pub is_leaf: bool,
    /// The full path to this node in the tree.
    pub path: Path,
}

/// Generate the flat list of visible tree lines from a `Value` tree.
///
/// Only nodes whose ancestors are all in `expanded` are included.
/// Objects and arrays show a `>` indicator when collapsed and `v` when expanded.
///
/// The root node is rendered as a special "root" line if it's an object or array.
pub fn visible_lines(tree: &Value, expanded: &[Path]) -> Vec<TreeLine> {
    let mut lines = Vec::new();

    // Root is always visible
    let is_root_expanded = expanded.iter().any(|p| p.is_empty());
    let root_key = "root".to_string();
    let root_summary = match tree {
        Value::Object(map) => format!("({} keys)", map.len()),
        Value::Array(arr) => format!("[{} items]", arr.len()),
        leaf => format_leaf_preview(leaf),
    };

    lines.push(TreeLine {
        depth: 0,
        key: root_key,
        value_summary: root_summary,
        is_expanded: is_root_expanded,
        is_leaf: tree.is_leaf(),
        path: Path::new(),
    });

    if is_root_expanded {
        match tree {
            Value::Object(map) => {
                for (key, val) in map.iter() {
                    let child_path: Path = vec![PathSegment::Key(key.clone())];
                    append_children(&mut lines, key, val, &child_path, 1, expanded);
                }
            }
            Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let child_path: Path = vec![PathSegment::Index(i)];
                    let key = format!("[{}]", i);
                    append_children(&mut lines, &key, val, &child_path, 1, expanded);
                }
            }
            _ => {}
        }
    }

    lines
}

/// Recursively append children to the visible lines list.
fn append_children(
    lines: &mut Vec<TreeLine>,
    key: &str,
    value: &Value,
    path: &Path,
    depth: usize,
    expanded: &[Path],
) {
    let is_expanded = expanded.contains(path);
    let summary = if value.is_leaf() {
        format_leaf_preview(value)
    } else {
        match value {
            Value::Object(map) => format!("({} keys)", map.len()),
            Value::Array(arr) => format!("[{} items]", arr.len()),
            _ => format_leaf_preview(value),
        }
    };

    lines.push(TreeLine {
        depth,
        key: key.to_string(),
        value_summary: summary,
        is_expanded,
        is_leaf: value.is_leaf(),
        path: path.clone(),
    });

    if is_expanded {
        match value {
            Value::Object(map) => {
                for (child_key, child_val) in map.iter() {
                    let mut child_path = path.clone();
                    child_path.push(PathSegment::Key(child_key.clone()));
                    append_children(
                        lines,
                        child_key,
                        child_val,
                        &child_path,
                        depth + 1,
                        expanded,
                    );
                }
            }
            Value::Array(arr) => {
                for (i, child_val) in arr.iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(PathSegment::Index(i));
                    let child_key = format!("[{}]", i);
                    append_children(
                        lines,
                        &child_key,
                        child_val,
                        &child_path,
                        depth + 1,
                        expanded,
                    );
                }
            }
            _ => {}
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────

/// Format a leaf value for tree display.
pub fn format_leaf_preview(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            if f.fract() == 0.0 && f.is_finite() {
                format!("{f}.0")
            } else {
                format!("{f}")
            }
        }
        Value::String(s) => {
            if s.len() > 50 {
                format!("\"{}\"...", &s[..47])
            } else {
                format!("\"{s}\"")
            }
        }
        Value::Object(_) => "{...}".to_string(),
        Value::Array(_) => "[...]".to_string(),
    }
}

/// Format a value for the property panel display.
pub fn format_value_detailed(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            if f.fract() == 0.0 && f.is_finite() {
                format!("{f}.0")
            } else {
                format!("{f}")
            }
        }
        Value::String(s) => s.clone(),
        Value::Object(map) => {
            let mut out = String::from("{\n");
            for (k, v) in map.iter() {
                let preview = format_leaf_preview(v);
                out.push_str(&format!("  {k}: {preview}\n"));
            }
            out.push('}');
            out
        }
        Value::Array(arr) => {
            let mut out = String::from("[\n");
            for (i, v) in arr.iter().enumerate() {
                let preview = format_leaf_preview(v);
                out.push_str(&format!("  [{i}]: {preview}\n"));
            }
            out.push(']');
            out
        }
    }
}
