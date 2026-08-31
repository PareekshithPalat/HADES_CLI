use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

use crate::cdp::{CdpClient, CdpHttpManager};
use crate::detector::BrowserDetector;
use crate::error::BrowserError;
use crate::extraction::ContentExtractor;
use crate::interaction::{ActionLoopDetector, InteractionEngine};
use crate::process::BrowserProcess;
use crate::snapshot::SnapshotEngine;
use crate::types::{
    BrowserInfo, BrowserMode, BrowserStatus, ConsoleEntry, NavigationWait, NetworkEntry,
    PageSnapshot, PdfArtifact, ScreenshotArtifact, TabInfo,
};

/// Active browser automation session coordinating processes, tabs, and snapshots.
pub struct BrowserSession {
    pub session_id: String,
    pub mode: BrowserMode,
    pub browser_info: BrowserInfo,
    pub process: Option<BrowserProcess>,
    pub http_manager: CdpHttpManager,
    pub active_tab_id: Option<String>,
    pub cdp_clients: HashMap<String, Arc<CdpClient>>,
    pub latest_snapshot: Option<PageSnapshot>,
    pub snapshot_generation: u64,
    pub action_detector: ActionLoopDetector,
    pub visited_urls: Vec<String>,
    pub started_at: chrono::DateTime<Utc>,
    pub artifacts_dir: PathBuf,
}

impl BrowserSession {
    /// Initializes and starts a new browser session.
    pub async fn start(
        session_id: &str,
        mode: BrowserMode,
        explicit_path: Option<&str>,
        preference: &str,
        custom_port: Option<u16>,
        headless: bool,
        workspace_root: &Path,
    ) -> Result<Self, BrowserError> {
        let browser_info = BrowserDetector::select_browser(explicit_path, preference)?;

        let process =
            BrowserProcess::spawn(&browser_info, mode, session_id, custom_port, headless).await?;

        let port = process.cdp_port;
        let http_manager = CdpHttpManager::new(port);

        let artifacts_dir = workspace_root
            .join(".hades")
            .join("artifacts")
            .join(session_id);
        let _ = tokio::fs::create_dir_all(&artifacts_dir).await;

        let mut session = Self {
            session_id: session_id.to_string(),
            mode,
            browser_info,
            process: Some(process),
            http_manager,
            active_tab_id: None,
            cdp_clients: HashMap::new(),
            latest_snapshot: None,
            snapshot_generation: 0,
            action_detector: ActionLoopDetector::default(),
            visited_urls: Vec::new(),
            started_at: Utc::now(),
            artifacts_dir,
        };

        // Connect to primary default tab
        session.sync_tabs().await?;

        Ok(session)
    }

    /// Synchronizes open browser tabs and attaches CDP WebSocket to the active tab.
    pub async fn sync_tabs(&mut self) -> Result<Vec<TabInfo>, BrowserError> {
        let tabs = self.http_manager.list_tabs().await?;
        if tabs.is_empty() {
            // Open a blank tab if none exist
            let new_tab = self.http_manager.create_tab(Some("about:blank")).await?;
            self.active_tab_id = Some(new_tab.tab_id.clone());
            if let Some(ref ws) = new_tab.websocket_url {
                let client = CdpClient::connect(&new_tab.target_id, ws).await?;
                self.cdp_clients
                    .insert(new_tab.tab_id.clone(), Arc::new(client));
            }
            return Ok(vec![new_tab]);
        }

        if self.active_tab_id.is_none() {
            if let Some(first) = tabs.first() {
                self.active_tab_id = Some(first.tab_id.clone());
                if let Some(ref ws) = first.websocket_url {
                    if !self.cdp_clients.contains_key(&first.tab_id) {
                        let client = CdpClient::connect(&first.target_id, ws).await?;
                        self.cdp_clients
                            .insert(first.tab_id.clone(), Arc::new(client));
                    }
                }
            }
        }

        Ok(tabs)
    }

    /// Returns the active CDP client.
    pub async fn active_client(&mut self) -> Result<Arc<CdpClient>, BrowserError> {
        self.sync_tabs().await?;
        let tab_id = self.active_tab_id.as_deref().unwrap_or("tab_1");

        if let Some(client) = self.cdp_clients.get(tab_id) {
            return Ok(client.clone());
        }

        let tabs = self.http_manager.list_tabs().await?;
        if let Some(tab) = tabs.iter().find(|t| t.tab_id == tab_id) {
            if let Some(ref ws) = tab.websocket_url {
                let client = Arc::new(CdpClient::connect(&tab.target_id, ws).await?);
                self.cdp_clients.insert(tab.tab_id.clone(), client.clone());
                return Ok(client);
            }
        }

        Err(BrowserError::InvalidBrowserState(format!(
            "No active WebSocket connection for tab '{tab_id}'"
        )))
    }

