# ConfUI Build Plan
Work strictly top to bottom. One milestone per session.

- [x] M0: Scaffolding — cargo project, deps, module stubs, fixtures
- [x] M1: Core model — Value enum (Object/Array/String/Int/Float/Bool/Null),
      Path type, tree operations (get/set/insert/delete/rename/move),
      full unit tests. No I/O, no UI.
- [x] M2: Parsers — format detection (extension + content fallback),
      TOML/JSON/YAML → Value tree and back, round-trip tests against
      tests/fixtures/. TOML via toml_edit preserving comments/formatting.
- [x] M3: Read-only TUI — open file from CLI arg (clap), render tree
      sidebar + property panel + top bar + status bar, keyboard
      navigation (arrows, expand/collapse, PgUp/PgDn, Home/End, q).
      TestBackend rendering tests.
- [x] M4: Editing — Enter to edit inline, type-aware inputs (bool toggle
      with Space, string/number inputs with validation), modified
      indicator, Ctrl+S save with .bak backup + atomic write.
- [x] M5: Undo/redo — snapshot-based history, Ctrl+Z / Ctrl+Y.
- [x] M6: Structure editing — insert/delete/rename/duplicate/cut-paste
      keys and array items, confirmation for delete.
- [ ] M7: Search — `/` to search keys and values, highlight, n/N to jump.
- [ ] M8: Validation — inline errors (port range, number formats, URL,
      IP), shown in the right panel, never crash on bad input.
- [ ] M9: Themes — dark/light + Catppuccin/Nord/Tokyo Night/Gruvbox,
      custom themes from TOML.
- [ ] M10: Plugin API — trait-based plugin system providing docs,
      validation, defaults; ship one example plugin (Cargo.toml).