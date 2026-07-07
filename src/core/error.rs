//! Tree operations error types.

use std::fmt;

/// Errors that can occur during tree operations on `Value`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TreeError {
    /// The path does not exist in the tree.
    PathNotFound,

    /// A path segment type does not match the node type.
    /// (operation, expected type, actual type)
    WrongType(String, String, String),

    /// A key already exists in an object (used by `insert`).
    KeyAlreadyExists(String),

    /// An index is out of bounds for an array.
    IndexOutOfBounds(usize),

    /// The operation targets the root node (empty path) which is not allowed.
    RootOperation(String),

    /// The source and destination paths are the same (used by `move`).
    SourceEqualsTarget,
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathNotFound => write!(f, "path not found"),
            Self::WrongType(op, expected, actual) => {
                write!(f, "cannot {op}: expected {expected}, got {actual}")
            }
            Self::KeyAlreadyExists(k) => write!(f, "key already exists: `{k}`"),
            Self::IndexOutOfBounds(i) => write!(f, "index out of bounds: {i}"),
            Self::RootOperation(op) => write!(f, "cannot {op} on root value"),
            Self::SourceEqualsTarget => write!(f, "source and destination paths are the same"),
        }
    }
}

impl std::error::Error for TreeError {}
