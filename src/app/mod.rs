//! Application state and main event loop.

use std::path::Path;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;

use crate::history::History;
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
}

impl App {
    /// Create a new App by parsing the file at `file_path`.
    pub fn new(file_path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| color_eyre::eyre::eyre!("Cannot read {}: {}", file_path.display(), e))?;
        let format = parser::Format::detect(file_path, &content);
        let tree = parser::parse(file_path, &content)?;

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
                if self.tree.set(&cursor_path, new_val).is_ok() {
                    self.modified = true;
                    self.status = "Toggled bool — Ctrl+S to save".into();
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

    // ── undo / redo ────────────────────────────────────────────────

    /// Undo the last mutation.
    pub fn undo(&mut self) {
        if self.editing {
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

        let content = parser::serialize(&self.file_path, &self.tree)?;

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
}
