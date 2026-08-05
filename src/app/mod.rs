//! Application state and main event loop.

use std::path::Path;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;

use crate::history::History;
use crate::plugins::PluginRegistry;
use crate::plugins::cargo_toml::CargoTomlPlugin;
use crate::theme::{self, Theme};
use crate::widgets::visible_lines;
use confui::core::{Path as TreePath, PathSegment, Value};
use confui::parser::{self, Format};

use crate::ui;

/// Application state for the TUI.
#[allow(dead_code)]
pub struct App {
    /// The parsed config value tree.
    pub tree: Value,
    /// The file path (for serialization / display).
    pub file_path: std::path::PathBuf,
    /// Detected format.
    pub format: Format,
    /// Index into the visible tree lines for cursor position.
    pub cursor_index: usize,
    /// Set of paths that are expanded (objects/arrays whose children are shown).
    pub expanded: Vec<TreePath>,
    /// Vertical scroll offset in the tree sidebar.
    pub scroll_offset: usize,
    /// Property panel scroll offset.
    pub property_scroll: usize,
    /// A message shown in the status bar.
    pub status: String,
    /// Whether the file has been modified since last save.
    pub modified: bool,
    /// Whether the app should exit.
    pub should_quit: bool,
    /// Undo/redo history.
    pub history: History,
    /// Whether we are currently in inline edit mode for a leaf value.
    pub editing: bool,
    /// The path of the node being edited.
    pub edit_path: TreePath,
    /// The current edit buffer content.
    pub edit_buffer: String,
    /// Cursor position (byte index) within the edit buffer.
    pub edit_cursor: usize,
    /// Whether we're showing a delete confirmation.
    pub confirming_delete: bool,
    /// The path of the node pending delete confirmation.
    pub confirm_delete_path: TreePath,
    /// Whether we're choosing a type for a new node (after pressing `i`).
    pub inserting: bool,
    /// The container (object/array) the new node will be inserted into.
    pub insert_container_path: TreePath,
    /// Whether we're in rename mode (for object keys).
    pub renaming: bool,
    /// The path of the node being renamed.
    pub rename_path: TreePath,
    /// The rename buffer content.
    pub rename_buffer: String,
    /// Clipboard for cut/paste operations.
    pub clipboard: Option<(TreePath, Value)>,
    /// The current theme index into PRESETS.
    pub theme_index: usize,
    /// The current theme.
    pub theme: &'static Theme,
    /// Plugin registry.
    pub plugins: PluginRegistry,
    /// The currently active plugin for the open file.
    pub active_plugin: Option<String>,
    /// Format-specific state used to preserve formatting/comments on save.
    pub edit_state: parser::EditState,
    /// Whether we are in search input mode.
    pub searching: bool,
    /// The current search query.
    pub search_query: String,
    /// Paths of nodes matching the current search query.
    pub search_results: Vec<TreePath>,
    /// Index into search_results for the current match.
    pub search_index: usize,
}

impl App {
    /// Create a new App by parsing the file at `file_path`.
    pub fn new(file_path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| color_eyre::eyre::eyre!("Cannot read {}: {}", file_path.display(), e))?;
        let format = parser::Format::detect(file_path, &content);
        let (tree, edit_state) = parser::parse_for_edit(file_path, &content)?;

        let mut expanded = Vec::new();
        // Root is automatically expanded
        expanded.push(TreePath::new());
        // Auto-expand all top-level objects/arrays that are the only child of root
        if let Value::Object(map) = &tree {
            for key in map.keys() {
                if let Some(val) = tree.get(&vec![PathSegment::Key(key.clone())])
                    && !val.is_leaf()
                {
                    expanded.push(vec![PathSegment::Key(key.clone())]);
                }
            }
        }

        let status = format!(
            "{} — {}  |  ↑↓ navigate  ← collapse  → expand  Enter edit  Ctrl+S save  q quit",
            file_path
                .file_name()
                .map(|s| s.to_string_lossy())
                .unwrap_or_default(),
            format_name(format),
        );

        let history = History::new(tree.clone());

        let file_name = file_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Set up plugins
        let mut plugins = PluginRegistry::new();
        plugins.register(Box::new(CargoTomlPlugin));
        let active_plugin = plugins
            .find_for_file(&file_name)
            .map(|p| p.name().to_string());

