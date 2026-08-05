//! TOML parser — converts between TOML text and `core::Value`.
//!
//! Uses `toml_edit` to preserve formatting and comments on save.

use crate::core::Value;
use indexmap::IndexMap;
use toml_edit::{Document, DocumentMut, Item, Table, TableLike, Value as TomlValue};

/// Parse a TOML string into a `Value` tree.
pub fn parse_toml(input: &str) -> color_eyre::Result<Value> {
    let doc = Document::parse(input.to_owned())?;
    Ok(table_to_value(&*doc))
}

/// Serialize a `Value` tree into a TOML string from scratch.
///
/// This always produces a canonical re-serialization and does not preserve
/// the original document's comments/formatting. Use [`parse_toml_for_edit`]
/// and [`serialize_toml_into`] to keep formatting stable across an edit
/// session.
pub fn serialize_toml(value: &Value) -> color_eyre::Result<String> {
    let mut doc = DocumentMut::new();
    populate_table(&mut *doc, value);
    Ok(doc.to_string())
}

/// Parse a TOML string into a `Value` tree, also returning the editable
/// source document so later saves can preserve untouched comments/formatting.
pub fn parse_toml_for_edit(input: &str) -> color_eyre::Result<(Value, DocumentMut)> {
    let doc: DocumentMut = input.parse()?;
    let value = table_to_value(doc.as_table());
    Ok((value, doc))
}

/// Serialize `value` into `doc` in place and return the resulting TOML text.
///
/// Keys whose value hasn't changed since `doc` was last parsed/synced are
/// left completely untouched, so their comments and formatting survive.
/// Keys with changed or new values are (re)written; nested objects recurse
/// so only the sub-tree that actually changed loses its original formatting.
pub fn serialize_toml_into(doc: &mut DocumentMut, value: &Value) -> color_eyre::Result<String> {
    sync_table(doc.as_table_mut(), value);
    Ok(doc.to_string())
}

/// Recursively reconcile `table` with `value`, preserving items that are
/// unchanged and only rewriting the ones that differ.
fn sync_table(table: &mut dyn TableLike, value: &Value) {
    let map = match value {
        Value::Object(map) => map,
        // Root/table position no longer holds an object — fall back to a
        // full rebuild (this shouldn't happen for well-formed edits).
        _ => {
            let keys: Vec<String> = table.iter().map(|(k, _)| k.to_string()).collect();
            for k in &keys {
                table.remove(k);
            }
            populate_table(table, value);
            return;
        }
    };

    // Drop keys that no longer exist in the new value.
    let existing_keys: Vec<String> = table.iter().map(|(k, _)| k.to_string()).collect();
    for k in &existing_keys {
        if !map.contains_key(k) {
            table.remove(k);
        }
    }

    enum Action {
        Skip,
        Recurse,
        /// Update the value in place, keeping the existing `Item` (and thus
        /// the key's own leading comment/decor) untouched.
        UpdateLeaf,
        /// Fully replace the item — used only for brand-new keys or
        /// structural type changes, where there's no decor to preserve.
        Replace,
    }

    for (key, val) in map {
        let action = match table.get(key) {
            Some(item) if item_to_value(item) == *val => Action::Skip,
            Some(item) if matches!(val, Value::Object(_)) && item.is_table_like() => {
                Action::Recurse
            }
            Some(Item::Value(_)) => Action::UpdateLeaf,
            _ => Action::Replace,
        };
        match action {
            Action::Skip => {}
            Action::Recurse => {
                if let Some(sub) = table.get_mut(key).and_then(|i| i.as_table_like_mut()) {
                    sync_table(sub, val);
                }
            }
            Action::UpdateLeaf => match leaf_to_toml_value(val) {
                Some(mut new_tv) => {
                    if let Some(Item::Value(old_tv)) = table.get(key) {
                        *new_tv.decor_mut() = old_tv.decor().clone();
                    }
                    if let Some(item) = table.get_mut(key) {
                        *item = Item::Value(new_tv);
                    }
                }
                None => {
                    // Not representable as an inline value (e.g. a nested
                    // non-flat object, or null) — fall back to a full replace.
                    set_item(table, key, val);
                }
            },
            Action::Replace => {
                set_item(table, key, val);
            }
        }
    }
}

