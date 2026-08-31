use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message;
use tracing::debug;

use crate::error::BrowserError;
use crate::types::{ConsoleEntry, NetworkEntry, TabInfo};

/// A JSON-RPC CDP request sent over WebSocket.
#[derive(Debug, Serialize)]
struct CdpRequest {
    id: i64,
    method: String,
    params: Value,
}

/// A JSON-RPC CDP response received over WebSocket.
#[derive(Debug, Deserialize)]
struct CdpResponse {
    id: Option<i64>,
    result: Option<Value>,
    error: Option<CdpErrorPayload>,
    method: Option<String>,
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CdpErrorPayload {
    code: i64,
    message: String,
}

type PendingMap = Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, BrowserError>>>>>;

/// Active WebSocket session client connected to a specific browser target/tab.
pub struct CdpClient {
    pub target_id: String,
    pub websocket_url: String,
    next_id: AtomicI64,
    tx_command: mpsc::UnboundedSender<CdpRequest>,
    pending_responses: PendingMap,
    pub console_logs: Arc<RwLock<Vec<ConsoleEntry>>>,
    pub network_logs: Arc<RwLock<Vec<NetworkEntry>>>,
}

impl CdpClient {
    /// Connects to a target WebSocket URL and starts the duplex message pump.
    pub async fn connect(target_id: &str, websocket_url: &str) -> Result<Self, BrowserError> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(websocket_url)
            .await
            .map_err(|e| BrowserError::BrowserConnectionFailed {
                endpoint: websocket_url.to_string(),
                details: e.to_string(),
            })?;

        let (mut write_half, mut read_half) = ws_stream.split();
        let (tx_command, mut rx_command) = mpsc::unbounded_channel::<CdpRequest>();
        let pending = Arc::new(Mutex::new(HashMap::<
            i64,
            oneshot::Sender<Result<Value, BrowserError>>,
        >::new()));
        let pending_clone = pending.clone();

        let console_logs = Arc::new(RwLock::new(Vec::new()));
        let console_clone = console_logs.clone();

        let network_logs = Arc::new(RwLock::new(Vec::new()));
        let network_clone = network_logs.clone();

        // 1. Writer task
        tokio::spawn(async move {
            while let Some(req) = rx_command.recv().await {
                if let Ok(text) = serde_json::to_string(&req) {
                    if let Err(e) = write_half.send(Message::Text(text.into())).await {
                        debug!("CDP WebSocket write error: {e}");
                        break;
                    }
                }
            }
        });

        // 2. Reader task
        tokio::spawn(async move {
            while let Some(msg_res) = read_half.next().await {
                match msg_res {
                    Ok(Message::Text(text)) => {
                        if let Ok(resp) = serde_json::from_str::<CdpResponse>(&text) {
                            if let Some(id) = resp.id {
                                let mut map = pending_clone.lock().await;
                                if let Some(sender) = map.remove(&id) {
                                    if let Some(err) = resp.error {
                                        let _ = sender.send(Err(BrowserError::CdpProtocolError {
                                            code: err.code,
                                            message: err.message,
                                        }));
                                    } else {
                                        let res = resp.result.unwrap_or(Value::Null);
                                        let _ = sender.send(Ok(res));
                                    }
                                }
                            } else if let Some(method) = resp.method {
                                // Handle event notifications
                                Self::handle_event(
                                    &method,
                                    resp.params.unwrap_or(Value::Null),
                                    &console_clone,
                                    &network_clone,
                                )
                                .await;
                            }
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(e) => {
                        debug!("CDP WebSocket read error: {e}");
                        break;
                    }
                    _ => {}
                }
            }
        });

        let client = Self {
            target_id: target_id.to_string(),
            websocket_url: websocket_url.to_string(),
            next_id: AtomicI64::new(1),
            tx_command,
            pending_responses: pending,
            console_logs,
            network_logs,
        };

        // Enable default domains
        let _ = client.call("Page.enable", json!({})).await;
        let _ = client.call("Runtime.enable", json!({})).await;
        let _ = client.call("DOM.enable", json!({})).await;
        let _ = client.call("Network.enable", json!({})).await;
        let _ = client.call("Log.enable", json!({})).await;

        Ok(client)
    }

    /// Dispatches a CDP method call and awaits response.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, BrowserError> {
        self.call_with_timeout(method, params, Duration::from_secs(30))
            .await
    }

    /// Dispatches a CDP method call with explicit timeout.
    pub async fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, BrowserError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        {
            let mut map = self.pending_responses.lock().await;
            map.insert(id, tx);
        }

        let req = CdpRequest {
            id,
            method: method.to_string(),
            params,
        };

        if let Err(e) = self.tx_command.send(req) {
            let mut map = self.pending_responses.lock().await;
            map.remove(&id);
            return Err(BrowserError::BrowserDisconnected(format!(
                "Failed to send CDP command '{method}': {e}"
            )));
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => {
                let mut map = self.pending_responses.lock().await;
                map.remove(&id);
                Err(BrowserError::BrowserDisconnected(
                    "CDP response channel dropped".to_string(),
                ))
            }
            Err(_) => {
                let mut map = self.pending_responses.lock().await;
                map.remove(&id);
                Err(BrowserError::BrowserTimeout {
                    timeout_secs: timeout.as_secs(),
                    details: format!(
                        "CDP method '{method}' timed out after {}s",
                        timeout.as_secs()
                    ),
                })
            }
        }
    }