    /// Navigates the active tab to the specified URL and captures an updated accessibility snapshot.
    pub async fn navigate(
        &mut self,
        url: &str,
        wait_strategy: Option<NavigationWait>,
    ) -> Result<PageSnapshot, BrowserError> {
        let trimmed_url = url.trim();
        let target_url = if trimmed_url.starts_with("http://")
            || trimmed_url.starts_with("https://")
            || trimmed_url.starts_with("about:")
            || trimmed_url.starts_with("file://")
        {
            trimmed_url.to_string()
        } else {
            format!("https://{trimmed_url}")
        };

        let client = self.active_client().await?;

        info!(url = %target_url, "Navigating browser tab");
        let res = client
            .call(
                "Page.navigate",
                json!({
                    "url": target_url
                }),
            )
            .await?;

        if let Some(err_text) = res.get("errorText").and_then(|v| v.as_str()) {
            return Err(BrowserError::BrowserNavigationFailed {
                url: target_url,
                reason: err_text.to_string(),
            });
        }

        // Wait strategy
        match wait_strategy.unwrap_or(NavigationWait::Load) {
            NavigationWait::DomContentLoaded | NavigationWait::Load => {
                tokio::time::sleep(Duration::from_millis(600)).await;
            }
            NavigationWait::NetworkIdle => {
                tokio::time::sleep(Duration::from_millis(1200)).await;
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        self.snapshot_generation += 1;
        let tab_id = self
            .active_tab_id
            .clone()
            .unwrap_or_else(|| "tab_1".to_string());

        let snapshot =
            SnapshotEngine::capture_snapshot(&client, &tab_id, self.snapshot_generation).await?;
        self.visited_urls.push(snapshot.url.clone());
        self.latest_snapshot = Some(snapshot.clone());

        Ok(snapshot)
    }

    /// Captures a fresh snapshot of the active page without navigating.
    pub async fn snapshot(&mut self) -> Result<PageSnapshot, BrowserError> {
        let client = self.active_client().await?;
        self.snapshot_generation += 1;
        let tab_id = self
            .active_tab_id
            .clone()
            .unwrap_or_else(|| "tab_1".to_string());

        let snapshot =
            SnapshotEngine::capture_snapshot(&client, &tab_id, self.snapshot_generation).await?;
        self.latest_snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }

    /// Performs an interactive click with loop protection.
    pub async fn click(&mut self, target: &str) -> Result<String, BrowserError> {
        self.action_detector.record_action("click", target)?;
        let client = self.active_client().await?;
        let res = InteractionEngine::click(&client, target, self.latest_snapshot.as_ref()).await?;
        tokio::time::sleep(Duration::from_millis(400)).await;
        Ok(res)
    }

    /// Performs an interactive text fill with loop protection.
    pub async fn fill(&mut self, target: &str, text: &str) -> Result<String, BrowserError> {
        self.action_detector.record_action("fill", target)?;
        let client = self.active_client().await?;
        let res =
            InteractionEngine::fill(&client, target, text, self.latest_snapshot.as_ref()).await?;
        Ok(res)
    }

    /// Selects an option from a dropdown.
    pub async fn select_option(
        &mut self,
        target: &str,
        option_value: &str,
    ) -> Result<String, BrowserError> {
        self.action_detector.record_action("select", target)?;
        let client = self.active_client().await?;
        InteractionEngine::select_option(
            &client,
            target,
            option_value,
            self.latest_snapshot.as_ref(),
        )
        .await
    }

    /// Scrolls the viewport.
    pub async fn scroll(
        &mut self,
        direction: &str,
        amount_pixels: i32,
    ) -> Result<String, BrowserError> {
        let client = self.active_client().await?;
        InteractionEngine::scroll(&client, direction, amount_pixels).await
    }

    /// Captures a screenshot and persists it as an artifact.
    pub async fn capture_screenshot(
        &mut self,
        full_page: bool,
    ) -> Result<ScreenshotArtifact, BrowserError> {
        let client = self.active_client().await?;
        let res = client
            .call(
                "Page.captureScreenshot",
                json!({
                    "format": "png",
                    "captureBeyondViewport": full_page
                }),
            )
            .await?;

        let b64_data = res["data"].as_str().ok_or_else(|| {
            BrowserError::InvalidBrowserState(
                "No image data returned from CDP screenshot".to_string(),
            )
        })?;

        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64_data)
            .map_err(|e| {
                BrowserError::InvalidBrowserState(format!(
                    "Failed to decode base64 screenshot: {e}"
                ))
            })?;

        let id = format!("screenshot_{}", Utc::now().timestamp_millis());
        let screenshots_dir = self.artifacts_dir.join("screenshots");
        let _ = tokio::fs::create_dir_all(&screenshots_dir).await;
        let path = screenshots_dir.join(format!("{id}.png"));

        tokio::fs::write(&path, &bytes).await?;

        Ok(ScreenshotArtifact {
            id,
            path,
            format: "png".to_string(),
            width: 1280,
            height: 800,
            timestamp: Utc::now(),
        })
    }

    /// Renders page to a PDF artifact.
    pub async fn print_to_pdf(&mut self) -> Result<PdfArtifact, BrowserError> {
        let client = self.active_client().await?;
        let res = client
            .call(
                "Page.printToPDF",
                json!({
                    "printBackground": true,
                    "paperWidth": 8.5,
                    "paperHeight": 11.0
                }),
            )
            .await?;

        let b64_data = res["data"].as_str().ok_or_else(|| {
            BrowserError::InvalidBrowserState("No PDF data returned from CDP".to_string())
        })?;

        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64_data)
            .map_err(|e| {
                BrowserError::InvalidBrowserState(format!("Failed to decode base64 PDF: {e}"))
            })?;

        let id = format!("pdf_{}", Utc::now().timestamp_millis());
        let pdf_dir = self.artifacts_dir.join("pdf");
        let _ = tokio::fs::create_dir_all(&pdf_dir).await;
        let path = pdf_dir.join(format!("{id}.pdf"));

        let size = bytes.len() as u64;
        tokio::fs::write(&path, &bytes).await?;

        Ok(PdfArtifact {
            id,
            path,
            size_bytes: size,
            timestamp: Utc::now(),
        })
    }

