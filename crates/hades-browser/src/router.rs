use serde::{Deserialize, Serialize};

/// Chosen capability route for fulfilling a web-related user request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebCapabilityAction {
    /// Level 2: Fast web search query without browser.
    Search { query: String, reason: String },
    /// Level 3: Direct page fetch and Markdown conversion without browser.
    Fetch { url: String, reason: String },
    /// Level 4: Headless browser page navigation and snapshot.
    Navigate { url: String, reason: String },
    /// Level 5: Interactive browser actions (click, form filling, dropdowns).
    Interact { target: String, reason: String },
    /// Level 6: Advanced browser console, network, and performance diagnostics.
    Diagnose { reason: String },
}

/// Intelligent capability router that selects the lowest-cost and least invasive web tool.
pub struct WebCapabilityRouter;

impl WebCapabilityRouter {
    /// Evaluates user objective and intent to select the optimal web capability.
    pub fn route(objective: &str, _has_active_browser_page: bool) -> WebCapabilityAction {
        let text = objective.trim().to_lowercase();

        // 1. Diagnostic intent (Level 6)
        if text.contains("console error")
            || text.contains("network fail")
            || text.contains("inspect network")
            || text.contains("api 500")
            || text.contains("why is this javascript")
            || text.contains("why is this website failing")
        {
            return WebCapabilityAction::Diagnose {
                reason: "User requested web developer diagnostics (console or network errors)."
                    .to_string(),
            };
        }

        // 2. Interactive intent (Level 5)
        let interactive_keywords = [
            "click ",
            "fill ",
            "type into",
            "select option",
            "log in",
            "login",
            "sign in",
            "submit form",
            "press button",
            "test form",
            "checkout",
        ];
        if interactive_keywords.iter().any(|k| text.contains(k)) {
            return WebCapabilityAction::Interact {
                target: objective.to_string(),
                reason: "Objective requires user interaction on interactive page elements."
                    .to_string(),
            };
        }

        // 3. Check for URLs
        let extracted_url = Self::extract_first_url(objective);

        if let Some(url) = extracted_url {
            // If user explicitly says "open in browser" or "test page", use Browser (Level 4)
            if text.contains("open in browser")
                || text.contains("browser open")
                || text.contains("test page")
                || text.contains("inspect ui")
                || text.contains("screenshot")
                || text.contains("dynamic")
                || text.contains("localhost")
            {
                return WebCapabilityAction::Navigate {
                    url,
                    reason: "Request requires real browser rendering, visual snapshot, or local dev server testing.".to_string(),
                };
            }

            // Static reading / summarization -> Direct Fetch (Level 3)
            return WebCapabilityAction::Fetch {
                url,
                reason: "Direct HTTP fetch is sufficient to read page content without launching a browser.".to_string(),
            };
        }

        // 4. Default for information queries -> Fast Web Search (Level 2)
        WebCapabilityAction::Search {
            query: objective.to_string(),
            reason: "General information retrieval: perform fast web search before launching full browser.".to_string(),
        }
    }

    fn extract_first_url(input: &str) -> Option<String> {
        for token in input.split_whitespace() {
            let clean = token.trim_matches(|c| {
                c == '(' || c == ')' || c == '"' || c == '\'' || c == '<' || c == '>'
            });
            if clean.starts_with("http://") || clean.starts_with("https://") {
                return Some(clean.to_string());
            }
        }
        None
    }
}