    async fn handle_event(
        method: &str,
        params: Value,
        console_logs: &Arc<RwLock<Vec<ConsoleEntry>>>,
        network_logs: &Arc<RwLock<Vec<NetworkEntry>>>,
    ) {
        match method {
            "Runtime.consoleAPICalled" => {
                let log_type = params["type"].as_str().unwrap_or("log").to_string();
                let text = params["args"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|v| v["value"].as_str().unwrap_or("").to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default();

                let mut logs = console_logs.write().await;
                if logs.len() < 100 {
                    logs.push(ConsoleEntry {
                        level: log_type,
                        text,
                        source: "console".to_string(),
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
            "Log.entryAdded" => {
                let entry = &params["entry"];
                let level = entry["level"].as_str().unwrap_or("info").to_string();
                let text = entry["text"].as_str().unwrap_or("").to_string();
                let source = entry["source"].as_str().unwrap_or("log").to_string();

                let mut logs = console_logs.write().await;
                if logs.len() < 100 {
                    logs.push(ConsoleEntry {
                        level,
                        text,
                        source,
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
            "Network.requestWillBeSent" => {
                let req_id = params["requestId"].as_str().unwrap_or("").to_string();
                let req = &params["request"];
                let url = req["url"].as_str().unwrap_or("").to_string();
                let method = req["method"].as_str().unwrap_or("GET").to_string();

                let mut logs = network_logs.write().await;
                if logs.len() < 100 {
                    logs.push(NetworkEntry {
                        request_id: req_id,
                        url,
                        method,
                        status: None,
                        mime_type: None,
                        timing_ms: None,
                        error: None,
                    });
                }
            }
            "Network.responseReceived" => {
                let req_id = params["requestId"].as_str().unwrap_or("");
                let resp = &params["response"];
                let status = resp["status"].as_u64().map(|s| s as u16);
                let mime = resp["mimeType"].as_str().map(|m| m.to_string());

                let mut logs = network_logs.write().await;
                if let Some(entry) = logs.iter_mut().find(|e| e.request_id == req_id) {
                    entry.status = status;
                    entry.mime_type = mime;
                }
            }
            "Network.loadingFailed" => {
                let req_id = params["requestId"].as_str().unwrap_or("");
                let err_text = params["errorText"].as_str().unwrap_or("Failed").to_string();

                let mut logs = network_logs.write().await;
                if let Some(entry) = logs.iter_mut().find(|e| e.request_id == req_id) {
                    entry.error = Some(err_text);
                }
            }
            _ => {}
        }
    }
}

/// HTTP management client for DevTools discovery endpoints.
pub struct CdpHttpManager {
    port: u16,
    client: reqwest::Client,
}

impl CdpHttpManager {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Queries `http://127.0.0.1:{port}/json/list` to discover open tabs and target endpoints.
    pub async fn list_tabs(&self) -> Result<Vec<TabInfo>, BrowserError> {
        let url = format!("http://127.0.0.1:{}/json/list", self.port);
        let resp = self.client.get(&url).send().await.map_err(|e| {
            BrowserError::BrowserConnectionFailed {
                endpoint: url.clone(),
                details: e.to_string(),
            }
        })?;

        let items: Vec<Value> =
            resp.json()
                .await
                .map_err(|e| BrowserError::BrowserConnectionFailed {
                    endpoint: url,
                    details: e.to_string(),
                })?;

        let mut tabs = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            let target_type = item["type"].as_str().unwrap_or("");
            if target_type == "page" {
                let id = item["id"].as_str().unwrap_or("").to_string();
                let title = item["title"].as_str().unwrap_or("").to_string();
                let page_url = item["url"].as_str().unwrap_or("").to_string();
                let ws_url = item["webSocketDebuggerUrl"].as_str().map(|s| s.to_string());

                tabs.push(TabInfo {
                    tab_id: format!("tab_{}", i + 1),
                    target_id: id,
                    url: page_url,
                    title,
                    is_active: i == 0,
                    websocket_url: ws_url,
                });
            }
        }

        Ok(tabs)
    }

    /// Creates a new tab at `http://127.0.0.1:{port}/json/new?{url}`.
    pub async fn create_tab(&self, initial_url: Option<&str>) -> Result<TabInfo, BrowserError> {
        let url = match initial_url {
            Some(u) => format!("http://127.0.0.1:{}/json/new?{}", self.port, u),
            None => format!("http://127.0.0.1:{}/json/new", self.port),
        };

        let resp = self.client.put(&url).send().await.map_err(|e| {
            BrowserError::BrowserConnectionFailed {
                endpoint: url.clone(),
                details: e.to_string(),
            }
        })?;

        let item: Value = resp
            .json()
            .await
            .map_err(|e| BrowserError::BrowserConnectionFailed {
                endpoint: url,
                details: e.to_string(),
            })?;

        let id = item["id"].as_str().unwrap_or("").to_string();
        let title = item["title"].as_str().unwrap_or("").to_string();
        let page_url = item["url"].as_str().unwrap_or("").to_string();
        let ws_url = item["webSocketDebuggerUrl"].as_str().map(|s| s.to_string());

        Ok(TabInfo {
            tab_id: format!("tab_{}", id),
            target_id: id,
            url: page_url,
            title,
            is_active: true,
            websocket_url: ws_url,
        })
    }

    /// Closes a tab at `http://127.0.0.1:{port}/json/close/{target_id}`.
    pub async fn close_tab(&self, target_id: &str) -> Result<(), BrowserError> {
        let url = format!("http://127.0.0.1:{}/json/close/{}", self.port, target_id);
        let _ = self.client.get(&url).send().await;
        Ok(())
    }

    /// Activates a tab at `http://127.0.0.1:{port}/json/activate/{target_id}`.
    pub async fn activate_tab(&self, target_id: &str) -> Result<(), BrowserError> {
        let url = format!("http://127.0.0.1:{}/json/activate/{}", self.port, target_id);
        let _ = self.client.get(&url).send().await;
        Ok(())
    }
}