/// Convert a `TableLike` (document or table) into our `Value::Object`.
fn table_to_value(table: &impl TableLike) -> Value {
    let mut map = IndexMap::new();
    for (key, item) in table.iter() {
        let val = item_to_value(item);
        map.insert(key.to_string(), val);
    }
    Value::Object(map)
}

/// Convert a `toml_edit::Item` into our `Value`.
fn item_to_value(item: &Item) -> Value {
    match item {
        Item::Value(tv) => toml_value_to_value(tv),
        Item::Table(tbl) => table_to_value(tbl),
        Item::ArrayOfTables(arr) => {
            let mut values = Vec::new();
            for tbl in arr.iter() {
                values.push(table_to_value(tbl));
            }
            Value::Array(values)
        }
        Item::None => Value::Null,
    }
}

/// Convert a `toml_edit::Value` into our `Value`.
fn toml_value_to_value(tv: &TomlValue) -> Value {
    match tv {
        TomlValue::String(fs) => Value::String(fs.value().to_owned()),
        TomlValue::Integer(fi) => Value::Int(*fi.value()),
        TomlValue::Float(ff) => Value::Float(*ff.value()),
        TomlValue::Boolean(fb) => Value::Bool(*fb.value()),
        TomlValue::Datetime(fd) => Value::String(fd.value().to_string()),
        TomlValue::Array(arr) => {
            let values: Vec<Value> = arr.iter().map(toml_value_to_value).collect();
            Value::Array(values)
        }
        TomlValue::InlineTable(tbl) => {
            let mut map = IndexMap::new();
            for (k, v) in tbl.iter() {
                map.insert(k.to_string(), toml_value_to_value(v));
            }
            Value::Object(map)
        }
    }
}

/// Populate a TOML table (or document) from our `Value`.
///
/// The value must be an Object; leaf root values are wrapped.
fn populate_table(table: &mut dyn TableLike, value: &Value) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                set_item(table, key, val);
            }
        }
        _ => {
            set_item(table, "value", value);
        }
    }
}

/// Set a key in a TOML table, recursing into nested values.
fn set_item(table: &mut dyn TableLike, key: &str, value: &Value) {
    match value {
        Value::Null => {
            // TOML has no null; skip to avoid invalid TOML.
        }
        Value::Bool(b) => {
            table.insert(key, toml_edit::value(*b));
        }
        Value::Int(i) => {
            table.insert(key, toml_edit::value(*i));
        }
        Value::Float(f) => {
            table.insert(key, toml_edit::value(*f));
        }
        Value::String(s) => {
            table.insert(key, toml_edit::value(s.as_str()));
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                table.insert(key, Item::Value(TomlValue::Array(toml_edit::Array::new())));
            } else if is_array_of_tables(arr) {
                // Use array of tables syntax
                let mut aot = toml_edit::ArrayOfTables::new();
                for elem in arr {
                    if let Value::Object(map) = elem {
                        let mut tbl = Table::new();
                        for (k, v) in map {
                            set_item(&mut tbl, k, v);
                        }
                        aot.push(tbl);
                    }
                }
                table.insert(key, Item::ArrayOfTables(aot));
            } else {
                // Use inline array
                let mut toml_arr = toml_edit::Array::new();
                for elem in arr {
                    if let Some(tv) = leaf_to_toml_value(elem) {
                        toml_arr.push(tv);
                    }
                }
                table.insert(key, Item::Value(TomlValue::Array(toml_arr)));
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                // Empty inline table
                let tbl = toml_edit::InlineTable::new();
                table.insert(key, Item::Value(TomlValue::InlineTable(tbl)));
            } else if is_simple_table(map) {
                // Use inline table for flat objects
                let mut tbl = toml_edit::InlineTable::new();
                for (k, v) in map {
                    if let Some(tv) = leaf_to_toml_value(v) {
                        tbl.insert(k, tv);
                    }
                }
                table.insert(key, Item::Value(TomlValue::InlineTable(tbl)));
            } else {
                // Use a sub-table
                let mut sub = Table::new();
                for (k, v) in map {
                    set_item(&mut sub, k, v);
                }
                table.insert(key, Item::Table(sub));
            }
        }
    }
}

