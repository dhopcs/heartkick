//! CLI surface. Single binary, multiple modes.

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "heartkick",
    version,
    about = "Heart rate monitor app and daemon"
)]
pub struct Cli {
    /// Run as a headless daemon (no Tauri window).
    #[arg(long)]
    pub daemon: bool,

    /// Force the Tauri/webview GUI regardless of the config launch_mode setting.
    #[arg(long, conflicts_with = "tui")]
    pub gui: bool,

    /// Force the terminal UI regardless of the config launch_mode setting.
    #[arg(long, conflicts_with = "gui")]
    pub tui: bool,

    /// Override the log level (trace, debug, info, warn, error).
    #[arg(long)]
    pub log: Option<String>,
}
