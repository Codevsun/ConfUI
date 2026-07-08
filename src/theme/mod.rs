//! Theme definitions and loading.
//!
//! Provides preset themes (dark, light, Catppuccin, Nord, Tokyo Night,
//! Gruvbox) and the ability to load custom themes from TOML files.

use serde::Deserialize;

/// A complete set of colours for the ConfUI TUI.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Theme {
    /// Display name.
    pub name: &'static str,
    /// Background colour for the top bar.
    pub top_bar_bg: (u8, u8, u8),
    /// Text colour in the top bar.
    pub top_bar_fg: (u8, u8, u8),
    /// Accent colour (ConfUI badge).
    pub accent_bg: (u8, u8, u8),
    pub accent_fg: (u8, u8, u8),
    /// Tree sidebar — key names.
    pub tree_key: (u8, u8, u8),
    /// Tree sidebar — value summaries.
    pub tree_value: (u8, u8, u8),
    /// Tree sidebar — expand/collapse icons.
    pub tree_icon: (u8, u8, u8),
    /// Tree sidebar — cursor highlight background.
    pub cursor_bg: (u8, u8, u8),
    /// Tree sidebar — cursor highlight foreground.
    pub cursor_fg: (u8, u8, u8),
    /// Property panel — section headers.
    pub panel_header: (u8, u8, u8),
    /// Property panel — normal text.
    pub panel_text: (u8, u8, u8),
    /// Status bar background (normal mode).
    pub status_bg: (u8, u8, u8),
    /// Status bar foreground (normal mode).
    pub status_fg: (u8, u8, u8),
    /// Status bar background (editing mode).
    pub edit_status_bg: (u8, u8, u8),
    /// Edit buffer background.
    pub edit_bg: (u8, u8, u8),
    /// Edit cursor colour.
    pub edit_cursor: (u8, u8, u8),
    /// Edit cursor background.
    pub edit_cursor_bg: (u8, u8, u8),
    /// Validation error prefix colour.
    pub validation_error: (u8, u8, u8),
    /// Validation warning prefix colour.
    pub validation_warning: (u8, u8, u8),
    /// Selection highlight background in list.
    pub highlight_bg: (u8, u8, u8),
    pub highlight_fg: (u8, u8, u8),
}

// ── helper to create ratatui colours ──────────────────────────────

#[allow(dead_code)]
pub(crate) fn rgb(r: u8, g: u8, b: u8) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(r, g, b)
}

// ── presets ───────────────────────────────────────────────────────

/// Default dark theme.
pub const DARK: Theme = Theme {
    name: "Dark",
    top_bar_bg: (60, 60, 60),
    top_bar_fg: (255, 255, 255),
    accent_bg: (33, 150, 243),
    accent_fg: (255, 255, 255),
    tree_key: (0, 188, 212),
    tree_value: (200, 200, 200),
    tree_icon: (255, 235, 59),
    cursor_bg: (255, 235, 59),
    cursor_fg: (0, 0, 0),
    panel_header: (33, 150, 243),
    panel_text: (200, 200, 200),
    status_bg: (60, 60, 60),
    status_fg: (255, 255, 255),
    edit_status_bg: (139, 195, 74),
    edit_bg: (60, 60, 60),
    edit_cursor: (0, 0, 0),
    edit_cursor_bg: (255, 235, 59),
    validation_error: (239, 83, 80),
    validation_warning: (255, 193, 7),
    highlight_bg: (255, 235, 59),
    highlight_fg: (0, 0, 0),
};

/// Light theme.
pub const LIGHT: Theme = Theme {
    name: "Light",
    top_bar_bg: (200, 200, 200),
    top_bar_fg: (30, 30, 30),
    accent_bg: (33, 150, 243),
    accent_fg: (255, 255, 255),
    tree_key: (0, 105, 120),
    tree_value: (80, 80, 80),
    tree_icon: (180, 120, 0),
    cursor_bg: (100, 181, 246),
    cursor_fg: (255, 255, 255),
    panel_header: (21, 101, 192),
    panel_text: (50, 50, 50),
    status_bg: (200, 200, 200),
    status_fg: (30, 30, 30),
    edit_status_bg: (139, 195, 74),
    edit_bg: (230, 230, 230),
    edit_cursor: (255, 255, 255),
    edit_cursor_bg: (33, 150, 243),
    validation_error: (198, 40, 40),
    validation_warning: (180, 120, 0),
    highlight_bg: (100, 181, 246),
    highlight_fg: (255, 255, 255),
};

