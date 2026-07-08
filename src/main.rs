//! ConfUI — terminal TUI for browsing and editing config files.
//!
//! Opens a config file (TOML, JSON, YAML) and lets the user browse it
//! as an interactive tree. Use arrow keys to navigate, q to quit.

use clap::Parser;
use color_eyre::Result;

mod app;
mod history;
mod plugins;
mod theme;
mod ui;
mod validation;
mod widgets;

/// ConfUI — browse and edit config files in the terminal.
#[derive(Parser, Debug)]
#[command(name = "confui", about = "A TUI config file editor")]
struct Args {
    /// Path to the config file to open.
    file: std::path::PathBuf,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    // Set up the terminal
    let mut terminal = ui::init_terminal()?;

    // Run the app
    let result = app::run(&mut terminal, &args.file);

    // Restore terminal before handling result
    ui::restore_terminal()?;

    result
}
