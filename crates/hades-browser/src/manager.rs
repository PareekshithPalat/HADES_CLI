use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use hades_events::EventBus;

use crate::detector::BrowserDetector;
use crate::error::BrowserError;
use crate::extraction::ContentExtractor;
use crate::retrieval::{FetchResult, RetrievalEngine};
use crate::router::{WebCapabilityAction, WebCapabilityRouter};
use crate::session::BrowserSession;
use crate::types::{
    BrowserMode, BrowserStatus, ConsoleEntry, NavigationWait, NetworkEntry, PageSnapshot,
    PdfArtifact, ScreenshotArtifact, TabInfo, WebSearchResult,
};

/// High-level coordinator for the browser automation, web search, and headless sidecar subsystem.
pub struct BrowserManager {
    pub workspace_root: PathBuf,
    session: Arc<Mutex<Option<BrowserSession>>>,
    event_bus: Option<EventBus>,
}

impl BrowserManager {
    /// Creates a new `BrowserManager` tied to a workspace root directory.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            session: Arc::new(Mutex::new(None)),
            event_bus: None,
        }
    }

    /// Attaches an event bus for operational event dispatching.
    pub fn with_event_bus(mut self, event_bus: EventBus) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Fast search query without launching a browser.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<WebSearchResult>, BrowserError> {
        RetrievalEngine::search(query, limit).await
    }

    /// Direct HTTP page fetch and Markdown conversion without launching a browser.
    pub async fn fetch(&self, url: &str) -> Result<FetchResult, BrowserError> {
        RetrievalEngine::fetch(url).await
    }

    /// Evaluates user objective and routes to the optimal capability level.
    pub async fn route_capability(&self, objective: &str) -> WebCapabilityAction {
        let guard = self.session.lock().await;
        let has_active = guard.is_some();
        WebCapabilityRouter::route(objective, has_active)
    }

    /// Ensures an active browser session exists, starting one if necessary.
    pub async fn get_or_start_session(
        &self,
        session_id: &str,
        mode: BrowserMode,
        explicit_path: Option<&str>,
        preference: &str,
        custom_port: Option<u16>,
        headless: bool,
    ) -> Result<(), BrowserError> {
        let mut guard = self.session.lock().await;
        if guard.is_none() {
            info!(session_id, mode = ?mode, "Starting Hades browser session");
            let s = BrowserSession::start(
                session_id,
                mode,
                explicit_path,
                preference,
                custom_port,
                headless,
                &self.workspace_root,
            )
            .await?;
            *guard = Some(s);
        }
        Ok(())
    }

    /// Navigates active browser session to the target URL.
    pub async fn navigate(
        &self,
        url: &str,
        session_id: &str,
        wait: Option<NavigationWait>,
    ) -> Result<PageSnapshot, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        session.navigate(url, wait).await
    }

    /// Captures a fresh accessibility snapshot of the current page.
    pub async fn snapshot(&self, session_id: &str) -> Result<PageSnapshot, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        session.snapshot().await
    }

    /// Clicks on an element by reference ID (`ref_001`) or fallback selector.
    pub async fn click(&self, target: &str, session_id: &str) -> Result<String, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        session.click(target).await
    }

    /// Fills input with text.
    pub async fn fill(
        &self,
        target: &str,
        text: &str,
        session_id: &str,
    ) -> Result<String, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        session.fill(target, text).await
    }

    /// Selects option from dropdown.
    pub async fn select_option(
        &self,
        target: &str,
        value: &str,
        session_id: &str,
    ) -> Result<String, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        session.select_option(target, value).await
    }

    /// Scrolls the viewport.
    pub async fn scroll(
        &self,
        direction: &str,
        amount_pixels: i32,
        session_id: &str,
    ) -> Result<String, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        session.scroll(direction, amount_pixels).await
    }

    /// Hovers over an element by reference or selector.
    pub async fn hover(&self, target: &str, session_id: &str) -> Result<String, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        let client = session.active_client().await?;
        crate::interaction::InteractionEngine::hover(
            &client,
            target,
            session.latest_snapshot.as_ref(),
        )
        .await
    }

    /// Dispatches a key press.
    pub async fn press_key(&self, key: &str, session_id: &str) -> Result<String, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        let client = session.active_client().await?;
        crate::interaction::InteractionEngine::press_key(&client, key).await
    }

    /// Captures a screenshot artifact.
    pub async fn capture_screenshot(
        &self,
        full_page: bool,
        session_id: &str,
    ) -> Result<ScreenshotArtifact, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        session.capture_screenshot(full_page).await
    }

    /// Prints page to a PDF artifact.
    pub async fn print_to_pdf(&self, session_id: &str) -> Result<PdfArtifact, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        session.print_to_pdf().await
    }

    /// Extracts clean text.
    pub async fn extract_text(&self, session_id: &str) -> Result<String, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        let client = session.active_client().await?;
        ContentExtractor::extract_text(&client).await
    }

    /// Extracts clean Markdown.
    pub async fn extract_markdown(&self, session_id: &str) -> Result<String, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        let client = session.active_client().await?;
        ContentExtractor::extract_markdown(&client).await
    }

    /// Extracts links.
    pub async fn get_links(&self, session_id: &str) -> Result<Vec<(String, String)>, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        let client = session.active_client().await?;
        ContentExtractor::get_links(&client).await
    }

    /// Evaluates JavaScript within page context.
    pub async fn evaluate(&self, script: &str, session_id: &str) -> Result<String, BrowserError> {
        self.get_or_start_session(session_id, BrowserMode::Isolated, None, "auto", None, true)
            .await?;
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No active browser session".to_string())
        })?;

        session.evaluate(script).await
    }

    /// Retrieves live console logs.
    pub async fn get_console_logs(&self) -> Vec<ConsoleEntry> {
        let guard = self.session.lock().await;
        if let Some(ref s) = *guard {
            s.get_console_logs().await
        } else {
            Vec::new()
        }
    }

    /// Retrieves live network logs.
    pub async fn get_network_logs(&self) -> Vec<NetworkEntry> {
        let guard = self.session.lock().await;
        if let Some(ref s) = *guard {
            s.get_network_logs().await
        } else {
            Vec::new()
        }
    }

    /// Retrieves browser automation status.
    pub fn status(&self) -> BrowserStatus {
        if let Ok(guard) = self.session.try_lock() {
            if let Some(ref s) = *guard {
                return s.status();
            }
        }

        let detected = BrowserDetector::detect_all();
        let (name, ver, path) = if let Some(first) = detected.first() {
            (
                first.name.clone(),
                first.version.clone(),
                Some(first.binary_path.clone()),
            )
        } else {
            ("None detected".to_string(), "0.0".to_string(), None)
        };

        BrowserStatus {
            is_running: false,
            mode: BrowserMode::Isolated,
            browser_name: name,
            version: ver,
            binary_path: path,
            active_tabs: 0,
            active_session_id: None,
            cdp_port: None,
            uptime_secs: 0,
        }
    }

    /// Lists open tabs.
    pub async fn list_tabs(&self) -> Result<Vec<TabInfo>, BrowserError> {
        let mut guard = self.session.lock().await;
        if let Some(ref mut s) = *guard {
            s.sync_tabs().await
        } else {
            Ok(Vec::new())
        }
    }

    /// Closes the active browser session.
    pub async fn close_session(&self) -> Result<(), BrowserError> {
        let mut guard = self.session.lock().await;
        if let Some(mut s) = guard.take() {
            info!("Closing active browser session");
            s.close().await?;
        }
        Ok(())
    }
}
