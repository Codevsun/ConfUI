//! Undo/redo snapshot-based history stack.
//!
//! Stores snapshots of the entire [`Value`] tree before each mutation.
//! [`History::undo`] and [`History::redo`] restore previous or next states.

use confui::core::Value;

/// A bounded, snapshot-based undo/redo history.
///
/// # Approach
/// Before every mutation, call [`History::record`] with the current tree.
/// [`History::undo`] restores the previous state and pushes the current
/// one onto the redo stack. [`History::redo`] does the reverse.
///
/// When a new mutation is recorded after an undo, the redo stack is
/// cleared (the branch is lost).
#[derive(Debug)]
#[allow(dead_code)]
pub struct History {
    /// Past states (most recent at the end).
    undo_stack: Vec<Value>,
    /// Future states (most recent at the end).
    redo_stack: Vec<Value>,
    /// Maximum number of undo steps.
    max_undo: usize,
}

#[allow(dead_code)]
impl History {
    /// Create a new history seeded with the initial tree state.
    pub fn new(initial: Value) -> Self {
        Self {
            undo_stack: vec![initial],
            redo_stack: Vec::new(),
            max_undo: 100,
        }
    }

    /// Record a snapshot **before** a mutation is applied.
    ///
    /// Call this right before modifying the tree. Clears the redo stack
    /// because we are creating a new history branch.
    pub fn record(&mut self, state: Value) {
        self.undo_stack.push(state);
        self.redo_stack.clear();
        // Keep the stack bounded.
        while self.undo_stack.len() > self.max_undo {
            self.undo_stack.remove(0);
        }
    }

    /// Undo: restore the previous state.
    ///
    /// Pass the **current** tree state. Returns `Some(previous_state)` or
    /// `None` if there is nothing to undo.
    pub fn undo(&mut self, current: Value) -> Option<Value> {
        if self.undo_stack.len() <= 1 {
            return None;
        }
        self.redo_stack.push(current);
        self.undo_stack.pop()
    }

    /// Redo: restore the next state after an undo.
    ///
    /// Pass the **current** tree state. Returns `Some(next_state)` or
    /// `None` if there is nothing to redo.
    pub fn redo(&mut self, current: Value) -> Option<Value> {
        let next = self.redo_stack.pop()?;
        self.undo_stack.push(current);
        Some(next)
    }

    /// Returns `true` if undo is available (more than one state stored).
    pub fn can_undo(&self) -> bool {
        self.undo_stack.len() > 1
    }

    /// Returns `true` if redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear the entire history and reset with a new initial state.
    pub fn reset(&mut self, initial: Value) {
        self.undo_stack = vec![initial];
        self.redo_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use confui::core::Value;
    use confui::path;

    fn make_tree(v: i64) -> Value {
        Value::Object(vec![("count".into(), Value::Int(v))].into_iter().collect())
    }

    #[test]
    fn history_initial_state() {
        let h = History::new(make_tree(0));
        assert!(!h.can_undo());
        assert!(!h.can_redo());
    }

    #[test]
    fn history_record_undo_redo() {
        let mut h = History::new(make_tree(0));

        // First mutation: record state before changing 0 -> 1
        h.record(make_tree(0));
        // Tree is now 1

        // Second mutation: record state before changing 1 -> 2
        h.record(make_tree(1));
        // Tree is now 2

        assert!(h.can_undo());

        // Undo: tree is 2, we want to go back to 1
        let restored = h.undo(make_tree(2)).unwrap();
        assert_eq!(restored.get(&path!["count"]), Some(&Value::Int(1)));
        assert!(h.can_redo());

        // Undo again: tree is 1, go back to 0
        let restored = h.undo(make_tree(1)).unwrap();
        assert_eq!(restored.get(&path!["count"]), Some(&Value::Int(0)));
        assert!(!h.can_undo());

        // Redo: tree is 0, go forward to 1
        let restored = h.redo(make_tree(0)).unwrap();
        assert_eq!(restored.get(&path!["count"]), Some(&Value::Int(1)));

        // Redo again: tree is 1, go forward to 2
        let restored = h.redo(make_tree(1)).unwrap();
        assert_eq!(restored.get(&path!["count"]), Some(&Value::Int(2)));
        assert!(!h.can_redo());
    }

    #[test]
    fn history_undo_clears_redo_on_new_record() {
        let mut h = History::new(make_tree(0));
        h.record(make_tree(0)); // before 0 -> 1
        h.record(make_tree(1)); // before 1 -> 2

        // Undo: current is 2, we get back 1
        let restored = h.undo(make_tree(2)).unwrap();
        assert_eq!(restored.get(&path!["count"]), Some(&Value::Int(1)));
        assert!(h.can_redo());

        // New mutation: record before 1 -> 3 (clears redo)
        h.record(make_tree(1));
        assert!(!h.can_redo());

        // Undo goes back one step: from the hypothetical state 3 (recorded as 1), back to state 1
        let restored = h.undo(make_tree(1)).unwrap();
        assert_eq!(restored.get(&path!["count"]), Some(&Value::Int(1)));

        // Another undo goes back to the initial state 0
        let restored = h.undo(make_tree(1)).unwrap();
        assert_eq!(restored.get(&path!["count"]), Some(&Value::Int(0)));
    }

    #[test]
    fn history_undo_past_beginning() {
        let mut h = History::new(make_tree(0));
        assert!(h.undo(make_tree(0)).is_none());
    }

    #[test]
    fn history_redo_past_end() {
        let mut h = History::new(make_tree(0));
        assert!(h.redo(make_tree(0)).is_none());
    }

    #[test]
    fn history_max_undo_bounded() {
        let mut h = History::new(make_tree(0));
        // Add 101 records (beyond max_undo of 100)
        for i in 0..101 {
            h.record(make_tree(i));
        }
        // Should have at most 101 states (initial + 100 records)
        assert!(h.undo_stack.len() <= 101);
        // The oldest state (0) should have been evicted, so we can undo to 1
        let restored = h.undo(make_tree(101)).unwrap();
        assert_eq!(restored.get(&path!["count"]), Some(&Value::Int(100)));
    }

    #[test]
    fn history_reset() {
        let mut h = History::new(make_tree(0));
        h.record(make_tree(0));
        h.record(make_tree(1));
        h.reset(make_tree(99));
        assert!(!h.can_undo());
        assert!(!h.can_redo());
        let restored = h.undo(make_tree(99));
        assert!(restored.is_none());
    }
}