/// Check if an array value should be serialized as an array of tables.
fn is_array_of_tables(arr: &[Value]) -> bool {
    // `[[key]]` array-of-tables syntax requires every element to be a table.
    // A mixed array (objects alongside scalars) must use a plain inline
    // array instead — using array-of-tables here would silently drop every
    // non-object element (see `set_item`'s array-of-tables branch).
    !arr.is_empty() && arr.iter().all(|v| matches!(v, Value::Object(_)))
}

/// Check if an object is "simple" (all values are leaf nodes or arrays).
/// Used to decide inline table vs sub-table for TOML.
fn is_simple_table(map: &IndexMap<String, Value>) -> bool {
    map.values()
        .all(|v| v.is_leaf() || matches!(v, Value::Array(_)))
}

/// Convert a leaf `Value` into a `toml_edit::Value` for use in arrays or inline tables.
fn leaf_to_toml_value(value: &Value) -> Option<TomlValue> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(toml_edit::value(*b).into_value().unwrap()),
        Value::Int(i) => Some(toml_edit::value(*i).into_value().unwrap()),
        Value::Float(f) => Some(toml_edit::value(*f).into_value().unwrap()),
        Value::String(s) => Some(toml_edit::value(s.as_str()).into_value().unwrap()),
        Value::Array(arr) => {
            let mut toml_arr = toml_edit::Array::new();
            for elem in arr {
                if let Some(tv) = leaf_to_toml_value(elem) {
                    toml_arr.push(tv);
                }
            }
            Some(TomlValue::Array(toml_arr))
        }
        Value::Object(map) => {
            if is_simple_table(map) {
                let mut tbl = toml_edit::InlineTable::new();
                for (k, v) in map {
                    if let Some(tv) = leaf_to_toml_value(v) {
                        tbl.insert(k, tv);
                    }
                }
                Some(TomlValue::InlineTable(tbl))
            } else {
                // Complex objects can't be inline; skip
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant)]
    use super::*;

    #[test]
    fn parse_simple_toml() {
        let input = r#"name = "test"
count = 42"#;
        let val = parse_toml(input).unwrap();
        assert_eq!(val.get(&vec!["name".into()]), Some(&Value::string("test")));
        assert_eq!(val.get(&vec!["count".into()]), Some(&Value::int(42)));
    }

    #[test]
    fn parse_nested_toml() {
        let input = r#"[server]
host = "0.0.0.0"
port = 8080"#;
        let val = parse_toml(input).unwrap();
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
    fn parse_toml_with_array() {
        let input = r#"items = [1, 2, 3]"#;
        let val = parse_toml(input).unwrap();
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
    fn serialize_mixed_type_array_keeps_all_elements() {
        use indexmap::IndexMap;
        let mut obj = IndexMap::new();
        obj.insert("a".to_string(), Value::int(1));
        let value = Value::Object(IndexMap::from([(
            "items".to_string(),
            Value::Array(vec![Value::Object(obj), Value::int(2), Value::string("x")]),
        )]));
        let output = serialize_toml(&value).unwrap();
        let round_tripped = parse_toml(&output).unwrap();
        assert_eq!(
            round_tripped, value,
            "mixed-type array must round-trip without dropping elements:\n{output}"
        );
    }

    #[test]
    fn parse_toml_all_types() {
        let input = r#"
bool_val = true
int_val = 42
float_val = 3.14
string_val = "hello"
array_val = [1, 2]

[object_val]
a = 1
"#;
        let val = parse_toml(input).unwrap();
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
    fn round_trip_toml() {
        let input = r#"string = "hello"
int = 42
float = 3.14
bool = true
array = [1, 2, 3]

[object]
key = "val"
"#;
        let val1 = parse_toml(input).unwrap();
        let output = serialize_toml(&val1).unwrap();
        let val2 = parse_toml(&output).unwrap();
        assert_eq!(val1, val2);
    }

    #[test]
    fn round_trip_toml_fixture() {
        let content = include_str!("../../tests/fixtures/sample.toml");
        let val1 = parse_toml(content).unwrap();
        let output = serialize_toml(&val1).unwrap();
        let val2 = parse_toml(&output).unwrap();
        assert_eq!(val1, val2);
    }

    #[test]
    fn parse_toml_invalid_returns_error() {
        let result = parse_toml("not valid toml ==");
        assert!(result.is_err());
    }
}
