//! Layout and screen composition for the TUI.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Widget,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::App;
use crate::widgets::format_value_detailed;

/// Initialize the terminal for ratatui.
pub fn init_terminal()
-> color_eyre::Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let terminal = ratatui::Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to normal mode.
pub fn restore_terminal() -> color_eyre::Result<()> {
    use crossterm::cursor::Show;
    use crossterm::execute;
    use crossterm::terminal::disable_raw_mode;
    disable_raw_mode()?;
    execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        Show
    )?;
    Ok(())
}

/// Render the entire TUI layout.
pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();

    // Layout: top bar (1 line), main area, status bar (1 line)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top bar
            Constraint::Min(0),    // Main area
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    render_top_bar(frame, app, chunks[0]);

    // Main area: sidebar (tree) and property panel
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Tree sidebar
            Constraint::Percentage(60), // Property panel
        ])
        .split(chunks[1]);

    render_sidebar(frame, app, main_chunks[0]);
    render_property_panel(frame, app, main_chunks[1]);

    render_status_bar(frame, app, chunks[2]);
}

/// Render the top bar showing file info.
fn render_top_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let format_str = match app.format {
        confui::parser::Format::Json => "JSON",
        confui::parser::Format::Toml => "TOML",
        confui::parser::Format::Yaml => "YAML",
    };

    let file_name = app
        .file_path
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    let modified_indicator = if app.modified { " ●" } else { "" };

    let text = Line::from(vec![
        Span::styled(
            " ConfUI ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            file_name.as_ref(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  [{format_str}]{modified_indicator}")),
    ]);

    let block = Block::default().style(Style::default().bg(Color::DarkGray));
    Paragraph::new(text)
        .block(block)
        .render(area, frame.buffer_mut());
}

/// Render the tree sidebar.
fn render_sidebar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let lines = app.compute_visible_lines();
    let items: Vec<ListItem> = lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            // Indentation
            let indent = "  ".repeat(line.depth);

            // Expand/collapse indicator
            let icon = if line.is_leaf {
                "  "
            } else if line.is_expanded {
                "▾ "
            } else {
                "▸ "
            };

            let key_style = if idx == app.cursor_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            let value_style = if idx == app.cursor_index {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };

            let content = Line::from(vec![
                Span::raw(indent),
                Span::styled(icon, Style::default().fg(Color::Yellow)),
                Span::styled(line.key.clone(), key_style),
                Span::raw(" "),
                Span::styled(&line.value_summary, value_style),
            ]);

            ListItem::new(content)
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Tree ")
        .style(Style::default());

    let mut state = ListState::default().with_selected(Some(app.cursor_index));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    // We render using the state
    frame.render_stateful_widget(list, area, &mut state);
}

/// Render the property panel showing details about the selected node.
/// When in edit mode, shows an input field for editing the value.
fn render_property_panel(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let cursor_path = app.cursor_path();
    let selected = app.tree.get(&cursor_path);

    let mut lines: Vec<Line> = Vec::new();

    // Path display
    lines.push(Line::from(Span::styled(
        "Path:",
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Blue),
    )));

    let path_str = if cursor_path.is_empty() {
        "/".to_string()
    } else {
        let mut s = String::new();
        for seg in &cursor_path {
            match seg {
                confui::core::PathSegment::Key(k) => s.push_str(&format!("/{k}")),
                confui::core::PathSegment::Index(i) => s.push_str(&format!("/[{i}]")),
            }
        }
        s
    };

    let display_path = if app.editing {
        // Show the edit path during editing (even if cursor moved)
        let mut s = String::new();
        for seg in &app.edit_path {
            match seg {
                confui::core::PathSegment::Key(k) => s.push_str(&format!("/{k}")),
                confui::core::PathSegment::Index(i) => s.push_str(&format!("/[{i}]")),
            }
        }
        s
    } else {
        path_str
    };
    lines.push(Line::from(Span::raw(display_path)));
    lines.push(Line::from(Span::raw("")));

    if app.editing {
        // ── edit mode rendering ──────────────────────────────────
        // Show the type of the value being edited
        if let Some(value) = app.tree.get(&app.edit_path) {
            lines.push(Line::from(Span::styled(
                "Type:",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Blue),
            )));
            lines.push(Line::from(Span::raw(value.type_name())));
            lines.push(Line::from(Span::raw("")));
        }

        lines.push(Line::from(Span::styled(
            "Edit Value:",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Green),
        )));

        // Render buffer with cursor
        let cursor_pos = app.edit_cursor.min(app.edit_buffer.len());
        let before = &app.edit_buffer[..cursor_pos];
        let after = &app.edit_buffer[cursor_pos..];
        let display_line = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                before,
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ),
            Span::styled(
                "█",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(after, Style::default().fg(Color::White).bg(Color::DarkGray)),
            Span::raw(" "),
        ]);
        lines.push(display_line);

        // Instructions
        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::styled(
            "Enter: confirm   Esc: cancel   ← →: cursor",
            Style::default().fg(Color::DarkGray),
        )));
    } else if let Some(value) = selected {
        // ── normal (non-edit) rendering ───────────────────────────
        // Type
        lines.push(Line::from(Span::styled(
            "Type:",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Blue),
        )));
        lines.push(Line::from(Span::raw(value.type_name())));
        lines.push(Line::from(Span::raw("")));

        // Children count
        match value {
            confui::core::Value::Object(map) => {
                lines.push(Line::from(format!("Keys: {}", map.len())));
                // List keys
                if !map.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "Children:",
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(Color::Blue),
                    )));
                    for key in map.keys() {
                        lines.push(Line::from(format!("  • {key}")));
                    }
                }
            }
            confui::core::Value::Array(arr) => {
                lines.push(Line::from(format!("Items: {}", arr.len())));
            }
            _ => {}
        }

        // Value for leaf nodes
        if value.is_leaf() {
            lines.push(Line::from(Span::raw("")));
            lines.push(Line::from(Span::styled(
                "Value:",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Blue),
            )));
            let detailed = format_value_detailed(value);
            lines.push(Line::from(Span::raw(detailed)));
        } else {
            lines.push(Line::from(Span::raw("")));
            lines.push(Line::from(Span::styled(
                "Preview:",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Blue),
            )));
            let detailed = format_value_detailed(value);
            for line in detailed.lines() {
                lines.push(Line::from(Span::raw(line.to_string())));
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            "(nothing selected)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Properties ")
        .style(Style::default());

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.property_scroll as u16, 0));

    frame.render_widget(paragraph, area);
}

/// Render the status bar at the bottom.
fn render_status_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let (text, bg_color) = if app.editing {
        (
            Line::from(vec![Span::styled(
                &app.status,
                Style::default().fg(Color::Black).bg(Color::LightGreen),
            )]),
            Color::LightGreen,
        )
    } else {
        (
            Line::from(vec![Span::styled(
                &app.status,
                Style::default().fg(Color::White).bg(Color::DarkGray),
            )]),
            Color::DarkGray,
        )
    };

    let block = Block::default().style(Style::default().bg(bg_color));
    Paragraph::new(text)
        .block(block)
        .render(area, frame.buffer_mut());
}