    /// Retrieves live status summary.
    pub fn status(&self) -> BrowserStatus {
        let port = self.process.as_ref().map(|p| p.cdp_port);
        let is_running = self
            .process
            .as_ref()
            .map(|p| p.is_running.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false);

        BrowserStatus {
            is_running,
            mode: self.mode,
            browser_name: self.browser_info.name.clone(),
            version: self.browser_info.version.clone(),
            binary_path: Some(self.browser_info.binary_path.clone()),
            active_tabs: self.cdp_clients.len().max(1),
            active_session_id: Some(self.session_id.clone()),
            cdp_port: port,
            uptime_secs: Utc::now()
                .signed_duration_since(self.started_at)
                .num_seconds()
                .max(0) as u64,
        }
    }

    /// Evaluates arbitrary JavaScript inside the page context.
    pub async fn evaluate(&mut self, script: &str) -> Result<String, BrowserError> {
        let client = self.active_client().await?;
        let res = client
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": script,
                    "returnByValue": true,
                    "awaitPromise": true
                }),
            )
            .await?;

        let val = &res["result"]["value"];
        let s = if val.is_string() {
            val.as_str().unwrap_or("").to_string()
        } else {
            serde_json::to_string_pretty(val).unwrap_or_default()
        };

        Ok(ContentExtractor::bound_output(&s, 5_000))
    }

    /// Retrieves captured console logs.
    pub async fn get_console_logs(&self) -> Vec<ConsoleEntry> {
        if let Some(ref tab_id) = self.active_tab_id {
            if let Some(client) = self.cdp_clients.get(tab_id) {
                return client.console_logs.read().await.clone();
            }
        }
        Vec::new()
    }

    /// Retrieves captured network logs.
    pub async fn get_network_logs(&self) -> Vec<NetworkEntry> {
        if let Some(ref tab_id) = self.active_tab_id {
            if let Some(client) = self.cdp_clients.get(tab_id) {
                return client.network_logs.read().await.clone();
            }
        }
        Vec::new()
    }

    /// Gracefully closes this browser session.
    pub async fn close(&mut self) -> Result<(), BrowserError> {
        if let Some(mut proc) = self.process.take() {
            proc.shutdown().await?;
        }
        self.cdp_clients.clear();
        self.active_tab_id = None;
        self.latest_snapshot = None;
        Ok(())
    }
}