        Ok(Self {
            tree,
            file_path: file_path.to_path_buf(),
            format,
            cursor_index: 0,
            expanded,
            scroll_offset: 0,
            property_scroll: 0,
            status,
            modified: false,
            should_quit: false,
            history,
            editing: false,
            edit_path: TreePath::new(),
            edit_buffer: String::new(),
            edit_cursor: 0,
            confirming_delete: false,
            confirm_delete_path: TreePath::new(),
            inserting: false,
            insert_container_path: TreePath::new(),
            renaming: false,
            rename_path: TreePath::new(),
            rename_buffer: String::new(),
            clipboard: None,
            searching: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_index: 0,
            theme_index: 0,
            theme: &theme::DARK,
            plugins,
            active_plugin,
            edit_state,
        })
    }

    /// Move cursor up one visible line.
    pub fn cursor_up(&mut self) {
        let lines = self.compute_visible_lines();
        if self.cursor_index > 0 {
            self.cursor_index -= 1;
        }
        self.ensure_cursor_visible(&lines);
    }

    /// Move cursor down one visible line.
    pub fn cursor_down(&mut self) {
        let lines = self.compute_visible_lines();
        let max = lines.len().saturating_sub(1);
        if self.cursor_index < max {
            self.cursor_index += 1;
        }
        self.ensure_cursor_visible(&lines);
    }

    /// Move cursor to the top of the tree.
    pub fn cursor_home(&mut self) {
        self.cursor_index = 0;
        self.scroll_offset = 0;
    }

    /// Move cursor to the bottom of the tree.
    pub fn cursor_end(&mut self) {
        let lines = self.compute_visible_lines();
        self.cursor_index = lines.len().saturating_sub(1);
        self.ensure_cursor_visible(&lines);
    }

    /// Move cursor up by one page.
    pub fn cursor_page_up(&mut self) {
        let height = 20; // approximate page size
        for _ in 0..height {
            if self.cursor_index > 0 {
                self.cursor_index -= 1;
            }
        }
        let lines = self.compute_visible_lines();
        self.ensure_cursor_visible(&lines);
    }

    /// Move cursor down by one page.
    pub fn cursor_page_down(&mut self) {
        let height = 20;
        let lines = self.compute_visible_lines();
        let max = lines.len().saturating_sub(1);
        for _ in 0..height {
            if self.cursor_index < max {
                self.cursor_index += 1;
            }
        }
        self.ensure_cursor_visible(&lines);
    }

    /// Expand the current node if it's collapsible.
    pub fn expand(&mut self) {
        let lines = self.compute_visible_lines();
        if let Some(line) = lines.get(self.cursor_index)
            && !line.is_leaf
            && !self.is_expanded(&line.path)
        {
            self.expanded.push(line.path.clone());
        }
    }

    /// Collapse the current node if it's expanded.
    pub fn collapse(&mut self) {
        let lines = self.compute_visible_lines();
        if let Some(line) = lines.get(self.cursor_index)
            && self.is_expanded(&line.path)
        {
            self.expanded.retain(|p| *p != line.path);
        }
    }

    /// Toggle expand/collapse for the current node.
    pub fn toggle_expand(&mut self) {
        let lines = self.compute_visible_lines();
        if let Some(line) = lines.get(self.cursor_index) {
            if self.is_expanded(&line.path) {
                self.expanded.retain(|p| *p != line.path);
            } else if !line.is_leaf {
                self.expanded.push(line.path.clone());
            }
        }
    }

    /// Expand all nodes recursively.
    pub fn expand_all(&mut self) {
        self.expanded.clear();
        collect_all_paths(&self.tree, &TreePath::new(), &mut self.expanded);
    }

    /// Collapse all nodes (keeping root expanded).
    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        // Keep root expanded
        self.expanded.push(TreePath::new());
    }

    /// Get the current cursor path in the tree.
    pub fn cursor_path(&self) -> TreePath {
        let lines = self.compute_visible_lines();
        lines
            .get(self.cursor_index)
            .map(|l| l.path.clone())
            .unwrap_or_default()
    }

    // ── helpers ──────────────────────────────────────────────────

    /// Check if a path is currently expanded.
    fn is_expanded(&self, path: &TreePath) -> bool {
        self.expanded.contains(path)
    }

    /// Compute the flat list of visible tree lines.
    pub fn compute_visible_lines(&self) -> Vec<crate::widgets::TreeLine> {
        visible_lines(&self.tree, &self.expanded)
    }

    /// Ensure the cursor is visible by adjusting scroll_offset.
    fn ensure_cursor_visible(&mut self, _lines: &[crate::widgets::TreeLine]) {
        let height = 20; // approximate visible area
        if self.cursor_index < self.scroll_offset {
            self.scroll_offset = self.cursor_index;
        } else if self.cursor_index >= self.scroll_offset + height {
            self.scroll_offset = self.cursor_index.saturating_sub(height).saturating_add(1);
        }
    }

    // ── editing ──────────────────────────────────────────────────

    /// Start editing the value at the current cursor position.
    ///
    /// For booleans, toggles immediately instead of entering buffer mode.
    /// For other leaf types, populates the edit buffer with the current value.
    pub fn start_edit(&mut self) {
        if self.editing {
            return;
        }
        let cursor_path = self.cursor_path();
        if cursor_path.is_empty() {
            // Cannot edit root
            return;
        }
        let value = match self.tree.get(&cursor_path) {
            Some(v) => v.clone(),
            None => return,
        };
        match value {
            Value::Bool(b) => {
                // Toggle boolean immediately
                let new_val = Value::Bool(!b);
                self.history.record(self.tree.clone());
                if self.tree.set(&cursor_path, new_val).is_ok() {
                    self.modified = true;
                    self.status = "Toggled bool — Ctrl+S to save  Ctrl+Z undo".into();
                }
            }
            Value::Null => {
                self.status = "Cannot edit null value".into();
            }
            _ => {
                let buffer = match &value {
                    Value::String(s) => s.clone(),
                    Value::Int(i) => i.to_string(),
                    Value::Float(f) => format_float_for_edit(*f),
                    _ => return,
                };
                self.editing = true;
                self.edit_path = cursor_path;
                self.edit_buffer = buffer;
                self.edit_cursor = self.edit_buffer.len();
                self.status = "Editing — Enter confirm  Esc cancel".into();
            }
        }
    }

    /// Commit the current edit buffer to the tree.
    pub fn commit_edit(&mut self) {
        if !self.editing {
            return;
        }
        let current = match self.tree.get(&self.edit_path) {
            Some(v) => v.clone(),
            None => {
                self.editing = false;
                return;
            }
        };
        let new_value = match &current {
            Value::String(_) => Value::String(self.edit_buffer.clone()),
            Value::Int(_) => {
                let trimmed = self.edit_buffer.trim();
                match trimmed.parse::<i64>() {
                    Ok(i) => Value::Int(i),
                    Err(_) => {
                        self.status = format!("Invalid integer: '{}'", self.edit_buffer);
                        return;
                    }
                }
            }
            Value::Float(_) => {
                let trimmed = self.edit_buffer.trim();
                match trimmed.parse::<f64>() {
                    Ok(f) => Value::Float(f),
                    Err(_) => {
                        self.status = format!("Invalid float: '{}'", self.edit_buffer);
                        return;
                    }
                }
            }
            _ => {
                self.editing = false;
                return;
            }
        };
        let path = self.edit_path.clone();
        self.editing = false;
        self.history.record(self.tree.clone());
        if self.tree.set(&path, new_value).is_ok() {
            self.modified = true;
            self.status = "Value updated — Ctrl+S to save  Ctrl+Z undo".into();
        }
    }

    /// Cancel editing and restore the previous state.
    pub fn cancel_edit(&mut self) {
        self.editing = false;
        self.status = "Edit cancelled".into();
    }

    /// Insert a character at the edit cursor position.
    pub fn edit_insert_char(&mut self, c: char) {
        if !self.editing {
            return;
        }
        if self.edit_cursor > self.edit_buffer.len() {
            self.edit_cursor = self.edit_buffer.len();
        }
        self.edit_buffer.insert(self.edit_cursor, c);
        self.edit_cursor += c.len_utf8();
    }

    /// Delete the character before the edit cursor (Backspace).
    pub fn edit_delete_before_cursor(&mut self) {
        if !self.editing || self.edit_cursor == 0 {
            return;
        }
        if self.edit_cursor > self.edit_buffer.len() {
            self.edit_cursor = self.edit_buffer.len();
        }
        let prev = self.edit_buffer[..self.edit_cursor]
            .chars()
            .next_back()
            .map(|c| c.len_utf8())
            .unwrap_or(0);
        self.edit_buffer
            .drain(self.edit_cursor - prev..self.edit_cursor);
        self.edit_cursor -= prev;
    }

    /// Delete the character at the edit cursor (Delete).
    pub fn edit_delete_at_cursor(&mut self) {
        if !self.editing || self.edit_cursor >= self.edit_buffer.len() {
            return;
        }
        if self.edit_cursor > self.edit_buffer.len() {
            self.edit_cursor = self.edit_buffer.len();
        }
        let next = self.edit_buffer[self.edit_cursor..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0);
        self.edit_buffer
            .drain(self.edit_cursor..self.edit_cursor + next);
    }

    /// Move the edit cursor left by one character.
    pub fn edit_cursor_left(&mut self) {
        if !self.editing || self.edit_cursor == 0 {
            return;
        }
        if self.edit_cursor > self.edit_buffer.len() {
            self.edit_cursor = self.edit_buffer.len();
        }
        let prev = self.edit_buffer[..self.edit_cursor]
            .chars()
            .next_back()
            .map(|c| c.len_utf8())
            .unwrap_or(0);
        self.edit_cursor = self.edit_cursor.saturating_sub(prev);
    }

    /// Move the edit cursor right by one character.
    pub fn edit_cursor_right(&mut self) {
        if !self.editing || self.edit_cursor >= self.edit_buffer.len() {
            return;
        }
        if self.edit_cursor > self.edit_buffer.len() {
            self.edit_cursor = self.edit_buffer.len();
        }
        let next = self.edit_buffer[self.edit_cursor..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0);
        self.edit_cursor = self.edit_cursor.saturating_add(next);
    }

    /// Move edit cursor to the start of the buffer.
    pub fn edit_cursor_home(&mut self) {
        if self.editing {
            self.edit_cursor = 0;
        }
    }

    /// Move edit cursor to the end of the buffer.
    pub fn edit_cursor_end(&mut self) {
        if self.editing {
            self.edit_cursor = self.edit_buffer.len();
        }
    }

    // ── structure editing ──────────────────────────────────────────

    /// Begin inserting a new node: open the type picker.
    ///
    /// If the cursor is on a container (object/array — including the root),
    /// the new node is inserted as a child of it. Otherwise it's inserted as
    /// a sibling within the cursor's parent container.
    pub fn insert_node(&mut self) {
        if self.editing || self.renaming || self.confirming_delete || self.inserting {
            return;
        }
        let cursor_path = self.cursor_path();
        let container_path = match self.tree.get(&cursor_path) {
            Some(Value::Object(_)) | Some(Value::Array(_)) => cursor_path,
            _ if !cursor_path.is_empty() => cursor_path[..cursor_path.len() - 1].to_vec(),
            _ => {
                self.status = "Cannot insert here".into();
                return;
            }
        };
        match self.tree.get(&container_path) {
            Some(Value::Object(_)) | Some(Value::Array(_)) => {
                self.inserting = true;
                self.insert_container_path = container_path;
                self.status =
                    "Insert: s string  n int  f float  b bool  a array  o object  Esc cancel"
                        .into();
            }
            _ => {
                self.status = "Cannot insert: not inside an object or array".into();
            }
        }
    }

    /// Finish an insert: place `value` as a new key/item inside
    /// `insert_container_path`.
    pub fn commit_insert(&mut self, value: Value) {
        if !self.inserting {
            return;
        }
        self.inserting = false;
        let container_path = self.insert_container_path.clone();
        let type_name = value.type_name();
        match self.tree.get(&container_path).cloned() {
            Some(Value::Object(map)) => {
                // Pick a key that doesn't already exist — otherwise
                // inserting twice into the same object would collide.
                let mut new_key = "new_key".to_string();
                let mut n = 1;
                while map.contains_key(&new_key) {
                    n += 1;
                    new_key = format!("new_key_{n}");
                }
                let mut insert_path = container_path.clone();
                insert_path.push(PathSegment::Key(new_key.clone()));
                let snapshot = self.tree.clone();
                if self.tree.insert(&insert_path, value).is_ok() {
                    self.history.record(snapshot);
                    self.modified = true;
                    // Expand the container so the new key is visible
                    if !self.expanded.contains(&container_path) {
                        self.expanded.push(container_path.clone());
                    }
                    self.status = format!("Inserted {type_name} '{new_key}' — Ctrl+S save");
                } else {
                    self.status = "Could not insert new key".into();
                }
            }
            Some(Value::Array(arr)) => {
                let insert_idx = arr.len();
                let mut insert_path = container_path.clone();
                insert_path.push(PathSegment::Index(insert_idx));
                let snapshot = self.tree.clone();
                if self.tree.insert(&insert_path, value).is_ok() {
                    self.history.record(snapshot);
                    self.modified = true;
                    if !self.expanded.contains(&container_path) {
                        self.expanded.push(container_path.clone());
                    }
                    self.status = format!("Inserted {type_name} item [{insert_idx}] — Ctrl+S save");
                } else {
                    self.status = "Could not insert array item".into();
                }
            }
            _ => {
                self.status = "Could not insert: target no longer valid".into();
            }
        }
    }

    /// Cancel the pending insert type picker.
    pub fn cancel_insert(&mut self) {
        if self.inserting {
            self.inserting = false;
            self.status = "Insert cancelled".into();
        }
    }

    /// Initiate delete confirmation for the current node.
    pub fn delete_node(&mut self) {
        if self.editing || self.renaming || self.inserting {
            return;
        }
        let cursor_path = self.cursor_path();
        if cursor_path.is_empty() {
            self.status = "Cannot delete root".into();
            return;
        }
        if self.confirming_delete {
            // Second press: confirm
            self.confirm_delete();
        } else {
            // First press: ask for confirmation
            self.confirming_delete = true;
            self.confirm_delete_path = cursor_path;
            self.status = "Delete? Press d again to confirm, Esc to cancel".into();
        }
    }

    /// Confirm and execute the pending delete.
    fn confirm_delete(&mut self) {
        let path = self.confirm_delete_path.clone();
        self.confirming_delete = false;
        self.history.record(self.tree.clone());
        match self.tree.delete(&path) {
            Ok(removed) => {
                self.modified = true;
                self.status = format!("Deleted {} — Ctrl+Z undo  Ctrl+S save", removed.type_name());
            }
            Err(e) => {
                self.status = format!("Delete error: {}", e);
            }
        }
    }

    /// Cancel the pending delete confirmation.
    pub fn cancel_delete(&mut self) {
        if self.confirming_delete {
            self.confirming_delete = false;
            self.status = "Delete cancelled".into();
        }
    }

    /// Start renaming the current key (only valid for object keys).
    pub fn start_rename(&mut self) {
        if self.editing || self.renaming || self.confirming_delete || self.inserting {
            return;
        }
        let cursor_path = self.cursor_path();
        if cursor_path.is_empty() {
            self.status = "Cannot rename root".into();
            return;
        }
        let last = cursor_path.last().unwrap();
        match last {
            PathSegment::Key(k) => {
                self.renaming = true;
                self.rename_path = cursor_path.clone();
                self.rename_buffer = k.clone();
                self.status = "Rename — Enter confirm  Esc cancel".into();
            }
            PathSegment::Index(_) => {
                self.status = "Cannot rename array indices, use cut/paste to reorder".into();
            }
        }
    }

    /// Commit the current rename.
    pub fn commit_rename(&mut self) {
        if !self.renaming {
            return;
        }
        let new_key = self.rename_buffer.trim().to_string();
        if new_key.is_empty() {
            self.status = "Key name cannot be empty".into();
            return;
        }
        let path = self.rename_path.clone();
        self.renaming = false;
        let snapshot = self.tree.clone();
        match self.tree.rename_key(&path, &new_key) {
            Ok(()) => {
                // Only record history on success — otherwise a rejected
                // rename (e.g. duplicate key) would push a no-op snapshot,
                // requiring an extra Ctrl+Z to get past it.
                self.history.record(snapshot);
                self.modified = true;
                self.status = format!("Renamed to '{}' — Ctrl+Z undo", new_key);
            }
            Err(e) => {
                self.status = format!("Rename error: {}", e);
            }
        }
    }

    /// Cancel rename mode.
    pub fn cancel_rename(&mut self) {
        if self.renaming {
            self.renaming = false;
            self.status = "Rename cancelled".into();
        }
    }

    /// Duplicate the current node.
    pub fn duplicate_node(&mut self) {
        if self.editing || self.renaming || self.confirming_delete || self.inserting {
            return;
        }
        let cursor_path = self.cursor_path();
        if cursor_path.is_empty() {
            self.status = "Cannot duplicate root".into();
            return;
        }
        let value = match self.tree.get(&cursor_path) {
            Some(v) => v.clone(),
            None => {
                self.status = "Cannot duplicate: path not found".into();
                return;
            }
        };
        let parent_path: TreePath = cursor_path[..cursor_path.len() - 1].to_vec();
        let last = cursor_path.last().unwrap();

        let snapshot = self.tree.clone();
        let result = match last {
            PathSegment::Key(k) => {
                let new_key = format!("{}_copy", k);
                let mut p = parent_path.clone();
                p.push(PathSegment::Key(new_key.clone()));
                self.tree.insert(&p, value).map(|_| new_key)
            }
            PathSegment::Index(i) => {
                let mut p = parent_path.clone();
                p.push(PathSegment::Index(*i + 1));
                self.tree.insert(&p, value).map(|_| format!("[{}]", i + 1))
            }
        };
        match result {
            Ok(label) => {
                // Only record history on success (see commit_rename).
                self.history.record(snapshot);
                self.modified = true;
                self.status = format!("Duplicated as {} — Ctrl+Z undo", label);
            }
            Err(e) => {
                self.status = format!("Duplicate error: {}", e);
            }
        }
    }

    /// Cut the current node (copy to clipboard and delete).
    pub fn cut_node(&mut self) {
        if self.editing || self.renaming || self.confirming_delete || self.inserting {
            return;
        }
        let cursor_path = self.cursor_path();
        if cursor_path.is_empty() {
            self.status = "Cannot cut root".into();
            return;
        }
        let value = match self.tree.get(&cursor_path) {
            Some(v) => v.clone(),
            None => {
                self.status = "Cannot cut: path not found".into();
                return;
            }
        };
        self.history.record(self.tree.clone());
        match self.tree.delete(&cursor_path) {
            Ok(_) => {
                self.clipboard = Some((cursor_path, value));
                self.modified = true;
                self.status = "Cut — navigate to target and press Ctrl+P to paste".into();
            }
            Err(e) => {
                self.status = format!("Cut error: {}", e);
            }
        }
    }

    /// Paste a previously cut node at the current cursor position.
    ///
    /// `cut_node` already removes the value from the tree and stores it in
    /// the clipboard, so pasting inserts that held value directly — it must
    /// not try to move/delete from the (now stale) original path again.
    pub fn paste_node(&mut self) {
        if self.editing || self.renaming || self.confirming_delete || self.inserting {
            return;
        }
        let (from_path, value) = match &self.clipboard {
            Some(pair) => pair.clone(),
            None => {
                self.status = "Nothing to paste — use Ctrl+X to cut first".into();
                return;
            }
        };
        let cursor_path = self.cursor_path();
        if cursor_path.is_empty() {
            self.status = "Cannot paste at root".into();
            return;
        }
        let parent_path: TreePath = cursor_path[..cursor_path.len() - 1].to_vec();
        let parent = self.tree.get(&parent_path).cloned();

        self.history.record(self.tree.clone());
        let result = match parent {
            Some(Value::Object(map)) => {
                let mut key = match from_path.last() {
                    Some(PathSegment::Key(k)) => k.clone(),
                    _ => "pasted".to_string(),
                };
                while map.contains_key(&key) {
                    key = format!("{}_copy", key);
                }
                let mut p = parent_path.clone();
                p.push(PathSegment::Key(key.clone()));
                self.tree.insert(&p, value).map(|_| key)
            }
            Some(Value::Array(ref arr)) => {
                let idx = arr.len();
                let mut p = parent_path.clone();
                p.push(PathSegment::Index(idx));
                self.tree.insert(&p, value).map(|_| format!("[{}]", idx))
            }
            _ => Err(confui::core::TreeError::PathNotFound),
        };
        match result {
            Ok(label) => {
                self.clipboard = None;
                self.modified = true;
                self.status = format!("Pasted as {} — Ctrl+Z undo", label);
            }
            Err(e) => {
                self.status = format!("Paste error: {}", e);
            }
        }
    }

    // ── theme cycling ────────────────────────────────────────────────

    /// Cycle to the next theme preset.
    pub fn cycle_theme(&mut self) {
        self.theme_index = (self.theme_index + 1) % theme::PRESETS.len();
        self.theme = theme::PRESETS[self.theme_index];
        self.status = format!("Theme: {} — Ctrl+T to cycle", self.theme.name);
    }

    // ── search ─────────────────────────────────────────────────────

    /// Start search mode: open the search query input.
    pub fn start_search(&mut self) {
        if self.editing || self.renaming || self.confirming_delete || self.inserting {
            return;
        }
        self.searching = true;
        self.search_query.clear();
        self.search_results.clear();
        self.search_index = 0;
        self.status = "/ — type query, Enter to search, n/N next/prev, Esc cancel".into();
    }

    /// Update the search query and re-run the search.
    fn run_search(&mut self) {
        if self.search_query.is_empty() {
            self.search_results.clear();
            self.search_index = 0;
            return;
        }
        let query = self.search_query.to_lowercase();
        let mut results = Vec::new();
        search_tree(&self.tree, &TreePath::new(), &query, &mut results);
        self.search_results = results;
        self.search_index = 0;
    }

    /// Commit the search and jump to the first match.
    pub fn commit_search(&mut self) {
        if !self.searching {
            return;
        }
        self.searching = false;
        self.run_search();
        if self.search_results.is_empty() {
            self.status = "No matches found".into();
        } else {
            self.search_index = 0;
            self.jump_to_search_match();
            let total = self.search_results.len();
            self.status = format!(
                "Search: {} match{} — n next, N prev, / new search",
                total,
                if total == 1 { "" } else { "es" }
            );
        }
    }

    /// Cancel search mode.
    pub fn cancel_search(&mut self) {
        if self.searching {
            self.searching = false;
            self.search_query.clear();
            self.search_results.clear();
            self.search_index = 0;
            self.status = "Search cancelled".into();
        }
    }

    /// Jump to the next search match.
    pub fn search_next(&mut self) {
        if self.search_results.is_empty() {
            self.status = "No active search — press / to search".into();
            return;
        }
        self.search_index = (self.search_index + 1) % self.search_results.len();
        self.jump_to_search_match();
        self.status = format!(
            "Match {}/{} — n next, N prev",
            self.search_index + 1,
            self.search_results.len()
        );
    }

    /// Jump to the previous search match.
    pub fn search_prev(&mut self) {
        if self.search_results.is_empty() {
            self.status = "No active search — press / to search".into();
            return;
        }
        self.search_index = if self.search_index == 0 {
            self.search_results.len() - 1
        } else {
            self.search_index - 1
        };
        self.jump_to_search_match();
        self.status = format!(
            "Match {}/{} — n next, N prev",
            self.search_index + 1,
            self.search_results.len()
        );
    }

    /// Navigate to the current search match (expand its path and set cursor).
    fn jump_to_search_match(&mut self) {
        let target_path = match self.search_results.get(self.search_index) {
            Some(p) => p.clone(),
            None => return,
        };

        // Expand all ancestors so the target is visible
        let mut ancestors = Vec::new();
        for i in 0..target_path.len() {
            let ancestor: TreePath = target_path[..i].to_vec();
            if !self.expanded.contains(&ancestor) {
                ancestors.push(ancestor);
            }
        }
        self.expanded.extend(ancestors);
        if !self.expanded.contains(&target_path)
            && !target_path.is_empty()
            && let Some(val) = self.tree.get(&target_path)
            && !val.is_leaf()
        {
            self.expanded.push(target_path.clone());
        }

        // Find the target in visible lines and set cursor
        let lines = self.compute_visible_lines();
        if let Some(idx) = lines.iter().position(|l| l.path == target_path) {
            self.cursor_index = idx;
            self.ensure_cursor_visible(&lines);
        }
    }

    // ── undo / redo ────────────────────────────────────────────────

    /// Undo the last mutation.
    pub fn undo(&mut self) {
        if self.editing || self.searching {
            return;
        }
        let current = std::mem::replace(&mut self.tree, Value::Null);
        match self.history.undo(current.clone()) {
            Some(restored) => {
                self.tree = restored;
                self.modified = true;
                self.status = "Undone — Ctrl+Y redo  Ctrl+S save".into();
            }
            None => {
                self.tree = current;
                self.status = "Nothing to undo".into();
            }
        }
    }

    /// Redo a previously undone mutation.
    pub fn redo(&mut self) {
        if self.editing {
            return;
        }
        let current = std::mem::replace(&mut self.tree, Value::Null);
        match self.history.redo(current.clone()) {
            Some(restored) => {
                self.tree = restored;
                self.modified = true;
                self.status = "Redone — Ctrl+Z undo  Ctrl+S save".into();
            }
            None => {
                self.tree = current;
                self.status = "Nothing to redo".into();
            }
        }
    }

    // ── save ──────────────────────────────────────────────────────

    /// Save the current tree to the file with atomic write and .bak backup.
    ///
    /// Writes to a temporary file in the same directory, fsyncs, then
    /// atomically renames over the original. Creates a `.bak` backup first.
    pub fn save(&mut self) -> Result<()> {
        use std::io::Write;

        let content =
            parser::serialize_for_edit(&self.file_path, &self.tree, &mut self.edit_state)?;

        // Create backup (.bak)
        let backup_path = {
            let mut p = self.file_path.as_os_str().to_os_string();
            p.push(".bak");
            std::path::PathBuf::from(p)
        };
        std::fs::copy(&self.file_path, &backup_path)?;

        // Write to temp file
        let temp_path = {
            let file_name = self
                .file_path
                .file_name()
                .map(|s| s.to_string_lossy())
                .unwrap_or_default();
            let parent = self
                .file_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            parent.join(format!(".{}.tmp", file_name))
        };
        {
            let mut file = std::fs::File::create(&temp_path)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
        }

        // Atomically rename over original
        std::fs::rename(&temp_path, &self.file_path)?;

        self.modified = false;
        self.status = "Saved".into();
        Ok(())
    }
}

