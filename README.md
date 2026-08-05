<div align="center">

# ConfUI

**Browse and edit config files (TOML, JSON, YAML) as an interactive tree — right in your terminal.**

</div>

```
▾ root  (6 keys)
  ▾ server  (3 keys)
      host: "0.0.0.0"
      port: 8080
      enabled: true
  ▾ items  (5 items)
    [0]: "a"
    [1]: 42
    [2]: 1.5
```

Think of it like a file explorer for configs. No more scrolling through raw text — navigate, search, and edit with keyboard shortcuts.

```bash
curl -fsSL https://github.com/Codevsun/ConfUI/releases/latest/download/confui-installer.sh | sh
```

## Table of contents

- [Install](#install)
- [Usage](#usage)
- [Keybindings](#keybindings)
- [Features at a glance](#features-at-a-glance)
- [Custom themes](#custom-themes)
- [Plugin API](#plugin-api)
- [Architecture](#architecture-for-contributors)
- [Development](#development)
- [License](#license)

---

## Install

### Quick (prebuilt binary)

```bash
curl -fsSL https://github.com/Codevsun/ConfUI/releases/latest/download/confui-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Codevsun/ConfUI/releases/latest/download/confui-installer.ps1 | iex"
```

No Rust toolchain required — this downloads a prebuilt binary for your platform from the [latest release](https://github.com/Codevsun/ConfUI/releases/latest).

### From source

Requires Rust 1.85+ (via [rustup.rs](https://rustup.rs)):

```bash
git clone https://github.com/Codevsun/ConfUI.git
cd confui
cargo install --path .
```

This places the `confui` binary in `~/.cargo/bin/` — make sure that directory is on your `PATH`.

> **Note:** this project isn't published on crates.io yet, so `cargo install confui`
> isn't available — use the prebuilt-binary installer or build from source above.

### Updating

The prebuilt-binary install also places a `confui-update` binary next to `confui`.
Run it any time to check for and install the latest release:

```bash
confui-update
```

(If you built from source, just `git pull && cargo install --path .` again.)

---

## Usage

```bash
confui path/to/config.toml
confui path/to/config.json
confui path/to/config.yaml
```

The format is detected automatically from the file extension (`.toml`, `.json`, `.yaml`, `.yml`) with content-based fallback.

---

## Keybindings

### Navigation

| Key | Action |
|---|---|
| `↑` / `↓` | Move cursor up / down the tree |
| `→` | Expand the current node |
| `←` | Collapse the current node |
| `Enter` | Toggle expand/collapse (on containers) or start editing (on values) |
| `Home` | Jump to the first line |
| `End` | Jump to the last line |
| `PageUp` / `PageDown` | Move one page at a time |
| `Ctrl+E` | Expand **all** nodes |
| `Ctrl+R` | Collapse **all** nodes |

### Editing values

| Key | Action |
|---|---|
| `Enter` (on a value) | Start editing |
| `Space` (on a boolean) | Toggle `true` / `false` immediately |
| `Esc` (while editing) | Cancel editing |
| `←` / `→` (while editing) | Move the cursor inside the text |
| `Backspace` / `Delete` (while editing) | Delete characters |
| `Home` / `End` (while editing) | Jump to start / end of the text |

### Adding, deleting & rearranging

| Key | Action |
|---|---|
| `i` | Insert a new key/item — prompts for a type (`s` string, `n` int, `f` float, `b` bool, `a` array, `o` object), `Esc` to cancel |
| `d` | Delete the current node (press **twice** to confirm) |
| `R` | Rename the current key (objects only) |
| `D` | Duplicate the current node |
| `Ctrl+X` | Cut the current node (copy to clipboard + remove) |
| `Ctrl+P` | Paste from clipboard at the current position |

If the cursor is on an object/array (including the root), `i` inserts a new child inside it; otherwise it inserts a sibling next to the cursor.

### Undo / Redo

| Key | Action |
|---|---|
| `Ctrl+Z` | Undo the last change |
| `Ctrl+Y` | Redo the last undone change |

### Search

| Key | Action |
|---|---|
| `/` | Enter search mode — type a query, press `Enter` to search |
| `n` | Jump to the **next** match |
| `N` | Jump to the **previous** match |
| `Esc` (while searching) | Cancel search |

### Save & quit

| Key | Action |
|---|---|
| `Ctrl+S` | Save the file (atomic write with `.bak` backup) |
| `q` | Quit |

### Theme

| Key | Action |
|---|---|
| `Ctrl+T` | Cycle through themes (Dark → Light → Catppuccin → Nord → Tokyo Night → Gruvbox) |

---

## Features at a glance

- **Three formats** — TOML, JSON, YAML. Detects automatically.
- **Interactive tree** — expand/collapse containers, see values inline.
- **Inline editing** — type-aware inputs for strings, integers, floats. Booleans toggle with Space.
- **Structure editing** — insert, delete (with confirmation), rename keys, duplicate, cut & paste.
- **Full undo/redo** — snapshot-based, up to 100 steps.
- **Search** — search keys and values, jump between matches with `n`/`N`.
- **Inline validation** — port ranges, URLs, IP addresses, number bounds, Rust editions — shown in the property panel.
- **Preserves TOML formatting** — uses `toml_edit`; keys/sections you didn't touch keep their original comments and layout on save (only the specific values you change are rewritten).
- **Safe saving** — atomic write: temp file → fsync → rename. A `.bak` backup is created first.
- **6 theme presets** — Dark, Light, Catppuccin, Nord, Tokyo Night, Gruvbox. Cycle with `Ctrl+T`.
- **Custom themes** — define your own colours in a TOML file.
- **Plugin system** — domain-specific docs and validation (built-in plugin for `Cargo.toml`).

---

## Custom themes

Create a TOML file with hex colours. Unspecified fields fall back to the Dark theme.

```toml
name = "Solarized Dark"
top_bar_bg = "#002b36"
top_bar_fg = "#839496"
accent_bg = "#268bd2"
accent_fg = "#002b36"
tree_key = "#268bd2"
tree_value = "#93a1a1"
tree_icon = "#b58900"
cursor_bg = "#b58900"
cursor_fg = "#002b36"
panel_header = "#268bd2"
panel_text = "#93a1a1"
status_bg = "#073642"
status_fg = "#839496"
edit_status_bg = "#859900"
edit_cursor = "#002b36"
edit_cursor_bg = "#b58900"
validation_error = "#dc322f"
validation_warning = "#b58900"
highlight_bg = "#b58900"
highlight_fg = "#002b36"
```

---

## Plugin API

Implement the `Plugin` trait to add docs, validation, or defaults for your own config files:

```rust
pub trait Plugin: Debug {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn matches_file(&self, file_name: &str) -> bool;
    fn docs_for(&self, path: &Path) -> Option<String>;
    fn validate(&self, path: &Path, value: &Value) -> Vec<ValidationMessage>;
    fn defaults(&self) -> Option<Value> { None }
}
```

See [`src/plugins/cargo_toml.rs`](src/plugins/cargo_toml.rs) for a complete example.

---

## Architecture (for contributors)

```
src/
├── main.rs              # CLI entrypoint, terminal setup
├── lib.rs               # Exports core + parser
├── core/                # Value tree model — no I/O, no UI
├── parser/               # All parsing/serialization (one file per format)
├── app/mod.rs            # App state + event loop + keyboard handling
├── ui/mod.rs             # Ratatui layout and rendering
├── widgets/mod.rs        # Visible line computation, value formatting
├── history/mod.rs        # Undo/redo (snapshot-based, 100 steps)
├── validation/mod.rs     # Inline validation (ports, URLs, IPs, etc.)
├── theme/mod.rs          # Theme definition, 6 presets, TOML loading
└── plugins/              # Plugin trait + built-in Cargo.toml plugin
    ├── mod.rs
    └── cargo_toml.rs
```

**Key design rules:**
- UI renders only from the tree model (`core::value::Value`), never from raw text.
- All parsing lives in `parser/` — the UI never knows what format the file is.
- Core modules have zero ratatui/crossterm imports; they compile and test headless.
- Saving never corrupts: temp file → fsync → atomic rename over original, with `.bak` backup.

---

## Development

```bash
cargo test                      # Run all tests
cargo fmt --check               # Check formatting
cargo clippy --all-targets -- -D warnings  # Lint
```

Contributions welcome — open an issue or PR on [GitHub](https://github.com/Codevsun/ConfUI).

---

## License

MIT — see [LICENSE](LICENSE).