/// Catppuccin Mocha.
pub const CATPPUCCIN: Theme = Theme {
    name: "Catppuccin",
    top_bar_bg: (30, 30, 46),
    top_bar_fg: (205, 214, 244),
    accent_bg: (137, 180, 250),
    accent_fg: (30, 30, 46),
    tree_key: (137, 180, 250),
    tree_value: (186, 194, 222),
    tree_icon: (249, 226, 175),
    cursor_bg: (249, 226, 175),
    cursor_fg: (30, 30, 46),
    panel_header: (137, 180, 250),
    panel_text: (186, 194, 222),
    status_bg: (30, 30, 46),
    status_fg: (205, 214, 244),
    edit_status_bg: (166, 227, 161),
    edit_bg: (69, 71, 90),
    edit_cursor: (30, 30, 46),
    edit_cursor_bg: (249, 226, 175),
    validation_error: (243, 139, 168),
    validation_warning: (249, 226, 175),
    highlight_bg: (249, 226, 175),
    highlight_fg: (30, 30, 46),
};

/// Nord.
pub const NORD: Theme = Theme {
    name: "Nord",
    top_bar_bg: (46, 52, 64),
    top_bar_fg: (216, 222, 233),
    accent_bg: (94, 129, 172),
    accent_fg: (216, 222, 233),
    tree_key: (136, 192, 208),
    tree_value: (216, 222, 233),
    tree_icon: (235, 203, 139),
    cursor_bg: (235, 203, 139),
    cursor_fg: (46, 52, 64),
    panel_header: (136, 192, 208),
    panel_text: (216, 222, 233),
    status_bg: (46, 52, 64),
    status_fg: (216, 222, 233),
    edit_status_bg: (163, 190, 140),
    edit_bg: (59, 66, 82),
    edit_cursor: (46, 52, 64),
    edit_cursor_bg: (235, 203, 139),
    validation_error: (191, 97, 106),
    validation_warning: (235, 203, 139),
    highlight_bg: (235, 203, 139),
    highlight_fg: (46, 52, 64),
};

/// Tokyo Night.
pub const TOKYO_NIGHT: Theme = Theme {
    name: "Tokyo Night",
    top_bar_bg: (26, 27, 38),
    top_bar_fg: (169, 177, 214),
    accent_bg: (122, 162, 247),
    accent_fg: (26, 27, 38),
    tree_key: (122, 162, 247),
    tree_value: (169, 177, 214),
    tree_icon: (224, 175, 104),
    cursor_bg: (224, 175, 104),
    cursor_fg: (26, 27, 38),
    panel_header: (122, 162, 247),
    panel_text: (169, 177, 214),
    status_bg: (26, 27, 38),
    status_fg: (169, 177, 214),
    edit_status_bg: (158, 206, 106),
    edit_bg: (41, 42, 58),
    edit_cursor: (26, 27, 38),
    edit_cursor_bg: (224, 175, 104),
    validation_error: (247, 118, 142),
    validation_warning: (224, 175, 104),
    highlight_bg: (224, 175, 104),
    highlight_fg: (26, 27, 38),
};

/// Gruvbox dark.
pub const GRUVBOX: Theme = Theme {
    name: "Gruvbox",
    top_bar_bg: (40, 40, 40),
    top_bar_fg: (235, 219, 178),
    accent_bg: (69, 133, 136),
    accent_fg: (235, 219, 178),
    tree_key: (131, 165, 152),
    tree_value: (235, 219, 178),
    tree_icon: (215, 153, 33),
    cursor_bg: (215, 153, 33),
    cursor_fg: (40, 40, 40),
    panel_header: (131, 165, 152),
    panel_text: (235, 219, 178),
    status_bg: (40, 40, 40),
    status_fg: (235, 219, 178),
    edit_status_bg: (184, 187, 38),
    edit_bg: (60, 56, 54),
    edit_cursor: (40, 40, 40),
    edit_cursor_bg: (215, 153, 33),
    validation_error: (204, 36, 29),
    validation_warning: (215, 153, 33),
    highlight_bg: (215, 153, 33),
    highlight_fg: (40, 40, 40),
};

/// All preset themes in display/cycle order.
pub const PRESETS: &[&Theme] = &[&DARK, &LIGHT, &CATPPUCCIN, &NORD, &TOKYO_NIGHT, &GRUVBOX];

// ── TOML loading ─────────────────────────────────────────────────

/// Serializable representation of a theme for TOML loading.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ThemeConfig {
    name: Option<String>,
    top_bar_bg: Option<String>,
    top_bar_fg: Option<String>,
    accent_bg: Option<String>,
    accent_fg: Option<String>,
    tree_key: Option<String>,
    tree_value: Option<String>,
    tree_icon: Option<String>,
    cursor_bg: Option<String>,
    cursor_fg: Option<String>,
    panel_header: Option<String>,
    panel_text: Option<String>,
    status_bg: Option<String>,
    status_fg: Option<String>,
    edit_status_bg: Option<String>,
    edit_bg: Option<String>,
    edit_cursor: Option<String>,
    edit_cursor_bg: Option<String>,
    validation_error: Option<String>,
    validation_warning: Option<String>,
    highlight_bg: Option<String>,
    highlight_fg: Option<String>,
}

