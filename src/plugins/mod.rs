//! Plugin API trait and registry.
//!
//! Plugins extend the editor with documentation, validation rules, and
//! default value templates. Ship one built-in example plugin for
//! Cargo.toml files.

pub mod cargo_toml;

use std::fmt::Debug;

use crate::validation::ValidationMessage;
use confui::core::{Path, Value};

/// A plugin that adds domain-specific knowledge about a config file format.
///
/// Plugins can:
/// - Provide inline documentation for config keys (shown in the property panel)
/// - Add custom validation rules on top of the generic validators
/// - Supply default values for common keys
#[allow(dead_code)]
pub trait Plugin: Debug {
    /// Human-readable name (e.g. "Cargo.toml").
    fn name(&self) -> &str;

    /// A short description of what this plugin covers.
    fn description(&self) -> &str;

    /// Return `true` if this plugin is applicable to the given file name.
    fn matches_file(&self, file_name: &str) -> bool;

    /// Documentation for the given path.
    ///
    /// Return `Some(markdown_text)` if this plugin has documentation for
    /// that key, or `None` otherwise.
    fn docs_for(&self, path: &Path) -> Option<String>;

    /// Custom validation for the given value at `path`.
    ///
    /// These are called **in addition to** the generic validators.
    fn validate(&self, path: &Path, value: &Value) -> Vec<ValidationMessage> {
        let _ = (path, value);
        Vec::new()
    }

    /// Default value template.
    ///
    /// Returns an optional subtree that should be merged when the file
    /// is empty or missing common keys.
    fn defaults(&self) -> Option<Value> {
        None
    }
}

/// The plugin registry — a list of loaded plugins.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// Find the first plugin that matches the given file name.
    pub fn find_for_file(&self, file_name: &str) -> Option<&dyn Plugin> {
        self.plugins
            .iter()
            .find(|p| p.matches_file(file_name))
            .map(|p| p.as_ref())
    }

    /// Return all registered plugins.
    #[allow(dead_code)]
    pub fn all(&self) -> &[Box<dyn Plugin>] {
        &self.plugins
    }
}
