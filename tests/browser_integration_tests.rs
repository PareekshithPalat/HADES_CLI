use std::sync::Arc;
use tempfile::tempdir;

use hades_agent::AgentRole;
use hades_browser::{
    ActionLoopDetector, BrowserDetector, BrowserError, BrowserManager, BrowserToolSet,
    ContentExtractor, WebCapabilityAction, WebCapabilityRouter,
};
use hades_config::{ConfigService, HadesConfig};
use hades_core::{CommandOutput, HadesApp};
use hades_events::EventBus;
use hades_storage::StorageService;
use hades_tools::{ToolContext, ToolRegistry, ToolStatus};

#[test]
fn test_browser_config_validation_and_defaults() {
    let mut config = HadesConfig::default();
    assert!(config.browser.enabled);
    assert_eq!(config.browser.mode, "isolated");
    assert_eq!(config.browser.preferred_browser, "auto");
    assert!(config.browser.headless);
    assert_eq!(config.browser.default_timeout_seconds, 30);
    assert_eq!(config.browser.max_actions_per_task, 100);
    assert_eq!(config.browser.max_tabs, 10);
    assert!(config.validate().is_ok());

    // Validation failure on zero timeout
    config.browser.default_timeout_seconds = 0;
    assert!(config.validate().is_err());
}

#[test]
fn test_browser_detector_cross_platform() {
    let detected = BrowserDetector::detect_all();
    // Verify probe runs safely without crashing regardless of system environment
    println!("Detected {} local browser installations", detected.len());

    // When explicitly requesting "auto", select_browser returns a valid result or BrowserNotFound
    let selection = BrowserDetector::select_browser(None, "auto");
    if detected.is_empty() {
        assert!(matches!(selection, Err(BrowserError::BrowserNotFound(_))));
    } else {
        assert!(selection.is_ok());
    }
}

#[test]
fn test_web_capability_router_intent_matrix() {
    // 1. General search intent (Level 2)
    let act1 = WebCapabilityRouter::route("Search for Rust Tokio tutorial", false);
    assert!(matches!(act1, WebCapabilityAction::Search { .. }));

    // 2. Direct static documentation read (Level 3)
    let act2 = WebCapabilityRouter::route(
        "Summarize the content of https://doc.rust-lang.org/book/",
        false,
    );
    assert!(matches!(act2, WebCapabilityAction::Fetch { .. }));

    // 3. Headless browser navigation / SPA inspection (Level 4)
    let act3 = WebCapabilityRouter::route(
        "Open in browser http://localhost:8080 and inspect ui layout",
        false,
    );
    assert!(matches!(act3, WebCapabilityAction::Navigate { .. }));

    // 4. Interactive click / form action (Level 5)
    let act4 = WebCapabilityRouter::route("Click login button and fill password field", true);
    assert!(matches!(act4, WebCapabilityAction::Interact { .. }));

    // 5. Advanced diagnostics (Level 6)
    let act5 = WebCapabilityRouter::route(
        "Inspect console error logs and check why network api failing with 500",
        true,
    );
    assert!(matches!(act5, WebCapabilityAction::Diagnose { .. }));
}

#[test]
fn test_action_loop_detector_tripwire() {
    let mut detector = ActionLoopDetector::new(5);

    // 3 actions on the same element are tolerated
    assert!(detector.record_action("click", "ref_001").is_ok());
    assert!(detector.record_action("click", "ref_001").is_ok());
    assert!(detector.record_action("click", "ref_001").is_ok());

    // 4th identical action triggers runaway loop detection
    let err = detector.record_action("click", "ref_001");
    assert!(matches!(
        err,
        Err(BrowserError::BrowserLoopDetected {
            action,
            element_ref
        }) if action == "click" && element_ref == "ref_001"
    ));
}

#[test]
fn test_html_to_clean_markdown_stripping() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Test Page</title>
            <style>body { background: #fff; } .ad { display: block; }</style>
        </head>
        <body>
            <script>window.tracker = 123;</script>
            <svg><path d="M0 0"/></svg>
            <h1>Hades Web Architecture</h1>
            <p>Direct HTTP fetch converts HTML into <strong>clean Markdown</strong> without launching a full browser sidecar.</p>
            <a href="https://example.com/docs">Documentation Link</a>
            <ul>
                <li>Search Layer</li>
                <li>Fetch Layer</li>
                <li>CDP Browser Automation</li>
            </ul>
        </body>
        </html>
    "#;

    let md = ContentExtractor::html_to_clean_markdown(html);

    assert!(md.contains("# Hades Web Architecture"));
    assert!(md.contains("clean Markdown"));
    assert!(md.contains("[Documentation Link](https://example.com/docs)"));
    assert!(md.contains("- Search Layer"));
    assert!(md.contains("- Fetch Layer"));
    assert!(md.contains("- CDP Browser Automation"));

    // Stripped elements
    assert!(!md.contains("<script>"));
    assert!(!md.contains("<style>"));
    assert!(!md.contains("<svg>"));
}