/// Format a float value for the edit buffer (strip trailing zeros).
fn format_float_for_edit(f: f64) -> String {
    if f.fract() == 0.0 && f.is_finite() && f.abs() < 1_000_000_000_000.0 {
        format!("{}", f as i64)
    } else {
        format!("{}", f)
    }
}

/// Recursively collect all paths in the tree.
fn collect_all_paths(value: &Value, parent: &TreePath, paths: &mut Vec<TreePath>) {
    paths.push(parent.clone());
    match value {
        Value::Object(map) => {
            for key in map.keys() {
                let mut p = parent.clone();
                p.push(PathSegment::Key(key.clone()));
                if let Some(child) = map.get(key) {
                    collect_all_paths(child, &p, paths);
                }
            }
        }
        Value::Array(arr) => {
            for (i, child) in arr.iter().enumerate() {
                let mut p = parent.clone();
                p.push(PathSegment::Index(i));
                collect_all_paths(child, &p, paths);
            }
        }
        _ => {}
    }
}

/// Recursively search the tree for keys/values matching the given lowercase query.
fn search_tree(value: &Value, parent: &TreePath, query: &str, results: &mut Vec<TreePath>) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter() {
                let mut p = parent.clone();
                p.push(PathSegment::Key(key.clone()));
                if key.to_lowercase().contains(query) {
                    results.push(p.clone());
                }
                if let Some(text) = value_to_search_text(val)
                    && text.to_lowercase().contains(query)
                    && !results.contains(&p)
                {
                    results.push(p.clone());
                }
                search_tree(val, &p, query, results);
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                let mut p = parent.clone();
                p.push(PathSegment::Index(i));
                if let Some(text) = value_to_search_text(val)
                    && text.to_lowercase().contains(query)
                {
                    results.push(p.clone());
                }
                search_tree(val, &p, query, results);
            }
        }
        _ => {}
    }
}

