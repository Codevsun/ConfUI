//! Generic Value tree model.
//!
//! The [`Value`] enum represents any config value as a tree node.
//! All UI rendering operates on this type — never on raw file text.

use indexmap::IndexMap;

use super::Path;
use super::error::TreeError;
use super::path::PathSegment;

/// A generic config value node in the tree model.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Value {
    /// An ordered map of string keys to child values.
    Object(IndexMap<String, Value>),
    /// A list of child values.
    Array(Vec<Value>),
    /// A UTF-8 string value.
    String(String),
    /// A signed 64-bit integer.
    Int(i64),
    /// A 64-bit floating-point number.
    Float(f64),
    /// A boolean.
    Bool(bool),
    /// A null / empty value.
    Null,
}

#[allow(dead_code)]
impl Value {
    // ── helpers ──────────────────────────────────────────────────────

    /// Returns `true` if this node is a leaf (string, int, float, bool, or null).
    pub fn is_leaf(&self) -> bool {
        matches!(
            self,
            Self::String(_) | Self::Int(_) | Self::Float(_) | Self::Bool(_) | Self::Null
        )
    }

    /// Returns a human-readable type name for this value.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Object(_) => "object",
            Self::Array(_) => "array",
            Self::String(_) => "string",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::Bool(_) => "bool",
            Self::Null => "null",
        }
    }

    // ── get ───────────────────────────────────────────────────────────

    /// Walk the tree following `path` and return a reference to the value at
    /// that location, or `None` if the path does not exist.
    pub fn get(&self, path: &Path) -> Option<&Value> {
        let mut current = self;
        for segment in path {
            current = match segment {
                PathSegment::Key(k) => match current {
                    Self::Object(map) => map.get(k)?,
                    _ => return None,
                },
                PathSegment::Index(i) => match current {
                    Self::Array(arr) => arr.get(*i)?,
                    _ => return None,
                },
            };
        }
        Some(current)
    }

    /// Walk the tree following `path` and return a mutable reference to the
    /// value at that location, or `None` if the path does not exist.
    pub fn get_mut(&mut self, path: &Path) -> Option<&mut Value> {
        let mut current = self;
        for segment in path {
            current = match segment {
                PathSegment::Key(k) => match current {
                    Self::Object(map) => map.get_mut(k)?,
                    _ => return None,
                },
                PathSegment::Index(i) => match current {
                    Self::Array(arr) => arr.get_mut(*i)?,
                    _ => return None,
                },
            };
        }
        Some(current)
    }

    // ── set ───────────────────────────────────────────────────────────

    /// Replace the value at `path` with `value`, returning the old value.
    ///
    /// Returns an error if the path does not exist or if a segment tries to
    /// index into the wrong node type.
    pub fn set(&mut self, path: &Path, value: Value) -> Result<Value, TreeError> {
        if path.is_empty() {
            return Err(TreeError::RootOperation("set".into()));
        }
        let parent_path = &path[..path.len() - 1];
        let last = &path[path.len() - 1];

        match self.get_mut(&parent_path.to_vec()) {
            Some(parent) => match (last, parent) {
                (PathSegment::Key(k), Value::Object(map)) => map
                    .get_mut(k)
                    .map(|old| std::mem::replace(old, value))
                    .ok_or(TreeError::PathNotFound),
                (PathSegment::Index(i), Value::Array(arr)) => arr
                    .get_mut(*i)
                    .map(|old| std::mem::replace(old, value))
                    .ok_or(TreeError::IndexOutOfBounds(*i)),
                (PathSegment::Key(_), actual) => Err(TreeError::WrongType(
                    "set".into(),
                    "object".into(),
                    actual.type_name().into(),
                )),
                (PathSegment::Index(_), actual) => Err(TreeError::WrongType(
                    "set".into(),
                    "array".into(),
                    actual.type_name().into(),
                )),
            },
            None => Err(TreeError::PathNotFound),
        }
    }

    // ── insert ────────────────────────────────────────────────────────

    /// Insert a new key or index at `path` with the given `value`.
    ///
    /// For objects, the parent must exist and be an object, and the key must
    /// not already exist. For arrays, the index must be within bounds or equal
    /// to the length (appending).
    pub fn insert(&mut self, path: &Path, value: Value) -> Result<(), TreeError> {
        if path.is_empty() {
            return Err(TreeError::RootOperation("insert".into()));
        }
        let parent_path = &path[..path.len() - 1];
        let last = &path[path.len() - 1];

        match self.get_mut(&parent_path.to_vec()) {
            Some(parent) => match (last, parent) {
                (PathSegment::Key(k), Value::Object(map)) => {
                    if map.contains_key(k) {
                        return Err(TreeError::KeyAlreadyExists(k.clone()));
                    }
                    map.insert(k.clone(), value);
                    Ok(())
                }
                (PathSegment::Index(i), Value::Array(arr)) => {
                    if *i <= arr.len() {
                        arr.insert(*i, value);
                        Ok(())
                    } else {
                        Err(TreeError::IndexOutOfBounds(*i))
                    }
                }
                (PathSegment::Key(_), actual) => Err(TreeError::WrongType(
                    "insert".into(),
                    "object".into(),
                    actual.type_name().into(),
                )),
                (PathSegment::Index(_), actual) => Err(TreeError::WrongType(
                    "insert".into(),
                    "array".into(),
                    actual.type_name().into(),
                )),
            },
            None => Err(TreeError::PathNotFound),
        }
    }

    // ── delete ────────────────────────────────────────────────────────

    /// Remove the value at `path`, returning it.
    ///
    /// Returns an error if the path does not exist or if a segment tries to
    /// index into the wrong node type.
    pub fn delete(&mut self, path: &Path) -> Result<Value, TreeError> {
        if path.is_empty() {
            return Err(TreeError::RootOperation("delete".into()));
        }
        let parent_path = &path[..path.len() - 1];
        let last = &path[path.len() - 1];

        match self.get_mut(&parent_path.to_vec()) {
            Some(parent) => match (last, parent) {
                (PathSegment::Key(k), Value::Object(map)) => {
                    map.shift_remove(k).ok_or(TreeError::PathNotFound)
                }
                (PathSegment::Index(i), Value::Array(arr)) => {
                    if *i < arr.len() {
                        Ok(arr.remove(*i))
                    } else {
                        Err(TreeError::IndexOutOfBounds(*i))
                    }
                }
                (PathSegment::Key(_), actual) => Err(TreeError::WrongType(
                    "delete".into(),
                    "object".into(),
                    actual.type_name().into(),
                )),
                (PathSegment::Index(_), actual) => Err(TreeError::WrongType(
                    "delete".into(),
                    "array".into(),
                    actual.type_name().into(),
                )),
            },
            None => Err(TreeError::PathNotFound),
        }
    }

    // ── rename_key ────────────────────────────────────────────────────

    /// Rename a key inside an object at the given parent path.
    ///
    /// The last segment of `path` is the key to rename. `new_key` replaces it.
    /// The new key must not already exist.
    pub fn rename_key(&mut self, path: &Path, new_key: &str) -> Result<(), TreeError> {
        if path.is_empty() {
            return Err(TreeError::RootOperation("rename_key".into()));
        }
        let parent_path = &path[..path.len() - 1];
        let last = &path[path.len() - 1];

        let old_key = match last {
            PathSegment::Key(k) => k.clone(),
            PathSegment::Index(_) => {
                return Err(TreeError::WrongType(
                    "rename_key".into(),
                    "object key".into(),
                    "array index".into(),
                ));
            }
        };

        match self.get_mut(&parent_path.to_vec()) {
            Some(Value::Object(map)) => {
                if !map.contains_key(&old_key) {
                    return Err(TreeError::PathNotFound);
                }
                if map.contains_key(new_key) {
                    return Err(TreeError::KeyAlreadyExists(new_key.into()));
                }
                if let Some(val) = map.shift_remove(&old_key) {
                    map.insert(new_key.into(), val);
                    Ok(())
                } else {
                    Err(TreeError::PathNotFound)
                }
            }
            Some(other) => Err(TreeError::WrongType(
                "rename_key".into(),
                "object".into(),
                other.type_name().into(),
            )),
            None => Err(TreeError::PathNotFound),
        }
    }

    // ── move_value ────────────────────────────────────────────────────

    /// Move a value from `from` path to `to` path.
    ///
    /// Both paths must exist (or for arrays, `to` may append at the end).
    /// The source is removed first, then the value is inserted at the
    /// destination.
    pub fn move_value(&mut self, from: &Path, to: &Path) -> Result<(), TreeError> {
        if from == to {
            return Err(TreeError::SourceEqualsTarget);
        }
        let value = self.delete(from)?;
        self.insert(to, value)
    }

    // ── convenience constructors ──────────────────────────────────────

    /// Create an empty object value.
    pub fn object() -> Self {
        Self::Object(IndexMap::new())
    }

    /// Create an empty array value.
    pub fn array() -> Self {
        Self::Array(Vec::new())
    }

    /// Create a string value.
    pub fn string(s: impl Into<String>) -> Self {
        Self::String(s.into())
    }

    /// Create an integer value.
    pub fn int(i: i64) -> Self {
        Self::Int(i)
    }

    /// Create a float value.
    pub fn float(f: f64) -> Self {
        Self::Float(f)
    }

    /// Create a boolean value.
    pub fn bool(b: bool) -> Self {
        Self::Bool(b)
    }
}

