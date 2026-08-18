mod cli;
mod logging;

use clap::Parser;
use tracing::{error, info};

use cli::CliArgs;
use hades_config::ConfigService;
use hades_core::HadesApp;
use hades_events::EventBus;
use hades_storage::StorageService;
use hades_tui::TuiRunner;

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();

    // 1. Initialize background file logging
    let _log_guard = match logging::init_logging(args.log_dir.as_deref()) {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Warning: Failed to initialize file logger: {}", e);
            None
        }
    };

    info!("Starting Hades CLI");

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

    // 7. Launch interactive terminal user interface
    if let Err(e) = TuiRunner::run(&mut app, args.session).await {
        error!(error = %e, "TUI encountered an unexpected error");
        eprintln!("TUI runtime error: {}", e);
        std::process::exit(1);
    }

    info!("Hades exited cleanly");
}
