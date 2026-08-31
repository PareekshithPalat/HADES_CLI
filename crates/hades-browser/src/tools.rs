use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use hades_tools::{RiskLevel, Tool, ToolContext, ToolDefinition, ToolResult};

use crate::manager::BrowserManager;
use crate::types::{BrowserMode, NavigationWait};

/// Helper to format and register all browser & web tools into a `ToolRegistry`.
pub struct BrowserToolSet;

impl BrowserToolSet {
    /// Registers all standard browser automation and web retrieval tools into the registry.
    pub fn register_all(registry: &mut hades_tools::ToolRegistry, manager: Arc<BrowserManager>) {
        registry.register(WebSearchTool::new(manager.clone()));
        registry.register(WebFetchTool::new(manager.clone()));
        registry.register(BrowserStartTool::new(manager.clone()));
        registry.register(BrowserCloseTool::new(manager.clone()));
        registry.register(BrowserStatusTool::new(manager.clone()));
        registry.register(BrowserTabsTool::new(manager.clone()));
        registry.register(BrowserOpenTool::new(manager.clone()));
        registry.register(BrowserSnapshotTool::new(manager.clone()));
        registry.register(BrowserExtractTextTool::new(manager.clone()));
        registry.register(BrowserExtractMarkdownTool::new(manager.clone()));
        registry.register(BrowserGetLinksTool::new(manager.clone()));
        registry.register(BrowserClickTool::new(manager.clone()));
        registry.register(BrowserFillTool::new(manager.clone()));
        registry.register(BrowserSelectTool::new(manager.clone()));
        registry.register(BrowserScrollTool::new(manager.clone()));
        registry.register(BrowserHoverTool::new(manager.clone()));
        registry.register(BrowserPressKeyTool::new(manager.clone()));
        registry.register(BrowserScreenshotTool::new(manager.clone()));
        registry.register(BrowserPdfTool::new(manager.clone()));
        registry.register(BrowserConsoleTool::new(manager.clone()));
        registry.register(BrowserNetworkTool::new(manager.clone()));
        registry.register(BrowserEvaluateTool::new(manager));
    }
}

// ------------------------------------------------------------------------------------------------
// 1. WebSearchTool (web.search)
// ------------------------------------------------------------------------------------------------
pub struct WebSearchTool {
    manager: Arc<BrowserManager>,
}