#[tokio::test]
async fn test_browser_tool_suite_registration_and_execution() {
    let tmp = tempdir().expect("create temp dir");
    let manager = Arc::new(BrowserManager::new(tmp.path()));
    let mut registry = ToolRegistry::new();

    BrowserToolSet::register_all(&mut registry, manager);

    // Verify all 22 web & browser tools are registered
    let expected_tools = [
        "web.search",
        "web.fetch",
        "browser.start",
        "browser.close",
        "browser.status",
        "browser.tabs",
        "browser.open",
        "browser.snapshot",
        "browser.extract_text",
        "browser.extract_markdown",
        "browser.get_links",
        "browser.click",
        "browser.fill",
        "browser.select",
        "browser.scroll",
        "browser.hover",
        "browser.press_key",
        "browser.screenshot",
        "browser.pdf",
        "browser.console",
        "browser.network",
        "browser.evaluate",
    ];

    for name in &expected_tools {
        assert!(
            registry.contains(name),
            "Tool '{}' should be registered in ToolRegistry",
            name
        );
    }

    let context = ToolContext::new("test_session", tmp.path(), tmp.path());

    // Test web.search input validation
    let search_tool = registry.get("web.search").expect("web.search");
    let invalid_search = search_tool
        .execute("call_1", serde_json::json!({}), &context)
        .await;
    assert_eq!(invalid_search.status, ToolStatus::InvalidInput);

    // Test web.fetch input validation
    let fetch_tool = registry.get("web.fetch").expect("web.fetch");
    let invalid_fetch = fetch_tool
        .execute("call_2", serde_json::json!({}), &context)
        .await;
    assert_eq!(invalid_fetch.status, ToolStatus::InvalidInput);

    // Test browser.status execution
    let status_tool = registry.get("browser.status").expect("browser.status");
    let status_res = status_tool
        .execute("call_3", serde_json::json!({}), &context)
        .await;
    assert_eq!(status_res.status, ToolStatus::Success);
    assert!(status_res.output.contains("Browser Status:"));
}

#[tokio::test]
async fn test_browser_agent_roles_and_permission_whitelists() {
    let browser_agent = AgentRole::BrowserAgent;
    let web_testing_agent = AgentRole::WebTestingAgent;
    let researcher = AgentRole::Researcher;

    assert_eq!(browser_agent.name(), "Browser Agent");
    assert_eq!(web_testing_agent.name(), "Web Testing Agent");

    // Whitelist checks
    let browser_tools = browser_agent.allowed_tool_patterns();
    assert!(browser_tools.contains(&"web.*"));
    assert!(browser_tools.contains(&"browser.open"));
    assert!(browser_tools.contains(&"browser.snapshot"));

    let web_testing_tools = web_testing_agent.allowed_tool_patterns();
    assert!(web_testing_tools.contains(&"web.*"));
    assert!(web_testing_tools.contains(&"browser.*"));

    let researcher_tools = researcher.allowed_tool_patterns();
    assert!(researcher_tools.contains(&"web.*"));
    assert!(researcher_tools.contains(&"browser.open"));
}

#[tokio::test]
async fn test_browser_slash_command_execution() {
    let tmp = tempdir().expect("create temp dir");
    let config_service = ConfigService::with_path(tmp.path().join("config.toml"));
    let storage_service = StorageService::with_root(tmp.path().join("storage"));
    let event_bus = EventBus::new();

    let mut app = HadesApp::new(config_service, storage_service, event_bus);
    app.init().expect("app init");

    // Execute `/browser` command
    let output = app
        .execute_command("/browser")
        .expect("execute /browser command");

    match output {
        CommandOutput::Text(text) => {
            assert!(text.contains("HADES WEB INTELLIGENCE & BROWSER SIDECAR"));
            assert!(text.contains("Available Web Retrieval & Automation Capabilities:"));
            assert!(text.contains("1. Search Layer:"));
            assert!(text.contains("2. Fetch Layer:"));
            assert!(text.contains("3. Browser Sidecar:"));
        }
        _ => panic!("Expected Text command output"),
    }
}
