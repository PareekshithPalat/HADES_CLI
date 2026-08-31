use thiserror::Error;

/// Domain error type representing any failure within the browser and web retrieval subsystem.
#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("Browser executable binary not found on host system: {0}")]
    BrowserNotFound(String),

    #[error("Failed to spawn browser sidecar process: {0}")]
    BrowserLaunchFailed(String),

    #[error("Failed to establish CDP connection to browser endpoint '{endpoint}': {details}")]
    BrowserConnectionFailed { endpoint: String, details: String },

    #[error("CDP connection disconnected unexpectedly: {0}")]
    BrowserDisconnected(String),

    #[error("Browser operation timed out after {timeout_secs}s: {details}")]
    BrowserTimeout { timeout_secs: u64, details: String },

    #[error("Page navigation failed for '{url}': {reason}")]
    BrowserNavigationFailed { url: String, reason: String },

    #[error("Permission denied for browser action '{action}' on domain '{domain}': {reason}")]
    BrowserPermissionDenied {
        action: String,
        domain: String,
        reason: String,
    },

    #[error("Browser action limit exceeded: {current} actions executed (max allowed: {max})")]
    BrowserActionLimitExceeded { current: usize, max: usize },

    #[error("Browser action loop detected: repeated action '{action}' on element '{element_ref}' without meaningful page state change")]
    BrowserLoopDetected { action: String, element_ref: String },

    #[error("Stale element reference '{element_ref}': page snapshot generation has expired or DOM changed")]
    StaleElementReference { element_ref: String },

    #[error("Target element '{target}' was not found in active page accessibility snapshot")]
    ElementNotFound { target: String },

    #[error("Element '{target}' is not interactable (hidden, disabled, or obstructed): {reason}")]
    ElementNotInteractable { target: String, reason: String },

    #[error("Download operation failed for '{filename}': {reason}")]
    DownloadFailed { filename: String, reason: String },

    #[error("Invalid browser runtime state: {0}")]
    InvalidBrowserState(String),

    #[error("Web retrieval error for '{url}': {reason}")]
    RetrievalFailed { url: String, reason: String },

    #[error("Search query execution failed: {0}")]
    SearchFailed(String),

    #[error("CDP protocol error ({code}): {message}")]
    CdpProtocolError { code: i64, message: String },

    #[error("I/O error during browser operation: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL parsing error for '{0}'")]
    UrlParse(String),
}