impl WebSearchTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "web.search",
            "Perform a fast web search query and return structured source results with titles, URLs, and snippets without launching a browser.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query keywords" },
                    "limit": { "type": "integer", "description": "Maximum number of search results to return (default: 5, max: 20)" }
                },
                "required": ["query"]
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, _context: &ToolContext) -> ToolResult {
        let query = match input["query"].as_str() {
            Some(q) => q,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "web.search",
                    "Missing required 'query' parameter",
                )
            }
        };
        let limit = input["limit"].as_u64().unwrap_or(5) as usize;

        match self.manager.search(query, limit).await {
            Ok(results) => {
                let mut out = format!("Web Search Results for \"{query}\":\n\n");
                for (i, r) in results.iter().enumerate() {
                    out.push_str(&format!(
                        "{}. {}\n   URL: {}\n   {}\n\n",
                        i + 1,
                        r.title,
                        r.url,
                        r.snippet
                    ));
                }
                ToolResult::success(call_id, "web.search", out)
            }
            Err(e) => ToolResult::failure(call_id, "web.search", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 2. WebFetchTool (web.fetch)
// ------------------------------------------------------------------------------------------------
pub struct WebFetchTool {
    manager: Arc<BrowserManager>,
}

impl WebFetchTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "web.fetch",
            "Directly fetch a web page via HTTP and convert HTML into clean, structured Markdown without launching a browser.",
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Full URL to fetch (http:// or https://)" }
                },
                "required": ["url"]
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, _context: &ToolContext) -> ToolResult {
        let url = match input["url"].as_str() {
            Some(u) => u,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "web.fetch",
                    "Missing required 'url' parameter",
                )
            }
        };

        match self.manager.fetch(url).await {
            Ok(res) => {
                let out = format!(
                    "Source: {} (Status: {})\nContent-Type: {}\n\n{}",
                    res.url, res.status_code, res.content_type, res.content_markdown
                );
                ToolResult::success(call_id, "web.fetch", out)
            }
            Err(e) => ToolResult::failure(call_id, "web.fetch", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 3. BrowserStartTool (browser.start)
// ------------------------------------------------------------------------------------------------
pub struct BrowserStartTool {
    manager: Arc<BrowserManager>,
}

impl BrowserStartTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserStartTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.start",
            "Start the headless browser automation sidecar process.",
            json!({
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "enum": ["isolated", "persistent", "attach"], "description": "Browser session mode (default: isolated)" },
                    "headless": { "type": "boolean", "description": "Whether to run browser in headless mode (default: true)" }
                }
            }),
            RiskLevel::Low,
            true,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, context: &ToolContext) -> ToolResult {
        let mode_str = input["mode"].as_str().unwrap_or("isolated");
        let headless = input["headless"].as_bool().unwrap_or(true);
        let mode = match mode_str {
            "persistent" => BrowserMode::Persistent,
            "attach" => BrowserMode::Attach,
            _ => BrowserMode::Isolated,
        };

        match self
            .manager
            .get_or_start_session(&context.session_id, mode, None, "auto", None, headless)
            .await
        {
            Ok(_) => {
                let status = self.manager.status();
                ToolResult::success(
                    call_id,
                    "browser.start",
                    format!(
                        "Browser sidecar started successfully (Engine: {}, Version: {}, Mode: {})",
                        status.browser_name, status.version, status.mode
                    ),
                )
            }
            Err(e) => ToolResult::failure(call_id, "browser.start", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 4. BrowserCloseTool (browser.close)
// ------------------------------------------------------------------------------------------------
pub struct BrowserCloseTool {
    manager: Arc<BrowserManager>,
}

impl BrowserCloseTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserCloseTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.close",
            "Close the active browser session and release sidecar resources.",
            json!({ "type": "object" }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, _input: Value, _context: &ToolContext) -> ToolResult {
        match self.manager.close_session().await {
            Ok(_) => ToolResult::success(
                call_id,
                "browser.close",
                "Browser session closed successfully.",
            ),
            Err(e) => ToolResult::failure(call_id, "browser.close", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 5. BrowserStatusTool (browser.status)
// ------------------------------------------------------------------------------------------------
pub struct BrowserStatusTool {
    manager: Arc<BrowserManager>,
}

impl BrowserStatusTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserStatusTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.status",
            "Retrieve current runtime state, detected browser binary, and active tabs of the browser sidecar.",
            json!({ "type": "object" }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, _input: Value, _context: &ToolContext) -> ToolResult {
        let status = self.manager.status();
        let out = format!(
            "Browser Status:\n- Running: {}\n- Engine: {}\n- Version: {}\n- Mode: {}\n- Active Tabs: {}\n- CDP Port: {:?}",
            status.is_running, status.browser_name, status.version, status.mode, status.active_tabs, status.cdp_port
        );
        ToolResult::success(call_id, "browser.status", out)
    }
}

// ------------------------------------------------------------------------------------------------
// 6. BrowserTabsTool (browser.tabs)
// ------------------------------------------------------------------------------------------------
pub struct BrowserTabsTool {
    manager: Arc<BrowserManager>,
}

impl BrowserTabsTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserTabsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.tabs",
            "List all open tabs in the active browser session.",
            json!({ "type": "object" }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, _input: Value, _context: &ToolContext) -> ToolResult {
        match self.manager.list_tabs().await {
            Ok(tabs) => {
                let mut out = format!("Open Browser Tabs ({}):\n\n", tabs.len());
                for t in tabs {
                    out.push_str(&format!("- [{}] \"{}\" ({})\n", t.tab_id, t.title, t.url));
                }
                ToolResult::success(call_id, "browser.tabs", out)
            }
            Err(e) => ToolResult::failure(call_id, "browser.tabs", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 7. BrowserOpenTool (browser.open)
// ------------------------------------------------------------------------------------------------
pub struct BrowserOpenTool {
    manager: Arc<BrowserManager>,
}

impl BrowserOpenTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserOpenTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.open",
            "Navigate the browser to a URL and return a structured accessibility snapshot with element references.",
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Target website URL to open" }
                },
                "required": ["url"]
            }),
            RiskLevel::Low,
            false,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, context: &ToolContext) -> ToolResult {
        let url = match input["url"].as_str() {
            Some(u) => u,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "browser.open",
                    "Missing required 'url' parameter",
                )
            }
        };

        match self
            .manager
            .navigate(url, &context.session_id, Some(NavigationWait::Load))
            .await
        {
            Ok(snapshot) => {
                let out = format!(
                    "{}\n\n{}",
                    snapshot.content_summary, snapshot.accessibility_tree_text
                );
                ToolResult::success(call_id, "browser.open", out)
            }
            Err(e) => ToolResult::failure(call_id, "browser.open", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 8. BrowserSnapshotTool (browser.snapshot)
// ------------------------------------------------------------------------------------------------
pub struct BrowserSnapshotTool {
    manager: Arc<BrowserManager>,
}

impl BrowserSnapshotTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserSnapshotTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.snapshot",
            "Capture an accessibility-first snapshot of the active page showing interactive elements and IDs.",
            json!({ "type": "object" }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, _input: Value, context: &ToolContext) -> ToolResult {
        match self.manager.snapshot(&context.session_id).await {
            Ok(snapshot) => {
                let out = format!(
                    "{}\n\n{}",
                    snapshot.content_summary, snapshot.accessibility_tree_text
                );
                ToolResult::success(call_id, "browser.snapshot", out)
            }
            Err(e) => ToolResult::failure(call_id, "browser.snapshot", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 9. BrowserExtractTextTool (browser.extract_text)
// ------------------------------------------------------------------------------------------------
pub struct BrowserExtractTextTool {
    manager: Arc<BrowserManager>,
}

impl BrowserExtractTextTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserExtractTextTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.extract_text",
            "Extract rendered text content from the active browser page.",
            json!({ "type": "object" }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, _input: Value, context: &ToolContext) -> ToolResult {
        match self.manager.extract_text(&context.session_id).await {
            Ok(text) => ToolResult::success(call_id, "browser.extract_text", text),
            Err(e) => ToolResult::failure(call_id, "browser.extract_text", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 10. BrowserExtractMarkdownTool (browser.extract_markdown)
// ------------------------------------------------------------------------------------------------
pub struct BrowserExtractMarkdownTool {
    manager: Arc<BrowserManager>,
}

impl BrowserExtractMarkdownTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserExtractMarkdownTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.extract_markdown",
            "Extract page contents as clean, structured Markdown, stripping scripts, styles, and boilerplate.",
            json!({ "type": "object" }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, _input: Value, context: &ToolContext) -> ToolResult {
        match self.manager.extract_markdown(&context.session_id).await {
            Ok(md) => ToolResult::success(call_id, "browser.extract_markdown", md),
            Err(e) => ToolResult::failure(call_id, "browser.extract_markdown", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 11. BrowserGetLinksTool (browser.get_links)
// ------------------------------------------------------------------------------------------------
pub struct BrowserGetLinksTool {
    manager: Arc<BrowserManager>,
}

impl BrowserGetLinksTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserGetLinksTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.get_links",
            "List all outbound hyperlinks on the active browser page.",
            json!({ "type": "object" }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, _input: Value, context: &ToolContext) -> ToolResult {
        match self.manager.get_links(&context.session_id).await {
            Ok(links) => {
                let mut out = format!("Extracted Page Links ({}):\n\n", links.len());
                for (text, url) in links {
                    out.push_str(&format!("- [{}]({})\n", text, url));
                }
                ToolResult::success(call_id, "browser.get_links", out)
            }
            Err(e) => ToolResult::failure(call_id, "browser.get_links", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 12. BrowserClickTool (browser.click)
// ------------------------------------------------------------------------------------------------
pub struct BrowserClickTool {
    manager: Arc<BrowserManager>,
}

impl BrowserClickTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserClickTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.click",
            "Click on an interactive element by reference ID (e.g. 'ref_001') or fallback CSS selector.",
            json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Element reference (e.g. 'ref_001') or CSS selector" }
                },
                "required": ["target"]
            }),
            RiskLevel::Medium,
            true,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, context: &ToolContext) -> ToolResult {
        let target = match input["target"].as_str() {
            Some(t) => t,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "browser.click",
                    "Missing required 'target' parameter",
                )
            }
        };

        match self.manager.click(target, &context.session_id).await {
            Ok(msg) => ToolResult::success(call_id, "browser.click", msg),
            Err(e) => ToolResult::failure(call_id, "browser.click", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 13. BrowserFillTool (browser.fill)
// ------------------------------------------------------------------------------------------------
pub struct BrowserFillTool {
    manager: Arc<BrowserManager>,
}

impl BrowserFillTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserFillTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.fill",
            "Fill a form input or textarea with text using an element reference (e.g. 'ref_002').",
            json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Element reference (e.g. 'ref_002') or CSS selector" },
                    "text": { "type": "string", "description": "Text content to insert into the input field" }
                },
                "required": ["target", "text"]
            }),
            RiskLevel::Medium,
            true,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, context: &ToolContext) -> ToolResult {
        let target = match input["target"].as_str() {
            Some(t) => t,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "browser.fill",
                    "Missing required 'target' parameter",
                )
            }
        };
        let text = match input["text"].as_str() {
            Some(t) => t,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "browser.fill",
                    "Missing required 'text' parameter",
                )
            }
        };

        match self.manager.fill(target, text, &context.session_id).await {
            Ok(msg) => ToolResult::success(call_id, "browser.fill", msg),
            Err(e) => ToolResult::failure(call_id, "browser.fill", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 14. BrowserSelectTool (browser.select)
// ------------------------------------------------------------------------------------------------
pub struct BrowserSelectTool {
    manager: Arc<BrowserManager>,
}

impl BrowserSelectTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserSelectTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.select",
            "Select an option in a <select> dropdown.",
            json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Element reference or CSS selector" },
                    "value": { "type": "string", "description": "Option value to select" }
                },
                "required": ["target", "value"]
            }),
            RiskLevel::Medium,
            true,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, context: &ToolContext) -> ToolResult {
        let target = match input["target"].as_str() {
            Some(t) => t,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "browser.select",
                    "Missing required 'target' parameter",
                )
            }
        };
        let val = match input["value"].as_str() {
            Some(v) => v,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "browser.select",
                    "Missing required 'value' parameter",
                )
            }
        };

        match self
            .manager
            .select_option(target, val, &context.session_id)
            .await
        {
            Ok(msg) => ToolResult::success(call_id, "browser.select", msg),
            Err(e) => ToolResult::failure(call_id, "browser.select", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 15. BrowserScrollTool (browser.scroll)
// ------------------------------------------------------------------------------------------------
pub struct BrowserScrollTool {
    manager: Arc<BrowserManager>,
}

impl BrowserScrollTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserScrollTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.scroll",
            "Scroll the active page viewport up or down.",
            json!({
                "type": "object",
                "properties": {
                    "direction": { "type": "string", "enum": ["up", "down", "top", "bottom"], "description": "Scroll direction (default: down)" },
                    "amount": { "type": "integer", "description": "Pixel amount to scroll (default: 500)" }
                }
            }),
            RiskLevel::Low,
            false,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, context: &ToolContext) -> ToolResult {
        let dir = input["direction"].as_str().unwrap_or("down");
        let amount = input["amount"].as_i64().unwrap_or(500) as i32;

        match self.manager.scroll(dir, amount, &context.session_id).await {
            Ok(msg) => ToolResult::success(call_id, "browser.scroll", msg),
            Err(e) => ToolResult::failure(call_id, "browser.scroll", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 16. BrowserHoverTool (browser.hover)
// ------------------------------------------------------------------------------------------------
pub struct BrowserHoverTool {
    manager: Arc<BrowserManager>,
}

impl BrowserHoverTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserHoverTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.hover",
            "Hover mouse over an element by reference ID or CSS selector.",
            json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Element reference or CSS selector" }
                },
                "required": ["target"]
            }),
            RiskLevel::Low,
            false,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, context: &ToolContext) -> ToolResult {
        let target = match input["target"].as_str() {
            Some(t) => t,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "browser.hover",
                    "Missing required 'target' parameter",
                )
            }
        };

        match self.manager.hover(target, &context.session_id).await {
            Ok(msg) => ToolResult::success(call_id, "browser.hover", msg),
            Err(e) => ToolResult::failure(call_id, "browser.hover", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 17. BrowserPressKeyTool (browser.press_key)
// ------------------------------------------------------------------------------------------------
pub struct BrowserPressKeyTool {
    manager: Arc<BrowserManager>,
}

impl BrowserPressKeyTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserPressKeyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.press_key",
            "Dispatch a keyboard key press to the active page (e.g. 'Enter', 'Tab', 'Escape').",
            json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Key identifier to press" }
                },
                "required": ["key"]
            }),
            RiskLevel::Medium,
            true,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, context: &ToolContext) -> ToolResult {
        let key = match input["key"].as_str() {
            Some(k) => k,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "browser.press_key",
                    "Missing required 'key' parameter",
                )
            }
        };

        match self.manager.press_key(key, &context.session_id).await {
            Ok(msg) => ToolResult::success(call_id, "browser.press_key", msg),
            Err(e) => ToolResult::failure(call_id, "browser.press_key", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 18. BrowserScreenshotTool (browser.screenshot)
// ------------------------------------------------------------------------------------------------
pub struct BrowserScreenshotTool {
    manager: Arc<BrowserManager>,
}

impl BrowserScreenshotTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserScreenshotTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.screenshot",
            "Capture a screenshot of the active browser page and save it as a session artifact.",
            json!({
                "type": "object",
                "properties": {
                    "full_page": { "type": "boolean", "description": "Whether to capture full scrollable page or visible viewport (default: false)" }
                }
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, context: &ToolContext) -> ToolResult {
        let full_page = input["full_page"].as_bool().unwrap_or(false);
        match self
            .manager
            .capture_screenshot(full_page, &context.session_id)
            .await
        {
            Ok(artifact) => {
                let out = format!(
                    "Screenshot captured successfully:\n- Artifact ID: {}\n- Path: {}\n- Dimensions: {}x{}",
                    artifact.id, artifact.path.display(), artifact.width, artifact.height
                );
                ToolResult::success(call_id, "browser.screenshot", out).with_artifact(artifact.id)
            }
            Err(e) => ToolResult::failure(call_id, "browser.screenshot", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 19. BrowserPdfTool (browser.pdf)
// ------------------------------------------------------------------------------------------------
pub struct BrowserPdfTool {
    manager: Arc<BrowserManager>,
}

impl BrowserPdfTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserPdfTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.pdf",
            "Render the active browser page to a PDF document artifact.",
            json!({ "type": "object" }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, _input: Value, context: &ToolContext) -> ToolResult {
        match self.manager.print_to_pdf(&context.session_id).await {
            Ok(artifact) => {
                let out =
                    format!(
                    "PDF rendered successfully:\n- Artifact ID: {}\n- Path: {}\n- Size: {} bytes",
                    artifact.id, artifact.path.display(), artifact.size_bytes
                );
                ToolResult::success(call_id, "browser.pdf", out).with_artifact(artifact.id)
            }
            Err(e) => ToolResult::failure(call_id, "browser.pdf", e.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 20. BrowserConsoleTool (browser.console)
// ------------------------------------------------------------------------------------------------
pub struct BrowserConsoleTool {
    manager: Arc<BrowserManager>,
}

impl BrowserConsoleTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserConsoleTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.console",
            "Retrieve live JavaScript console logs and error messages from the active browser tab.",
            json!({ "type": "object" }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, _input: Value, _context: &ToolContext) -> ToolResult {
        let logs = self.manager.get_console_logs().await;
        if logs.is_empty() {
            return ToolResult::success(
                call_id,
                "browser.console",
                "No console logs recorded on active tab.",
            );
        }

        let mut out = format!("Browser Console Logs ({}):\n\n", logs.len());
        for entry in logs {
            out.push_str(&format!(
                "[{}] [{}] {}\n",
                entry.timestamp.format("%H:%M:%S"),
                entry.level.to_uppercase(),
                entry.text
            ));
        }
        ToolResult::success(call_id, "browser.console", out)
    }
}

// ------------------------------------------------------------------------------------------------
// 21. BrowserNetworkTool (browser.network)
// ------------------------------------------------------------------------------------------------
pub struct BrowserNetworkTool {
    manager: Arc<BrowserManager>,
}

impl BrowserNetworkTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserNetworkTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.network",
            "Inspect captured HTTP/HTTPS network requests, responses, and failures from the active page.",
            json!({ "type": "object" }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(&self, call_id: &str, _input: Value, _context: &ToolContext) -> ToolResult {
        let logs = self.manager.get_network_logs().await;
        if logs.is_empty() {
            return ToolResult::success(
                call_id,
                "browser.network",
                "No network requests recorded on active tab.",
            );
        }

        let mut out = format!("Browser Network Requests ({}):\n\n", logs.len());
        for entry in logs {
            let status_str = entry
                .status
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Pending".to_string());
            let err_str = entry
                .error
                .map(|e| format!(" (Error: {e})"))
                .unwrap_or_default();
            out.push_str(&format!(
                "- [{}] {} -> {}{}\n",
                entry.method, entry.url, status_str, err_str
            ));
        }
        ToolResult::success(call_id, "browser.network", out)
    }
}

// ------------------------------------------------------------------------------------------------
// 22. BrowserEvaluateTool (browser.evaluate)
// ------------------------------------------------------------------------------------------------
pub struct BrowserEvaluateTool {
    manager: Arc<BrowserManager>,
}

impl BrowserEvaluateTool {
    pub fn new(manager: Arc<BrowserManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserEvaluateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "browser.evaluate",
            "Evaluate sandboxed JavaScript within the active web page context.",
            json!({
                "type": "object",
                "properties": {
                    "script": { "type": "string", "description": "JavaScript code to evaluate in page context" }
                },
                "required": ["script"]
            }),
            RiskLevel::High,
            true,
        )
    }

    async fn execute(&self, call_id: &str, input: Value, context: &ToolContext) -> ToolResult {
        let script = match input["script"].as_str() {
            Some(s) => s,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "browser.evaluate",
                    "Missing required 'script' parameter",
                )
            }
        };

        match self.manager.evaluate(script, &context.session_id).await {
            Ok(output) => ToolResult::success(call_id, "browser.evaluate", output),
            Err(e) => ToolResult::failure(call_id, "browser.evaluate", e.to_string()),
        }
    }
}
