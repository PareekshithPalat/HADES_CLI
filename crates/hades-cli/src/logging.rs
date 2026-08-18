use std::fs;
use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Initializes the file-based tracing logger, ensuring no logs interfere with terminal rendering.
/// Returns a `WorkerGuard` that flushes any pending log messages on drop.
pub fn init_logging(
    custom_log_dir: Option<&Path>,
) -> Result<Option<WorkerGuard>, Box<dyn std::error::Error>> {
    let log_dir = match custom_log_dir {
        Some(dir) => dir.to_path_buf(),
        None => match dirs::home_dir() {
            Some(home) => home.join(".hades").join("logs"),
            None => PathBuf::from(".hades").join("logs"),
        },
    };

    // Ensure the log directory exists
    if !log_dir.exists() {
        fs::create_dir_all(&log_dir)?;
    }

    let file_appender = tracing_appender::rolling::never(&log_dir, "hades.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,hades=debug"));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_filter(filter);

    tracing_subscriber::registry().with(file_layer).init();

    tracing::info!(
        log_path = %log_dir.join("hades.log").display(),
        "Logging initialized"
    );

    Ok(Some(guard))
}
