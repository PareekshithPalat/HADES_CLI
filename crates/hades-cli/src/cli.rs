use clap::Parser;
use std::path::PathBuf;

/// Hades: Universal AI Agent CLI
#[derive(Debug, Parser)]
#[command(
    name = "hades",
    author,
    version,
    about = "Universal AI Agent CLI",
    long_about = "Hades is a cross-platform, universal AI agent CLI runtime."
)]
pub struct CliArgs {
    /// Custom path to configuration file (defaults to ~/.hades/config.toml)
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Custom directory for persistent storage (defaults to ~/.hades/data)
    #[arg(short, long, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    /// Custom directory for log files (defaults to ~/.hades/logs)
    #[arg(short, long, value_name = "DIR")]
    pub log_dir: Option<PathBuf>,

    /// Explicitly resume a previous conversation session by ID
    #[arg(short, long, value_name = "SESSION_ID")]
    pub session: Option<String>,
}