/// Get a text representation of a value for search purposes.
fn value_to_search_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Int(i) => Some(i.to_string()),
        Value::Float(f) => Some(f.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some("null".into()),
        Value::Object(_) | Value::Array(_) => None,
    }
}

/// Format name for display.
fn format_name(format: Format) -> &'static str {
    match format {
        Format::Json => "JSON",
        Format::Toml => "TOML",
        Format::Yaml => "YAML",
    }
}

/// Run the main event loop.
/// Run the main event loop.
pub fn run(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    file_path: &Path,
) -> Result<()> {
    let mut app = App::new(file_path)?;

    loop {
        terminal.draw(|frame| {
            ui::render(frame, &mut app);
        })?;

        if app.should_quit {
            break;
        }

        if let Event::Key(key) = event::read()? {
            handle_key_event(&mut app, key);
        }
    }

    Ok(())
}

/// Handle keyboard events.
fn handle_key_event(app: &mut App, key: KeyEvent) {
    if app.editing {
        // ── edit mode key handling ────────────────────────────────
        match key.code {
            KeyCode::Enter => {
                app.commit_edit();
            }
            KeyCode::Esc => {
                app.cancel_edit();
            }
            KeyCode::Left => {
                app.edit_cursor_left();
            }
            KeyCode::Right => {
                app.edit_cursor_right();
            }
            KeyCode::Home => {
                app.edit_cursor_home();
            }
            KeyCode::End => {
                app.edit_cursor_end();
            }
            KeyCode::Backspace => {
                app.edit_delete_before_cursor();
            }
            KeyCode::Delete => {
                app.edit_delete_at_cursor();
            }
            KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                // Ctrl+C does nothing in edit mode (don't quit)
            }
            KeyCode::Char('s') if key.modifiers == KeyModifiers::CONTROL => {
                // Commit edit first, then save
                app.commit_edit();
                if let Err(e) = app.save() {
                    app.status = format!("Save error: {e}");
                }
            }
            KeyCode::Char(c) => {
                app.edit_insert_char(c);
            }
            _ => {}
        }
    } else if app.renaming {
        // ── rename mode key handling ──────────────────────────────
        match key.code {
            KeyCode::Enter => {
                app.commit_rename();
            }
            KeyCode::Esc => {
                app.cancel_rename();
            }
            KeyCode::Backspace => {
                if !app.rename_buffer.is_empty() {
                    app.rename_buffer.pop();
                }
            }
            KeyCode::Char(c) => {
                app.rename_buffer.push(c);
            }
            _ => {}
        }
    } else if app.confirming_delete {
        // ── confirm delete key handling ───────────────────────────
        match key.code {
            KeyCode::Char('d') | KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                app.confirm_delete();
            }
            _ => {
                app.cancel_delete();
            }
        }
    } else if app.inserting {
        // ── insert type picker key handling ───────────────────────
        match key.code {
            KeyCode::Esc => app.cancel_insert(),
            KeyCode::Char('s') => app.commit_insert(Value::string("")),
            KeyCode::Char('n') => app.commit_insert(Value::int(0)),
            KeyCode::Char('f') => app.commit_insert(Value::float(0.0)),
            KeyCode::Char('b') => app.commit_insert(Value::bool(false)),
            KeyCode::Char('a') => app.commit_insert(Value::array()),
            KeyCode::Char('o') => app.commit_insert(Value::object()),
            _ => {}
        }
    } else if app.searching {
        // ── search mode key handling ─────────────────────────────
        match key.code {
            KeyCode::Enter => {
                app.commit_search();
            }
            KeyCode::Esc => {
                app.cancel_search();
            }
            KeyCode::Backspace => {
                if !app.search_query.is_empty() {
                    app.search_query.pop();
                    app.run_search();
                }
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
                app.run_search();
            }
            _ => {}
        }
    } else {
        // ── normal mode key handling ─────────────────────────────
        match key.code {
            KeyCode::Char('q') => {
                app.should_quit = true;
            }
            KeyCode::Char('Q') => {
                app.should_quit = true;
            }
            KeyCode::Up => {
                app.cursor_up();
            }
            KeyCode::Down => {
                app.cursor_down();
            }
            KeyCode::Left => {
                app.collapse();
            }
            KeyCode::Right => {
                app.expand();
            }
            KeyCode::Enter => {
                let path = app.cursor_path();
                let lines = app.compute_visible_lines();
                if let Some(line) = lines.get(app.cursor_index)
                    && !line.is_leaf
                {
                    // Toggle expand/collapse for containers
                    app.toggle_expand();
                } else if !path.is_empty() {
                    // Start editing leaf values
                    app.start_edit();
                }
            }
            KeyCode::Home => {
                app.cursor_home();
            }
            KeyCode::End => {
                app.cursor_end();
            }
            KeyCode::PageUp => {
                app.cursor_page_up();
            }
            KeyCode::PageDown => {
                app.cursor_page_down();
            }
            KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
                app.expand_all();
            }
            KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
                app.collapse_all();
            }
            KeyCode::Char('s') if key.modifiers == KeyModifiers::CONTROL => {
                if let Err(e) = app.save() {
                    app.status = format!("Save error: {e}");
                }
            }
            KeyCode::Char('z') if key.modifiers == KeyModifiers::CONTROL => {
                app.undo();
            }
            KeyCode::Char('y') if key.modifiers == KeyModifiers::CONTROL => {
                app.redo();
            }
            // ── search keybindings ───────────────────────────────
            KeyCode::Char('/') => {
                app.start_search();
            }
            KeyCode::Char('n') => {
                app.search_next();
            }
            KeyCode::Char('N') => {
                app.search_prev();
            }
            // ── structure editing keybindings ─────────────────────
            KeyCode::Char('i') => {
                app.insert_node();
            }
            KeyCode::Char('d') => {
                app.delete_node();
            }
            KeyCode::Char('R') => {
                app.start_rename();
            }
            KeyCode::Char('D') => {
                app.duplicate_node();
            }
            KeyCode::Char('x') if key.modifiers == KeyModifiers::CONTROL => {
                app.cut_node();
            }
            KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
                app.paste_node();
            }
            KeyCode::Char('t') if key.modifiers == KeyModifiers::CONTROL => {
                app.cycle_theme();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::History;
    use ratatui::backend::TestBackend;

    fn test_tree() -> Value {
        use indexmap::IndexMap;
        Value::Object(IndexMap::from([
            (
                "server".into(),
                Value::Object(IndexMap::from([
                    ("host".into(), Value::string("0.0.0.0")),
                    ("port".into(), Value::int(8080)),
                ])),
            ),
            (
                "items".into(),
                Value::Array(vec![Value::string("a"), Value::int(42)]),
            ),
            ("count".into(), Value::int(100)),
        ]))
    }

    fn make_app(tree: Value) -> App {
        let mut expanded = Vec::new();
        expanded.push(TreePath::new());
        if let Value::Object(map) = &tree {
            for key in map.keys() {
                if let Some(val) = tree.get(&vec![PathSegment::Key(key.clone())])
                    && !val.is_leaf()
                {
                    expanded.push(vec![PathSegment::Key(key.clone())]);
                }
            }
        }
        let history = History::new(tree.clone());
        App {
            tree,
            file_path: std::path::PathBuf::from("test.toml"),
            format: Format::Toml,
            cursor_index: 0,
            expanded,
            scroll_offset: 0,
            property_scroll: 0,
            status: "test".into(),
            modified: false,
            should_quit: false,
            history,
            editing: false,
            edit_path: TreePath::new(),
            edit_buffer: String::new(),
            edit_cursor: 0,
            confirming_delete: false,
            confirm_delete_path: TreePath::new(),
            inserting: false,
            insert_container_path: TreePath::new(),
            renaming: false,
            rename_path: TreePath::new(),
            rename_buffer: String::new(),
            clipboard: None,
            searching: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_index: 0,
            theme_index: 0,
            theme: &theme::DARK,
            plugins: PluginRegistry::new(),
            active_plugin: None,
            edit_state: parser::EditState::Other,
        }
    }

    #[test]
    fn app_new_parses_file() {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.toml");
        let app = App::new(&path).unwrap();
        assert_eq!(app.format, Format::Toml);
        assert!(app.tree.get(&vec!["server".into()]).is_some());
        assert!(!app.should_quit);
    }

    #[test]
    fn app_navigation() {
        let mut app = make_app(test_tree());
        assert_eq!(app.cursor_index, 0);

        // visible lines: the root "server >", then server's children, "items >", then items' children, then "count: 100"
        app.cursor_down();
        assert_eq!(app.cursor_index, 1);

        app.cursor_down();
        assert_eq!(app.cursor_index, 2);

        app.cursor_up();
        assert_eq!(app.cursor_index, 1);

        app.cursor_home();
        assert_eq!(app.cursor_index, 0);

        app.cursor_end();
        // Should be at the last line
        let lines = app.compute_visible_lines();
        assert_eq!(app.cursor_index, lines.len() - 1);
    }

    #[test]
    fn app_expand_collapse() {
        let mut app = make_app(test_tree());
        let initial_count = app.compute_visible_lines().len();

        // Collapse "server"
        // First, find the index of "server" in visible lines
        let lines = app.compute_visible_lines();
        let server_idx = lines.iter().position(|l| l.key == "server").unwrap();
        app.cursor_index = server_idx;
        app.collapse();
        let after_collapse = app.compute_visible_lines().len();
        assert!(after_collapse < initial_count);

        // Re-expand
        app.expand();
        let after_expand = app.compute_visible_lines().len();
        assert_eq!(after_expand, initial_count);
    }

    #[test]
    fn app_expand_all_collapse_all() {
        let mut app = make_app(test_tree());
        // Start with some expanded state, collapse all
        app.collapse_all();
        let lines = app.compute_visible_lines();
        // Only root should be visible
        // root + 3 top-level keys (since root is expanded)
        // Actually, collapse_all keeps root expanded
        assert_eq!(lines.len(), 4); // root, server, items, count

        // Expand all
        app.expand_all();
        let lines_after = app.compute_visible_lines();
        assert!(lines_after.len() > 4);
    }

    #[test]
    fn test_quit() {
        let mut app = make_app(test_tree());
        assert!(!app.should_quit);
        handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        );
        assert!(app.should_quit);
    }

    #[test]
    fn render_test() {
        let mut app = make_app(test_tree());
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                ui::render(frame, &mut app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        // Just verify the buffer has content (not all spaces)
        let has_content = buffer.content.iter().any(|cell| cell.symbol() != " ");
        assert!(has_content, "Buffer should contain rendered content");
    }

    // ── editing tests ─────────────────────────────────────────────

    #[test]
    fn edit_string_value_start_commit() {
        let mut app = make_app(test_tree());
        // Navigate to server > host (value: "0.0.0.0")
        let lines = app.compute_visible_lines();
        // Find the host line
        let host_idx = lines.iter().position(|l| l.key == "host").unwrap();
        app.cursor_index = host_idx;

        app.start_edit();
        assert!(app.editing);
        assert_eq!(app.edit_buffer, "0.0.0.0");
        assert_eq!(app.edit_cursor, 7);
        assert_eq!(app.edit_path, vec!["server".into(), "host".into()]);

        // Modify the buffer
        app.edit_insert_char('9');
        assert_eq!(app.edit_buffer, "0.0.0.09");
        assert_eq!(app.edit_cursor, 8);

        // Commit
        app.commit_edit();
        assert!(!app.editing);
        assert!(app.modified);
        assert_eq!(
            app.tree.get(&vec!["server".into(), "host".into()]),
            Some(&Value::string("0.0.0.09"))
        );
    }

    #[test]
    fn edit_string_cancel_restores() {
        let mut app = make_app(test_tree());
        let lines = app.compute_visible_lines();
        let host_idx = lines.iter().position(|l| l.key == "host").unwrap();
        app.cursor_index = host_idx;

        app.start_edit();
        assert!(app.editing);
        app.edit_insert_char('X');
        assert_eq!(app.edit_buffer, "0.0.0.0X");

        app.cancel_edit();
        assert!(!app.editing);
        assert!(!app.modified);
        // Original value preserved
        assert_eq!(
            app.tree.get(&vec!["server".into(), "host".into()]),
            Some(&Value::string("0.0.0.0"))
        );
    }

    #[test]
    fn edit_int_value_validates() {
        let mut app = make_app(test_tree());
        // Navigate to server > port (value: 8080)
        let lines = app.compute_visible_lines();
        let port_idx = lines.iter().position(|l| l.key == "port").unwrap();
        app.cursor_index = port_idx;

        app.start_edit();
        assert!(app.editing);
        assert_eq!(app.edit_buffer, "8080");

        // Clear and type invalid
        app.edit_buffer.clear();
        app.edit_cursor = 0;
        app.edit_insert_char('a');
        app.commit_edit();
        // Should still be in edit mode with error
        assert!(app.editing);
        assert!(app.status.contains("Invalid integer"));

        // Fix it
        app.edit_buffer.clear();
        app.edit_cursor = 0;
        app.edit_insert_char('9');
        app.edit_insert_char('0');
        app.commit_edit();
        assert!(!app.editing);
        assert_eq!(
            app.tree.get(&vec!["server".into(), "port".into()]),
            Some(&Value::int(90))
        );
    }

    #[test]
    fn edit_bool_toggles_immediately() {
        let mut app = make_app(test_tree());
        // We need to add a bool to the test tree; let's modify the tree temporarily
        use indexmap::IndexMap;
        app.tree = Value::Object(IndexMap::from([
            ("enabled".into(), Value::bool(true)),
            ("count".into(), Value::int(42)),
        ]));
        app.expanded.clear();
        app.expanded.push(TreePath::new());

        let lines = app.compute_visible_lines();
        let enabled_idx = lines.iter().position(|l| l.key == "enabled").unwrap();
        app.cursor_index = enabled_idx;

        assert_eq!(
            app.tree.get(&vec!["enabled".into()]),
            Some(&Value::bool(true))
        );

        app.start_edit();
        // Bool toggles immediately, no edit mode
        assert!(!app.editing);
        assert!(app.modified);
        assert_eq!(
            app.tree.get(&vec!["enabled".into()]),
            Some(&Value::bool(false))
        );
    }

    #[test]
    fn edit_cursor_movement() {
        let mut app = make_app(test_tree());
        let lines = app.compute_visible_lines();
        let host_idx = lines.iter().position(|l| l.key == "host").unwrap();
        app.cursor_index = host_idx;

        app.start_edit();
        assert_eq!(app.edit_cursor, 7); // end of "0.0.0.0"

        // Move left a few times
        app.edit_cursor_left();
        assert_eq!(app.edit_cursor, 6);
        app.edit_cursor_left();
        assert_eq!(app.edit_cursor, 5);

        // Move right
        app.edit_cursor_right();
        assert_eq!(app.edit_cursor, 6);

        // Home
        app.edit_cursor_home();
        assert_eq!(app.edit_cursor, 0);

        // End
        app.edit_cursor_end();
        assert_eq!(app.edit_cursor, 7);

        // Insert at home
        app.edit_cursor_home();
        app.edit_insert_char('x');
        assert_eq!(app.edit_buffer, "x0.0.0.0");
        assert_eq!(app.edit_cursor, 1);
    }

    #[test]
    fn edit_delete_backspace() {
        let mut app = make_app(test_tree());
        let lines = app.compute_visible_lines();
        let host_idx = lines.iter().position(|l| l.key == "host").unwrap();
        app.cursor_index = host_idx;

        app.start_edit();
        assert_eq!(app.edit_buffer, "0.0.0.0");

        // Backspace at end removes last char '0'
        app.edit_delete_before_cursor();
        assert_eq!(app.edit_buffer, "0.0.0.");
        assert_eq!(app.edit_cursor, 6);

        // Move left by one (to position 5, which is '.')
        app.edit_cursor_left();
        assert_eq!(app.edit_cursor, 5);

        // Delete at cursor removes the '.' at position 5
        app.edit_delete_at_cursor();
        assert_eq!(app.edit_buffer, "0.0.0");
        assert_eq!(app.edit_cursor, 5);
    }

    #[test]
    fn save_creates_file() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("confui_test_save");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.toml");
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"count = 100\n").unwrap();
        file.sync_all().unwrap();

        let mut app = App::new(&file_path).unwrap();
        // Modify
        app.tree
            .set(&vec!["count".into()], Value::int(200))
            .unwrap();
        app.modified = true;

        // Save
        app.save().unwrap();
        assert!(!app.modified);

        // Verify content
        let saved = std::fs::read_to_string(&file_path).unwrap();
        assert!(saved.contains("200"));

        // Verify .bak exists
        let bak_path = {
            let mut p = file_path.as_os_str().to_os_string();
            p.push(".bak");
            std::path::PathBuf::from(p)
        };
        assert!(bak_path.exists(), "Backup file should exist");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_preserves_toml_comments_on_untouched_keys() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("confui_test_save_comments");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.toml");
        let original = "\
# top-level comment
count = 100  # inline comment