// ── From impls ──────────────────────────────────────────────────────

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::String(s.to_owned())
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Self::Int(i)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<Vec<Value>> for Value {
    fn from(arr: Vec<Value>) -> Self {
        Self::Array(arr)
    }
}

impl From<IndexMap<String, Value>> for Value {
    fn from(map: IndexMap<String, Value>) -> Self {
        Self::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path;

    fn sample_tree() -> Value {
        Value::Object(IndexMap::from([
            (
                "server".into(),
                Value::Object(IndexMap::from([
                    ("host".into(), Value::string("0.0.0.0")),
                    ("port".into(), Value::int(8080)),
                    ("enabled".into(), Value::bool(true)),
                ])),
            ),
            (
                "items".into(),
                Value::Array(vec![
                    Value::string("a"),
                    Value::int(42),
                    Value::float(1.5),
                    Value::Null,
                    Value::Object(IndexMap::from([("key".into(), Value::string("val"))])),
                ]),
            ),
            ("count".into(), Value::int(100)),
            ("ratio".into(), Value::float(0.5)),
            ("active".into(), Value::bool(false)),
            ("label".into(), Value::Null),
        ]))
    }

    // ── helpers ───────────────────────────────────────────────────

    #[test]
    fn is_leaf_string() {
        assert!(Value::string("hello").is_leaf());
    }

    #[test]
    fn is_leaf_int() {
        assert!(Value::int(42).is_leaf());
    }

    #[test]
    fn is_leaf_float() {
        assert!(Value::float(1.5).is_leaf());
    }

    #[test]
    fn is_leaf_bool() {
        assert!(Value::bool(true).is_leaf());
    }

    #[test]
    fn is_leaf_null() {
        assert!(Value::Null.is_leaf());
    }

    #[test]
    fn is_not_leaf_object() {
        assert!(!Value::object().is_leaf());
    }

    #[test]
    fn is_not_leaf_array() {
        assert!(!Value::array().is_leaf());
    }

    #[test]
    fn type_name_variants() {
        assert_eq!(Value::object().type_name(), "object");
        assert_eq!(Value::array().type_name(), "array");
        assert_eq!(Value::string("x").type_name(), "string");
        assert_eq!(Value::int(0).type_name(), "int");
        assert_eq!(Value::float(0.0).type_name(), "float");
        assert_eq!(Value::bool(false).type_name(), "bool");
        assert_eq!(Value::Null.type_name(), "null");
    }

    // ── get ───────────────────────────────────────────────────────

    #[test]
    fn get_root_value_empty_path() {
        let tree = sample_tree();
        let val = tree.get(&path![]);
        assert!(val.is_some());
        assert_eq!(val.unwrap().type_name(), "object");
    }

    #[test]
    fn get_existing_key() {
        let tree = sample_tree();
        let val = tree.get(&path!["server"]);
        assert!(val.is_some());
        assert_eq!(val.unwrap().type_name(), "object");
    }

    #[test]
    fn get_nested_value() {
        let tree = sample_tree();
        let val = tree.get(&path!["server", "port"]);
        assert_eq!(val, Some(&Value::int(8080)));
    }

    #[test]
    fn get_nested_string() {
        let tree = sample_tree();
        let val = tree.get(&path!["server", "host"]);
        assert_eq!(val, Some(&Value::string("0.0.0.0")));
    }

    #[test]
    fn get_array_index() {
        let tree = sample_tree();
        let val = tree.get(&path!["items", 0]);
        assert_eq!(val, Some(&Value::string("a")));
    }

    #[test]
    fn get_array_index_int() {
        let tree = sample_tree();
        let val = tree.get(&path!["items", 1]);
        assert_eq!(val, Some(&Value::int(42)));
    }

    #[test]
    fn get_array_index_float() {
        let tree = sample_tree();
        let val = tree.get(&path!["items", 2]);
        assert_eq!(val, Some(&Value::float(1.5)));
    }

    #[test]
    fn get_array_index_null() {
        let tree = sample_tree();
        let val = tree.get(&path!["items", 3]);
        assert_eq!(val, Some(&Value::Null));
    }

    #[test]
    fn get_nested_object_in_array() {
        let tree = sample_tree();
        let val = tree.get(&path!["items", 4, "key"]);
        assert_eq!(val, Some(&Value::string("val")));
    }

    #[test]
    fn get_path_not_found_nonexistent_key() {
        let tree = sample_tree();
        assert_eq!(tree.get(&path!["nope"]), None);
    }

    #[test]
    fn get_path_not_found_wrong_type() {
        let tree = sample_tree();
        assert_eq!(tree.get(&path!["count", "nested"]), None);
    }

    #[test]
    fn get_array_index_out_of_bounds() {
        let tree = sample_tree();
        assert_eq!(tree.get(&path!["items", 99]), None);
    }

    #[test]
    fn get_scalar_from_root() {
        let tree = Value::int(42);
        assert_eq!(tree.get(&path![]), Some(&Value::int(42)));
    }

    // ── get_mut ───────────────────────────────────────────────────

    #[test]
    fn get_mut_and_modify() {
        let mut tree = sample_tree();
        let val = tree.get_mut(&path!["server", "port"]);
        assert!(val.is_some());
        if let Some(Value::Int(port)) = val {
            *port = 9090;
        }
        assert_eq!(tree.get(&path!["server", "port"]), Some(&Value::int(9090)));
    }

    #[test]
    fn get_mut_nonexistent() {
        let mut tree = sample_tree();
        assert!(tree.get_mut(&path!["missing"]).is_none());
    }

    // ── set ───────────────────────────────────────────────────────

    #[test]
    fn set_existing_key() {
        let mut tree = sample_tree();
        let old = tree.set(&path!["server", "port"], Value::int(443)).unwrap();
        assert_eq!(old, Value::int(8080));
        assert_eq!(tree.get(&path!["server", "port"]), Some(&Value::int(443)));
    }

    #[test]
    fn set_array_element() {
        let mut tree = sample_tree();
        let old = tree.set(&path!["items", 0], Value::string("z")).unwrap();
        assert_eq!(old, Value::string("a"));
        assert_eq!(tree.get(&path!["items", 0]), Some(&Value::string("z")));
    }

    #[test]
    fn set_path_not_found() {
        let mut tree = sample_tree();
        let result = tree.set(&path!["server", "missing_key"], Value::int(1));
        assert_eq!(result, Err(TreeError::PathNotFound));
    }

    #[test]
    fn set_wrong_type_object_expected() {
        let mut tree = sample_tree();
        // "count" is an int, not an object
        let result = tree.set(&path!["count", "nested"], Value::int(1));
        assert_eq!(
            result,
            Err(TreeError::WrongType(
                "set".into(),
                "object".into(),
                "int".into()
            ))
        );
    }

    #[test]
    fn set_wrong_type_array_expected() {
        let mut tree = sample_tree();
        // "count" is an int, not an array
        let result = tree.set(&path!["count", 0], Value::int(1));
        assert_eq!(
            result,
            Err(TreeError::WrongType(
                "set".into(),
                "array".into(),
                "int".into()
            ))
        );
    }

    #[test]
    fn set_index_out_of_bounds() {
        let mut tree = sample_tree();
        let result = tree.set(&path!["items", 99], Value::int(1));
        assert_eq!(result, Err(TreeError::IndexOutOfBounds(99)));
    }

    #[test]
    fn set_root_operation_error() {
        let mut tree = sample_tree();
        let result = tree.set(&path![], Value::Null);
        assert_eq!(result, Err(TreeError::RootOperation("set".into())));
    }

    // ── insert ────────────────────────────────────────────────────

    #[test]
    fn insert_new_key_into_object() {
        let mut tree = sample_tree();
        tree.insert(&path!["server", "timeout"], Value::int(30))
            .unwrap();
        assert_eq!(tree.get(&path!["server", "timeout"]), Some(&Value::int(30)));
    }

    #[test]
    fn insert_key_already_exists() {
        let mut tree = sample_tree();
        let result = tree.insert(&path!["server", "host"], Value::string("new"));
        assert_eq!(result, Err(TreeError::KeyAlreadyExists("host".into())));
    }

    #[test]
    fn insert_into_array_at_end() {
        let mut tree = sample_tree();
        tree.insert(&path!["items", 5], Value::string("new_item"))
            .unwrap();
        assert_eq!(
            tree.get(&path!["items", 5]),
            Some(&Value::string("new_item"))
        );
        assert_eq!(tree.get(&path!["items", 0]), Some(&Value::string("a")));
    }

    #[test]
    fn insert_into_array_at_index() {
        let mut tree = sample_tree();
        tree.insert(&path!["items", 0], Value::string("first"))
            .unwrap();
        assert_eq!(tree.get(&path!["items", 0]), Some(&Value::string("first")));
        // original element shifted
        assert_eq!(tree.get(&path!["items", 1]), Some(&Value::string("a")));
    }

    #[test]
    fn insert_index_out_of_bounds() {
        let mut tree = sample_tree();
        let result = tree.insert(&path!["items", 99], Value::string("x"));
        assert_eq!(result, Err(TreeError::IndexOutOfBounds(99)));
    }

    #[test]
    fn insert_wrong_type() {
        let mut tree = sample_tree();
        // "count" is int, not object
        let result = tree.insert(&path!["count", "x"], Value::int(1));
        assert_eq!(
            result,
            Err(TreeError::WrongType(
                "insert".into(),
                "object".into(),
                "int".into()
            ))
        );
    }

    #[test]
    fn insert_root_error() {
        let mut tree = sample_tree();
        let result = tree.insert(&path![], Value::Null);
        assert_eq!(result, Err(TreeError::RootOperation("insert".into())));
    }

    // ── delete ────────────────────────────────────────────────────

    #[test]
    fn delete_existing_key() {
        let mut tree = sample_tree();
        let removed = tree.delete(&path!["server", "port"]).unwrap();
        assert_eq!(removed, Value::int(8080));
        assert_eq!(tree.get(&path!["server", "port"]), None);
    }

    #[test]
    fn delete_array_element() {
        let mut tree = sample_tree();
        let removed = tree.delete(&path!["items", 0]).unwrap();
        assert_eq!(removed, Value::string("a"));
        // remaining elements shift
        assert_eq!(tree.get(&path!["items", 0]), Some(&Value::int(42)));
    }

    #[test]
    fn delete_path_not_found() {
        let mut tree = sample_tree();
        let result = tree.delete(&path!["missing"]);
        assert_eq!(result, Err(TreeError::PathNotFound));
    }

    #[test]
    fn delete_index_out_of_bounds() {
        let mut tree = sample_tree();
        let result = tree.delete(&path!["items", 99]);
        assert_eq!(result, Err(TreeError::IndexOutOfBounds(99)));
    }

    #[test]
    fn delete_wrong_type() {
        let mut tree = sample_tree();
        let result = tree.delete(&path!["count", "x"]);
        assert_eq!(
            result,
            Err(TreeError::WrongType(
                "delete".into(),
                "object".into(),
                "int".into()
            ))
        );
    }

    #[test]
    fn delete_root_error() {
        let mut tree = sample_tree();
        let result = tree.delete(&path![]);
        assert_eq!(result, Err(TreeError::RootOperation("delete".into())));
    }

    // ── rename_key ────────────────────────────────────────────────

    #[test]
    fn rename_existing_key() {
        let mut tree = sample_tree();
        tree.rename_key(&path!["server", "host"], "address")
            .unwrap();
        assert_eq!(tree.get(&path!["server", "host"]), None);
        assert_eq!(
            tree.get(&path!["server", "address"]),
            Some(&Value::string("0.0.0.0"))
        );
    }

    #[test]
    fn rename_key_not_found() {
        let mut tree = sample_tree();
        let result = tree.rename_key(&path!["server", "nope"], "x");
        assert_eq!(result, Err(TreeError::PathNotFound));
    }

    #[test]
    fn rename_key_already_exists() {
        let mut tree = sample_tree();
        let result = tree.rename_key(&path!["server", "host"], "port");
        assert_eq!(result, Err(TreeError::KeyAlreadyExists("port".into())));
    }

    #[test]
    fn rename_on_wrong_type() {
        let mut tree = sample_tree();
        // "count" is int, not object
        let result = tree.rename_key(&path!["count", "x"], "y");
        assert_eq!(
            result,
            Err(TreeError::WrongType(
                "rename_key".into(),
                "object".into(),
                "int".into()
            ))
        );
    }

    #[test]
    fn rename_with_index_segment_error() {
        let mut tree = sample_tree();
        let result = tree.rename_key(&path!["items", 0], "new_name");
        assert_eq!(
            result,
            Err(TreeError::WrongType(
                "rename_key".into(),
                "object key".into(),
                "array index".into()
            ))
        );
    }

    #[test]
    fn rename_root_error() {
        let mut tree = sample_tree();
        let result = tree.rename_key(&path![], "x");
        assert_eq!(result, Err(TreeError::RootOperation("rename_key".into())));
    }

    // ── move_value ────────────────────────────────────────────────

    #[test]
    fn move_between_keys() {
        let mut tree = sample_tree();
        tree.move_value(&path!["server", "host"], &path!["server", "address"])
            .unwrap();
        assert_eq!(tree.get(&path!["server", "host"]), None);
        assert_eq!(
            tree.get(&path!["server", "address"]),
            Some(&Value::string("0.0.0.0"))
        );
    }

    #[test]
    fn move_source_equals_target_error() {
        let mut tree = sample_tree();
        let result = tree.move_value(&path!["server", "host"], &path!["server", "host"]);
        assert_eq!(result, Err(TreeError::SourceEqualsTarget));
    }

    #[test]
    fn move_source_not_found() {
        let mut tree = sample_tree();
        let result = tree.move_value(&path!["missing"], &path!["server", "x"]);
        assert_eq!(result, Err(TreeError::PathNotFound));
    }

    #[test]
    fn move_to_existing_key_error() {
        let mut tree = sample_tree();
        let result = tree.move_value(&path!["server", "host"], &path!["server", "port"]);
        assert_eq!(result, Err(TreeError::KeyAlreadyExists("port".into())));
    }

    // ── from impls ────────────────────────────────────────────────

    #[test]
    fn from_string() {
        let v: Value = "hello".to_string().into();
        assert_eq!(v, Value::string("hello"));
    }

    #[test]
    fn from_str() {
        let v: Value = "hello".into();
        assert_eq!(v, Value::string("hello"));
    }

    #[test]
    fn from_i64() {
        let v: Value = 42i64.into();
        assert_eq!(v, Value::int(42));
    }

    #[test]
    fn from_f64() {
        let v: Value = 1.5f64.into();
        assert_eq!(v, Value::float(1.5));
    }

    #[test]
    fn from_bool() {
        let v: Value = true.into();
        assert_eq!(v, Value::bool(true));
    }

    #[test]
    fn from_vec() {
        let v: Value = vec![Value::int(1), Value::int(2)].into();
        assert_eq!(v, Value::Array(vec![Value::int(1), Value::int(2)]));
    }

    #[test]
    fn from_indexmap() {
        let mut map = IndexMap::new();
        map.insert("a".into(), Value::int(1));
        let v: Value = map.clone().into();
        assert_eq!(v, Value::Object(map));
    }
}
