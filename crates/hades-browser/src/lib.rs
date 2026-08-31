pub mod cdp;
pub mod detector;
pub mod error;
pub mod extraction;
pub mod interaction;
pub mod manager;
pub mod process;
pub mod retrieval;
pub mod router;
pub mod session;
pub mod snapshot;
pub mod tools;
pub mod types;

pub use detector::BrowserDetector;
pub use error::BrowserError;
pub use extraction::ContentExtractor;
pub use interaction::{ActionLoopDetector, InteractionEngine};
pub use manager::BrowserManager;
pub use process::BrowserProcess;
pub use retrieval::{FetchResult, RetrievalEngine};
pub use router::{WebCapabilityAction, WebCapabilityRouter};
pub use session::BrowserSession;
pub use snapshot::SnapshotEngine;
pub use tools::BrowserToolSet;
pub use types::{
    BrowserInfo, BrowserMode, BrowserStatus, BrowserType, ConsoleEntry, DownloadArtifact,
    ElementRef, NavigationWait, NetworkEntry, PageSnapshot, PdfArtifact, ScreenshotArtifact,
    TabInfo, WebSearchResult,
};

#[cfg(test)]
mod tests {
    use super::*;
    use hades_tools::{RiskLevel, ToolRegistry};
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn test_web_capability_router_hierarchy() {
        // Level 2: Search
        let r1 = WebCapabilityRouter::route("Find Tokio documentation for Rust", false);
        assert!(matches!(r1, WebCapabilityAction::Search { .. }));

        // Level 3: Fetch
        let r2 = WebCapabilityRouter::route(
            "Read https://docs.rs/tokio/latest/tokio/ and summarize it",
            false,
        );
        assert!(matches!(r2, WebCapabilityAction::Fetch { .. }));

        // Level 4: Browser open / test
        let r3 = WebCapabilityRouter::route(
            "Open in browser http://localhost:3000 and check homepage",
            false,
        );
        assert!(matches!(r3, WebCapabilityAction::Navigate { .. }));

        // Level 5: Interaction
        let r4 = WebCapabilityRouter::route("Click login button and fill username", true);
        assert!(matches!(r4, WebCapabilityAction::Interact { .. }));

        // Level 6: Diagnostics
        let r5 = WebCapabilityRouter::route(
            "Why is this javascript network request failing with 500 error",
            true,
        );
        assert!(matches!(r5, WebCapabilityAction::Diagnose { .. }));
    }

    #[test]
    fn test_action_loop_detector_triggers_on_repetition() {
        let mut detector = ActionLoopDetector::new(10);
        assert!(detector.record_action("click", "ref_001").is_ok());
        assert!(detector.record_action("click", "ref_001").is_ok());
        assert!(detector.record_action("click", "ref_001").is_ok());
        // 4th identical action triggers BrowserLoopDetected
        let res = detector.record_action("click", "ref_001");
        assert!(matches!(res, Err(BrowserError::BrowserLoopDetected { .. })));
    }

    #[test]
    fn test_html_to_markdown_cleaning() {
        let html = r#"
        <html>
            <head><style>.btn { color: red; }</style></head>
            <body>
                <script>console.log("track");</script>
                <h1>Welcome to Hades</h1>
                <p>Hades is an <strong>agentic AI</strong> CLI.</p>
                <a href="https://github.com">GitHub Repository</a>
                <ul>
                    <li>Feature 1</li>
                    <li>Feature 2</li>
                </ul>
            </body>
        </html>
        "#;

        let md = ContentExtractor::html_to_clean_markdown(html);
        assert!(md.contains("# Welcome to Hades"));
        assert!(md.contains("[GitHub Repository](https://github.com)"));
        assert!(md.contains("- Feature 1"));
        assert!(!md.contains("<script>"));
        assert!(!md.contains("<style>"));
    }

    #[tokio::test]
    async fn test_browser_tool_registration_and_definitions() {
        let tmp = tempdir().expect("tempdir");
        let manager = Arc::new(BrowserManager::new(tmp.path()));
        let mut registry = ToolRegistry::new();

        BrowserToolSet::register_all(&mut registry, manager);

        assert!(registry.contains("web.search"));
        assert!(registry.contains("web.fetch"));
        assert!(registry.contains("browser.start"));
        assert!(registry.contains("browser.close"));
        assert!(registry.contains("browser.status"));
        assert!(registry.contains("browser.tabs"));
        assert!(registry.contains("browser.open"));
        assert!(registry.contains("browser.snapshot"));
        assert!(registry.contains("browser.click"));
        assert!(registry.contains("browser.fill"));
        assert!(registry.contains("browser.scroll"));
        assert!(registry.contains("browser.screenshot"));
        assert!(registry.contains("browser.pdf"));
        assert!(registry.contains("browser.console"));
        assert!(registry.contains("browser.network"));
        assert!(registry.contains("browser.evaluate"));

        // Check definition schemas
        let open_tool = registry.get("browser.open").expect("browser.open");
        assert_eq!(open_tool.definition().risk_level, RiskLevel::Low);

        let click_tool = registry.get("browser.click").expect("browser.click");
        assert_eq!(click_tool.definition().risk_level, RiskLevel::Medium);

        let eval_tool = registry.get("browser.evaluate").expect("browser.evaluate");
        assert_eq!(eval_tool.definition().risk_level, RiskLevel::High);
    }
}
