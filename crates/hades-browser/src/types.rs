use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Operating mode of the browser automation subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BrowserMode {
    /// Fresh temporary profile per session with isolated cookies/storage, automatically cleaned up.
    #[default]
    Isolated,
    /// Dedicated persistent Hades profile stored at `~/.hades/browser/profile/` for repeated workflows.
    Persistent,
    /// Connect to an existing user-started browser instance via an explicit remote debugging port.
    Attach,
}

impl fmt::Display for BrowserMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Isolated => write!(f, "Isolated (Ephemeral)"),
            Self::Persistent => write!(f, "Persistent Profile"),
            Self::Attach => write!(f, "Attach Existing"),
        }
    }
}

/// Supported browser engine variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BrowserType {
    #[default]
    Chrome,
    Chromium,
    Edge,
    Brave,
    Custom,
}

impl fmt::Display for BrowserType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Chrome => write!(f, "Google Chrome"),
            Self::Chromium => write!(f, "Chromium"),
            Self::Edge => write!(f, "Microsoft Edge"),
            Self::Brave => write!(f, "Brave"),
            Self::Custom => write!(f, "Custom Browser"),
        }
    }
}

/// Metadata describing a detected browser binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserInfo {
    pub browser_type: BrowserType,
    pub name: String,
    pub version: String,
    pub binary_path: PathBuf,
    pub is_available: bool,
}

/// Information about an open browser tab / target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabInfo {
    pub tab_id: String,
    pub target_id: String,
    pub url: String,
    pub title: String,
    pub is_active: bool,
    pub websocket_url: Option<String>,
}

/// Accessibility-first element reference within a page snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementRef {
    /// LLM-facing stable identifier (e.g., "ref_001").
    pub id: String,
    /// Semantic accessibility role (e.g., "button", "link", "textbox", "heading").
    pub role: String,
    /// Accessible name or visible text label.
    pub name: String,
    /// Current input value or placeholder, if any.
    pub value: Option<String>,
    /// Fallback CSS selector.
    pub selector: String,
    /// CDP BackendNodeId for direct protocol actions.
    pub backend_node_id: Option<i64>,
    /// Whether element is currently visible and interactable.
    pub is_interactable: bool,
}

/// Structured, accessibility-aware snapshot of a web page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSnapshot {
    pub url: String,
    pub title: String,
    pub tab_id: String,
    pub generation: u64,
    pub elements: Vec<ElementRef>,
    pub content_summary: String,
    pub accessibility_tree_text: String,
    pub captured_at: DateTime<Utc>,
}

/// Wait strategy condition after page navigation or interaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationWait {
    DomContentLoaded,
    Load,
    NetworkIdle,
    Selector(String),
    Element(String),
}

/// Live status summary of the browser automation sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserStatus {
    pub is_running: bool,
    pub mode: BrowserMode,
    pub browser_name: String,
    pub version: String,
    pub binary_path: Option<PathBuf>,
    pub active_tabs: usize,
    pub active_session_id: Option<String>,
    pub cdp_port: Option<u16>,
    pub uptime_secs: u64,
}

/// Browser console log entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleEntry {
    pub level: String,
    pub text: String,
    pub source: String,
    pub timestamp: DateTime<Utc>,
}

/// Sanitized network request log entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEntry {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub status: Option<u16>,
    pub mime_type: Option<String>,
    pub timing_ms: Option<u64>,
    pub error: Option<String>,
}

/// Metadata for a downloaded file artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadArtifact {
    pub id: String,
    pub filename: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub mime_type: String,
    pub timestamp: DateTime<Utc>,
}

/// Metadata for a page screenshot artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotArtifact {
    pub id: String,
    pub path: PathBuf,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub timestamp: DateTime<Utc>,
}

/// Metadata for a generated PDF artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfArtifact {
    pub id: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub timestamp: DateTime<Utc>,
}

/// A structured result item from a search engine query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}