/// Parse a colour string in the form `"#rrggbb"` into an `(r, g, b)` tuple.
#[allow(dead_code)]
fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Load a custom theme from a TOML string.
///
/// Any field not specified will fall back to the `Dark` theme default.
#[allow(dead_code)]
pub fn load_from_toml(toml_str: &str) -> Result<Theme, String> {
    let cfg: ThemeConfig =
        toml::from_str(toml_str).map_err(|e| format!("Invalid theme TOML: {e}"))?;

    let fallback = &DARK;
    let name = cfg.name.unwrap_or_else(|| "Custom".into());
    let name: &'static str = Box::leak(name.into_boxed_str());

    Ok(Theme {
        name,
        top_bar_bg: cfg
            .top_bar_bg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.top_bar_bg),
        top_bar_fg: cfg
            .top_bar_fg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.top_bar_fg),
        accent_bg: cfg
            .accent_bg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.accent_bg),
        accent_fg: cfg
            .accent_fg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.accent_fg),
        tree_key: cfg
            .tree_key
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.tree_key),
        tree_value: cfg
            .tree_value
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.tree_value),
        tree_icon: cfg
            .tree_icon
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.tree_icon),
        cursor_bg: cfg
            .cursor_bg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.cursor_bg),
        cursor_fg: cfg
            .cursor_fg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.cursor_fg),
        panel_header: cfg
            .panel_header
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.panel_header),
        panel_text: cfg
            .panel_text
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.panel_text),
        status_bg: cfg
            .status_bg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.status_bg),
        status_fg: cfg
            .status_fg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.status_fg),
        edit_status_bg: cfg
            .edit_status_bg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.edit_status_bg),
        edit_bg: cfg
            .edit_bg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.edit_bg),
        edit_cursor: cfg
            .edit_cursor
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.edit_cursor),
        edit_cursor_bg: cfg
            .edit_cursor_bg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.edit_cursor_bg),
        validation_error: cfg
            .validation_error
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.validation_error),
        validation_warning: cfg
            .validation_warning
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.validation_warning),
        highlight_bg: cfg
            .highlight_bg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.highlight_bg),
        highlight_fg: cfg
            .highlight_fg
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or(fallback.highlight_fg),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_valid() {
        assert_eq!(parse_hex("#ff0000"), Some((255, 0, 0)));
        assert_eq!(parse_hex("#00ff00"), Some((0, 255, 0)));
        assert_eq!(parse_hex("#0000ff"), Some((0, 0, 255)));
        assert_eq!(parse_hex("#ffffff"), Some((255, 255, 255)));
        assert_eq!(parse_hex("#000000"), Some((0, 0, 0)));
    }

    #[test]
    fn parse_hex_invalid() {
        assert_eq!(parse_hex("ff0000"), None);
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#gggggg"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn load_from_toml_full() {
        let toml = "\
name = \"Test Theme\"\n\
top_bar_bg = \"#111111\"\n\
top_bar_fg = \"#ffffff\"\n\
accent_bg = \"#222222\"\n\
accent_fg = \"#cccccc\"\n\
tree_key = \"#00ff00\"\n\
tree_value = \"#aaaaaa\"\n\
tree_icon = \"#ffff00\"\n\
cursor_bg = \"#ff0000\"\n\
cursor_fg = \"#000000\"\n\
panel_header = \"#333333\"\n\
panel_text = \"#bbbbbb\"\n\
status_bg = \"#444444\"\n\
status_fg = \"#dddddd\"\n\
edit_status_bg = \"#555555\"\n\
edit_bg = \"#666666\"\n\
edit_cursor = \"#777777\"\n\
edit_cursor_bg = \"#888888\"\n\
validation_error = \"#ff0000\"\n\
validation_warning = \"#ffaa00\"\n\
highlight_bg = \"#cccccc\"\n\
highlight_fg = \"#111111\"\n";
        let theme = load_from_toml(toml).unwrap();
        assert_eq!(theme.name, "Test Theme");
        assert_eq!(theme.top_bar_bg, (17, 17, 17));
        assert_eq!(theme.top_bar_fg, (255, 255, 255));
        assert_eq!(theme.tree_key, (0, 255, 0));
        assert_eq!(theme.validation_error, (255, 0, 0));
    }

    #[test]
    fn load_from_toml_partial() {
        let toml = "\
name = \"Partial\"\n\
tree_key = \"#aabbcc\"\n";
        let theme = load_from_toml(toml).unwrap();
        assert_eq!(theme.name, "Partial");
        assert_eq!(theme.tree_key, (170, 187, 204));
        // Unspecified fields fall back to Dark
        assert_eq!(theme.top_bar_bg, DARK.top_bar_bg);
    }
}
