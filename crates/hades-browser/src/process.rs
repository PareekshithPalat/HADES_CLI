use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::process::{Child, Command};
use tracing::{debug, info};

use crate::error::BrowserError;
use crate::types::{BrowserInfo, BrowserMode};

/// Manages the spawned browser sidecar process lifecycle, ports, and profile directories.
pub struct BrowserProcess {
    child: Option<Child>,
    pub process_id: Option<u32>,
    pub session_id: String,
    pub mode: BrowserMode,
    pub cdp_port: u16,
    pub profile_dir: PathBuf,
    pub is_hades_owned: bool,
    pub is_running: Arc<AtomicBool>,
    temp_dir: Option<TempDir>,
}

impl BrowserProcess {
    /// Spawns a new browser sidecar instance.
    pub async fn spawn(
        info: &BrowserInfo,
        mode: BrowserMode,
        session_id: &str,
        custom_port: Option<u16>,
        headless: bool,
    ) -> Result<Self, BrowserError> {
        let (profile_dir, temp_dir) = match mode {
            BrowserMode::Isolated => {
                let tmp = TempDir::new().map_err(|e| {
                    BrowserError::BrowserLaunchFailed(format!(
                        "Failed to create temporary browser profile directory: {e}"
                    ))
                })?;
                let path = tmp.path().to_path_buf();
                (path, Some(tmp))
            }
            BrowserMode::Persistent => {
                let home = dirs::home_dir().ok_or_else(|| {
                    BrowserError::BrowserLaunchFailed("Home directory not found".to_string())
                })?;
                let path = home.join(".hades").join("browser").join("profile");
                tokio::fs::create_dir_all(&path).await.map_err(|e| {
                    BrowserError::BrowserLaunchFailed(format!(
                        "Failed to create persistent profile directory: {e}"
                    ))
                })?;
                (path, None)
            }
            BrowserMode::Attach => {
                let port = custom_port.unwrap_or(9222);
                return Ok(Self {
                    child: None,
                    process_id: None,
                    session_id: session_id.to_string(),
                    mode,
                    cdp_port: port,
                    profile_dir: PathBuf::new(),
                    is_hades_owned: false,
                    is_running: Arc::new(AtomicBool::new(true)),
                    temp_dir: None,
                });
            }
        };

        let port = match custom_port {
            Some(p) => p,
            None => Self::find_free_port()?,
        };

        info!(
            browser = %info.name,
            port,
            profile = ?profile_dir,
            headless,
            "Spawning Hades browser sidecar process"
        );

        let mut cmd = Command::new(&info.binary_path);
        cmd.arg(format!("--remote-debugging-port={port}"));
        cmd.arg(format!("--user-data-dir={}", profile_dir.display()));

        if headless {
            cmd.arg("--headless=new");
        }

        cmd.arg("--no-first-run");
        cmd.arg("--no-default-browser-check");
        cmd.arg("--disable-background-networking");
        cmd.arg("--disable-sync");
        cmd.arg("--disable-extensions");
        cmd.arg("--disable-default-apps");
        cmd.arg("--disable-popup-blocking");
        cmd.arg("--window-size=1280,800");
        cmd.arg("--disable-gpu");
        cmd.arg("about:blank");

        // Detach stdout/stderr
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd.spawn().map_err(|e| {
            BrowserError::BrowserLaunchFailed(format!(
                "Failed to execute '{}': {e}",
                info.binary_path.display()
            ))
        })?;

        let pid = child.id();

        let process = Self {
            child: Some(child),
            process_id: pid,
            session_id: session_id.to_string(),
            mode,
            cdp_port: port,
            profile_dir,
            is_hades_owned: true,
            is_running: Arc::new(AtomicBool::new(true)),
            temp_dir,
        };

        // Wait for CDP endpoint to become healthy
        process.wait_for_cdp_ready(Duration::from_secs(15)).await?;

        Ok(process)
    }

    /// Allocates an available TCP port on localhost.
    pub fn find_free_port() -> Result<u16, BrowserError> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| {
            BrowserError::BrowserLaunchFailed(format!("Failed to bind free port: {e}"))
        })?;
        let port = listener
            .local_addr()
            .map_err(|e| {
                BrowserError::BrowserLaunchFailed(format!("Failed to retrieve local port: {e}"))
            })?
            .port();
        drop(listener);
        Ok(port)
    }

    /// Polls the CDP JSON endpoint until the browser responds or timeout occurs.
    pub async fn wait_for_cdp_ready(&self, timeout: Duration) -> Result<(), BrowserError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| BrowserError::BrowserConnectionFailed {
                endpoint: format!("http://127.0.0.1:{}", self.cdp_port),
                details: e.to_string(),
            })?;

        let url = format!("http://127.0.0.1:{}/json/version", self.cdp_port);
        let start = tokio::time::Instant::now();

        while start.elapsed() < timeout {
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    debug!(port = self.cdp_port, "CDP endpoint is healthy and ready");
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Err(BrowserError::BrowserTimeout {
            timeout_secs: timeout.as_secs(),
            details: format!(
                "Browser did not start DevTools endpoint at 'http://127.0.0.1:{}' within {}s",
                self.cdp_port,
                timeout.as_secs()
            ),
        })
    }

    /// Gracefully closes and cleans up the browser sidecar process.
    pub async fn shutdown(&mut self) -> Result<(), BrowserError> {
        if !self.is_hades_owned {
            debug!("Attached external browser will not be terminated");
            self.is_running.store(false, Ordering::SeqCst);
            return Ok(());
        }

        self.is_running.store(false, Ordering::SeqCst);

        if let Some(mut child) = self.child.take() {
            info!(pid = ?self.process_id, "Shutting down Hades browser process");
            // 1. Try graceful kill
            let _ = child.kill().await;

            // 2. Wait up to 3 seconds for process exit
            let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
        }

        // Clean up tempdir explicitly if present
        if let Some(tmp) = self.temp_dir.take() {
            let _ = tmp.close();
        }

        Ok(())
    }
}

impl Drop for BrowserProcess {
    fn drop(&mut self) {
        if self.is_hades_owned && self.child.is_some() {
            if let Some(mut child) = self.child.take() {
                let _ = child.start_kill();
            }
        }
    }
}
