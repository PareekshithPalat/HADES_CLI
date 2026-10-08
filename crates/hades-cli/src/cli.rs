use clap::{Parser, Subcommand};
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

    /// Run a single prompt non-interactively, stream the answer to stdout and exit.
    /// Pass `-` to read the prompt from stdin.
    #[arg(short, long, value_name = "TEXT", allow_hyphen_values = true)]
    pub prompt: Option<String>,

    /// Optional subcommand
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum Commands {
    /// Launch Hades in Model Context Protocol (MCP) server mode over STDIO
    #[command(name = "mcp-server")]
    McpServer {
        /// Optional workspace directory path to serve (defaults to current working directory)
        #[arg(short, long, value_name = "DIR")]
        workspace: Option<PathBuf>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_flag_parses_short_and_long_forms() {
        let short = CliArgs::try_parse_from(["hades", "-p", "What is 2+2?"]).unwrap();
        assert_eq!(short.prompt.as_deref(), Some("What is 2+2?"));

        let long = CliArgs::try_parse_from(["hades", "--prompt", "explain", "-s", "abc"]).unwrap();
        assert_eq!(long.prompt.as_deref(), Some("explain"));
        assert_eq!(long.session.as_deref(), Some("abc"));
    }

    #[test]
    fn test_prompt_flag_accepts_stdin_marker_and_is_optional() {
        let stdin = CliArgs::try_parse_from(["hades", "-p", "-"]).unwrap();
        assert_eq!(stdin.prompt.as_deref(), Some("-"));

        let none = CliArgs::try_parse_from(["hades"]).unwrap();
        assert_eq!(none.prompt, None);
    }
}
