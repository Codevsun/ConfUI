# ConfUI — Agent Instructions

## What this project is
ConfUI is a terminal TUI application (Rust, ratatui) that opens any config file
(TOML, JSON, YAML) and lets the user browse and edit it as an interactive tree
instead of raw text. Think "VS Code for config files, in the terminal."

## Architecture invariants (never violate these)
1. The UI renders ONLY from the generic tree model (`core::value::Value`),
   never from raw file text.
2. All parsing/serialization lives in `src/parser/`. No serde imports anywhere
   else. The UI layer never knows what format the file was.
3. Business logic (core, parser, history, validation) must have ZERO ratatui
   or crossterm imports. It must compile and test headless.
4. Saving must never corrupt a file: write to a temp file in the same
   directory, fsync, then atomically rename over the original. Create a
   `.bak` backup first.
5. Use `toml_edit` (not `toml`) for TOML so formatting and comments are
   preserved on save.
6. Keys/nodes are addressed by a `Path` type (list of segments), never by
   string concatenation.

## Workflow rules
- Work on ONE milestone from PLAN.md at a time. Do not scaffold or stub
  future milestones. Do not add features not in the current milestone.
- Read PLAN.md at the start of every session to find the current milestone.
- Update PLAN.md checkboxes only when the verification loop passes.
- Make small, focused changes. Prefer editing existing files over rewriting.

## Verification loop (definition of "done")
A task is NOT complete until all of these pass:
1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`
Never report success if any step fails. Run them yourself; do not ask the
user to run them.

## Testing rules
- Every parser must have round-trip tests: parse → serialize → parse →
  assert trees equal. Use real-world fixture files in `tests/fixtures/`.
- Core tree operations (insert, delete, rename, move) get unit tests.
- TUI rendering is tested headless with ratatui's `TestBackend`: render to a
  buffer and assert on its contents. No test may require a real terminal.

## Rust style
- Idiomatic stable Rust. No `unwrap()`/`expect()` outside tests; use
  `color-eyre::Result` and `?`.
- No `unsafe`.
- Doc comments (`///`) on all public items.
- Keep modules small; split files that grow past ~300 lines.
- Avoid unnecessary `clone()`; prefer borrowing.