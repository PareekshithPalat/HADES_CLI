mod cli;
mod completions;
mod logging;
mod prune;

use clap::Parser;
use std::io::Read;
use tracing::{error, info};

use cli::CliArgs;
use hades_config::ConfigService;
use hades_core::HadesApp;
use hades_events::EventBus;
use hades_storage::{FileSessionRepository, StorageService};
use hades_tui::TuiRunner;

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();

    // Shell completion generation: print the script and exit before any setup
    if let Some(shell) = args.completions {
        let bin_name = completions::invoked_bin_name();
        completions::write_completions(shell, &bin_name, &mut std::io::stdout());
        return;
    }

    // 1. Initialize background file logging
    let _log_guard = match logging::init_logging(args.log_dir.as_deref()) {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Warning: Failed to initialize file logger: {}", e);
            None
        }
    };

    info!("Starting Hades CLI");

    // Check if MCP server mode was requested
    if let Some(cli::Commands::McpServer { workspace }) = args.command {
        let work_dir = workspace.unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
        let server = hades_mcp::HadesMcpServer::new(work_dir);
        if let Err(e) = server.run_stdio().await {
            error!(error = %e, "Hades MCP server terminated with error");
            eprintln!("MCP Server error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Session maintenance mode (`--prune`): runs without initializing the full runtime
    if let Some(criteria) = prune::criteria_from_args(args.prune, args.prune_older_than) {
        let repository = match FileSessionRepository::new() {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("Error resolving session storage: {}", e);
                std::process::exit(1);
            }
        };
        if let Err(e) = prune::run(&repository, criteria, &mut std::io::stdout()).await {
            error!(error = %e, "Session prune failed");
            eprintln!("Error pruning sessions: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // 2. Initialize configuration service
    let config_service = match args.config {
        Some(path) => ConfigService::with_path(path),
        None => match ConfigService::new() {
            Ok(service) => service,
            Err(e) => {
                error!(error = %e, "Failed to resolve default configuration path");
                eprintln!("Error initializing configuration: {}", e);
                std::process::exit(1);
            }
        },
    };

    // 3. Initialize storage service
    let storage_service = match args.data_dir {
        Some(path) => StorageService::with_root(path),
        None => match StorageService::new() {
            Ok(service) => service,
            Err(e) => {
                error!(error = %e, "Failed to resolve default storage path");
                eprintln!("Error initializing storage: {}", e);
                std::process::exit(1);
            }
        },
    };

    // 4. Initialize event bus
    let event_bus = EventBus::new();

    // 5. Construct core application runtime
    let mut app = HadesApp::new(config_service, storage_service, event_bus);

    // 6. Initialize core runtime subsystems
    if let Err(e) = app.init() {
        error!(error = %e, "Hades core initialization failed");
        eprintln!("Initialization error: {}", e);
        std::process::exit(1);
    }

    // 7. Non-interactive one-off prompt (`--prompt` / `-p`) bypasses the TUI entirely
    if let Some(prompt) = args.prompt {
        let succeeded = run_headless_prompt(&mut app, &prompt, args.session.as_deref()).await;
        // Drop the app first so MCP and browser child processes are cleaned up before exiting.
        drop(app);
        std::process::exit(if succeeded { 0 } else { 1 });
    }

    // 8. Launch interactive terminal user interface
    if let Err(e) = TuiRunner::run(&mut app, args.session).await {
        error!(error = %e, "TUI encountered an unexpected error");
        eprintln!("TUI runtime error: {}", e);
        std::process::exit(1);
    }

    info!("Hades exited cleanly");
}

/// Runs a single prompt without the TUI. The answer goes to stdout and diagnostics to
/// stderr, so the output can be piped. Returns whether the run succeeded.
async fn run_headless_prompt(app: &mut HadesApp, prompt: &str, session: Option<&str>) -> bool {
    let prompt = if prompt == "-" {
        let mut buffer = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut buffer) {
            eprintln!("Error: failed to read prompt from stdin: {e}");
            return false;
        }
        buffer
    } else {
        prompt.to_string()
    };

    if app.model_manager().active_model_id().is_none() {
        eprintln!(
            "Error: No active AI model configured. Run `hades` and select one with /model before using --prompt."
        );
        return false;
    }

    match app.init_session(session).await {
        Ok(Some(warning)) => eprintln!("Warning: {warning}"),
        Ok(None) => {}
        Err(e) => {
            eprintln!("Error: {e}");
            return false;
        }
    }

    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr();
    match app
        .run_headless_prompt(&prompt, &mut stdout, &mut stderr)
        .await
    {
        Ok(outcome) => {
            info!(
                tools_executed = outcome.tools_executed,
                tools_denied = outcome.tools_denied,
                "Headless prompt completed"
            );
            true
        }
        Err(e) => {
            error!(error = %e, "Headless prompt failed");
            drop(stdout);
            eprintln!("\nError: {e}");
            false
        }
    }
}