[server]
# server comment
host = \"0.0.0.0\"
";
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(original.as_bytes()).unwrap();
        file.sync_all().unwrap();

        let mut app = App::new(&file_path).unwrap();
        // Only touch "host"; "count" and its comments should be untouched.
        app.tree
            .set(
                &vec!["server".into(), "host".into()],
                Value::string("1.2.3.4"),
            )
            .unwrap();
        app.modified = true;
        app.save().unwrap();

        let saved = std::fs::read_to_string(&file_path).unwrap();
        assert!(
            saved.contains("# top-level comment"),
            "top-level comment lost:\n{saved}"
        );
        assert!(
            saved.contains("count = 100  # inline comment"),
            "inline comment on unchanged key lost:\n{saved}"
        );
        assert!(
            saved.contains("# server comment"),
            "table comment lost:\n{saved}"
        );
        assert!(saved.contains("1.2.3.4"), "edited value missing:\n{saved}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_preserves_untouched_toml_datetime_type() {
        // TOML datetimes are represented as `Value::String` internally
        // (there's no dedicated core::Value variant for them). A full
        // rebuild-from-scratch save would always re-emit them as quoted
        // strings, silently changing their type. As long as the datetime
        // field itself isn't edited, the untouched-key fast path in
        // `serialize_toml_into` must leave the original item (and its real
        // TOML datetime type) alone.
        use std::io::Write;
        let dir = std::env::temp_dir().join("confui_test_save_datetime");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.toml");
        let original = "created = 2024-01-01T00:00:00Z\ncount = 1\n";
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(original.as_bytes()).unwrap();
        file.sync_all().unwrap();

        let mut app = App::new(&file_path).unwrap();
        // Edit an unrelated key; leave "created" untouched.
        app.tree.set(&vec!["count".into()], Value::int(2)).unwrap();
        app.modified = true;
        app.save().unwrap();

        let saved = std::fs::read_to_string(&file_path).unwrap();
        assert!(
            saved.contains("created = 2024-01-01T00:00:00Z"),
            "untouched datetime should keep its real TOML type, not become a quoted string:\n{saved}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn modified_indicator_updates() {
        let mut app = make_app(test_tree());
        assert!(!app.modified);

        let lines = app.compute_visible_lines();
        let host_idx = lines.iter().position(|l| l.key == "host").unwrap();
        app.cursor_index = host_idx;
        app.start_edit();
        app.commit_edit();

        assert!(app.modified);
    }

    #[test]
    fn enter_on_container_toggles_expand() {
        let mut app = make_app(test_tree());
        // "server" is at index 1 (after root)
        app.cursor_index = 1;
        let lines_before = app.compute_visible_lines().len();

        // Enter on server should collapse it
        handle_key_event(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let lines_after = app.compute_visible_lines().len();
        assert!(lines_after < lines_before);
    }

    #[test]
    fn enter_on_leaf_enters_edit_mode() {
        let mut app = make_app(test_tree());
        let lines = app.compute_visible_lines();
        let host_idx = lines.iter().position(|l| l.key == "host").unwrap();
        app.cursor_index = host_idx;

        handle_key_event(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(app.editing);
        assert_eq!(app.edit_buffer, "0.0.0.0");
    }

    #[test]
    fn esc_cancels_edit() {
        let mut app = make_app(test_tree());
        let lines = app.compute_visible_lines();
        let host_idx = lines.iter().position(|l| l.key == "host").unwrap();
        app.cursor_index = host_idx;

        handle_key_event(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.editing);

        handle_key_event(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.editing);
        assert!(!app.modified);
    }

    #[test]
    fn insert_node_twice_does_not_collide() {
        let mut app = make_app(test_tree());
        // Land cursor on "count" (a root-level leaf) so inserts target the root object.
        let lines = app.compute_visible_lines();
        let count_idx = lines.iter().position(|l| l.key == "count").unwrap();
        app.cursor_index = count_idx;

        app.insert_node();
        assert!(app.inserting);
        app.commit_insert(Value::string(""));
        assert!(
            app.status.contains("Inserted string 'new_key'"),
            "{}",
            app.status
        );
        assert_eq!(
            app.tree.get(&vec!["new_key".into()]),
            Some(&Value::string(""))
        );

        // Second insert at the same level must not collide with the first.
        app.insert_node();
        app.commit_insert(Value::string(""));
        assert!(
            app.status.contains("Inserted string 'new_key_2'"),
            "second insert should pick a non-colliding key, got: {}",
            app.status
        );
        assert!(app.tree.get(&vec!["new_key".into()]).is_some());
        assert!(app.tree.get(&vec!["new_key_2".into()]).is_some());
    }

    #[test]
    fn insert_node_type_picker_can_be_cancelled() {
        let mut app = make_app(test_tree());
        let lines = app.compute_visible_lines();
        let count_idx = lines.iter().position(|l| l.key == "count").unwrap();
        app.cursor_index = count_idx;

        app.insert_node();
        assert!(app.inserting);
        app.cancel_insert();
        assert!(!app.inserting);
        assert!(app.tree.get(&vec!["new_key".into()]).is_none());
        assert!(!app.modified);
    }

    #[test]
    fn insert_node_can_create_object_and_array() {
        let mut app = make_app(test_tree());
        let lines = app.compute_visible_lines();
        let count_idx = lines.iter().position(|l| l.key == "count").unwrap();
        app.cursor_index = count_idx;

        app.insert_node();
        app.commit_insert(Value::object());
        assert_eq!(
            app.tree.get(&vec!["new_key".into()]),
            Some(&Value::object())
        );

        app.insert_node();
        app.commit_insert(Value::array());
        assert_eq!(
            app.tree.get(&vec!["new_key_2".into()]),
            Some(&Value::array())
        );
    }

    #[test]
    fn insert_node_on_container_row_inserts_as_child() {
        let mut app = make_app(test_tree());
        // Cursor directly on "server" (an object row), not one of its children.
        let lines = app.compute_visible_lines();
        let server_idx = lines.iter().position(|l| l.key == "server").unwrap();
        app.cursor_index = server_idx;

        app.insert_node();
        app.commit_insert(Value::int(0));
        assert_eq!(
            app.tree.get(&vec!["server".into(), "new_key".into()]),
            Some(&Value::int(0)),
            "inserting while the cursor is on a container should add a child, not a root-level sibling"
        );
        assert!(app.tree.get(&vec!["new_key".into()]).is_none());
    }

    #[test]
    fn insert_node_into_empty_object() {
        use indexmap::IndexMap;
        let mut app = make_app(Value::Object(IndexMap::from([(
            "empty".to_string(),
            Value::object(),
        )])));
        let lines = app.compute_visible_lines();
        let empty_idx = lines.iter().position(|l| l.key == "empty").unwrap();
        app.cursor_index = empty_idx;

        app.insert_node();
        assert!(
            app.inserting,
            "cursor on an empty object's own row must still allow inserting into it"
        );
        app.commit_insert(Value::bool(true));
        assert_eq!(
            app.tree.get(&vec!["empty".into(), "new_key".into()]),
            Some(&Value::bool(true))
        );
    }

    #[test]
    fn insert_node_at_root() {
        let mut app = make_app(test_tree());
        // Cursor on the root line itself (index 0).
        app.cursor_index = 0;
        assert!(app.cursor_path().is_empty());

        app.insert_node();
        assert!(app.inserting);
        app.commit_insert(Value::float(1.5));
        assert_eq!(
            app.tree.get(&vec!["new_key".into()]),
            Some(&Value::float(1.5))
        );
    }

    #[test]
    fn cut_then_paste_reinserts_value() {
        let mut app = make_app(test_tree());
        // Cut "count" (top-level leaf)
        let lines = app.compute_visible_lines();
        let count_idx = lines.iter().position(|l| l.key == "count").unwrap();
        app.cursor_index = count_idx;
        app.cut_node();
        assert!(app.tree.get(&vec!["count".into()]).is_none());
        assert!(app.clipboard.is_some());

        // Paste it as a sibling of "host" (inside "server")
        let lines = app.compute_visible_lines();
        let host_idx = lines.iter().position(|l| l.key == "host").unwrap();
        app.cursor_index = host_idx;
        app.paste_node();

        assert!(app.clipboard.is_none(), "paste error: {}", app.status);
        assert_eq!(
            app.tree.get(&vec!["server".into(), "count".into()]),
            Some(&Value::int(100)),
            "pasting should reinsert the cut value under the target's parent, preserving its key"
        );
    }
}
