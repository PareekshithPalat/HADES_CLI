use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;

use crate::error::BrowserError;
use crate::extraction::ContentExtractor;
use crate::types::WebSearchResult;

/// Result returned from a direct HTTP web fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResult {
    pub url: String,
    pub status_code: u16,
    pub content_type: String,
    pub content_markdown: String,
    pub character_count: usize,
}

/// Fast, lightweight web search and direct page retrieval engine (does not launch a browser).
pub struct RetrievalEngine;

impl RetrievalEngine {
    /// Executes a web search query and returns structured results.
    pub async fn search(query: &str, limit: usize) -> Result<Vec<WebSearchResult>, BrowserError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Err(BrowserError::SearchFailed(
                "Search query cannot be empty".to_string(),
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .map_err(|e| BrowserError::SearchFailed(e.to_string()))?;

        // Query DuckDuckGo HTML endpoint
        let encoded_query: String =
            url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect();
        let search_url = format!("https://html.duckduckgo.com/html/?q={}", encoded_query);

        let resp = client
            .get(&search_url)
            .send()
            .await
            .map_err(|e| BrowserError::SearchFailed(format!("HTTP request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(BrowserError::SearchFailed(format!(
                "Search endpoint returned HTTP status {}",
                resp.status()
            )));
        }

        let body = resp.text().await.map_err(|e| {
            BrowserError::SearchFailed(format!("Failed to read response body: {e}"))
        })?;

        let mut results = Vec::new();
        let max_items = limit.clamp(1, 20);

        // Regex parser for DuckDuckGo result blocks
        let re_title = Regex::new(r#"class="result__a"[^>]*>([^<]+)</a>"#).unwrap();
        let re_block = Regex::new(r#"(?s)<div class="result__body[^>]*>(.*?)</div>"#).unwrap();
        let re_a =
            Regex::new(r#"(?s)<a class="result__url"[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#).unwrap();
        let re_snip = Regex::new(r#"(?s)<a class="result__snippet"[^>]*>(.*?)</a>"#).unwrap();

        for cap in re_block.captures_iter(&body) {
            let block = &cap[1];
            let mut link_url = String::new();
            let mut title = String::new();
            let mut snippet = String::new();

            if let Some(a_cap) = re_title.captures(block) {
                title = a_cap[1].trim().to_string();
            }

            if let Some(u_cap) = re_a.captures(block) {
                let raw_u = u_cap[1].trim();
                // Decode DuckDuckGo redirect URL `//duckduckgo.com/l/?uddg=...`
                if raw_u.contains("uddg=") {
                    if let Some(target) = raw_u.split("uddg=").nth(1) {
                        let clean = target.split('&').next().unwrap_or(target);
                        let decoded = url::form_urlencoded::parse(clean.as_bytes())
                            .map(|(k, v)| {
                                if v.is_empty() {
                                    k.to_string()
                                } else {
                                    format!("{k}={v}")
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("");
                        if !decoded.is_empty() {
                            link_url = decoded;
                        }
                    }
                }
                if link_url.is_empty() {
                    link_url = raw_u.to_string();
                }
            }

            if let Some(s_cap) = re_snip.captures(block) {
                snippet = s_cap[1]
                    .replace("<b>", "")
                    .replace("</b>", "")
                    .replace("&amp;", "&")
                    .replace("&quot;", "\"")
                    .trim()
                    .to_string();
            }

            if !title.is_empty() && !link_url.is_empty() {
                results.push(WebSearchResult {
                    title,
                    url: link_url,
                    snippet,
                });
            }

            if results.len() >= max_items {
                break;
            }
        }

        // Fallback: if HTML layout changed, return a graceful fallback result
        if results.is_empty() {
            debug!("Zero parsed results from primary parser for '{}'", trimmed);
            let fallback_q: String =
                url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect();
            results.push(WebSearchResult {
                title: format!("Search query for '{}'", trimmed),
                url: format!("https://duckduckgo.com/?q={}", fallback_q),
                snippet: format!(
                    "Direct search query results available online for '{}'",
                    trimmed
                ),
            });
        }

        Ok(results)
    }

    /// Fetches a web page directly via HTTP, extracting clean Markdown without a browser sidecar.
    pub async fn fetch(url: &str) -> Result<FetchResult, BrowserError> {
        let trimmed_url = url.trim();
        if !trimmed_url.starts_with("http://") && !trimmed_url.starts_with("https://") {
            return Err(BrowserError::RetrievalFailed {
                url: trimmed_url.to_string(),
                reason: "URL must begin with http:// or https://".to_string(),
            });
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .map_err(|e| BrowserError::RetrievalFailed {
                url: trimmed_url.to_string(),
                reason: e.to_string(),
            })?;

        let resp =
            client
                .get(trimmed_url)
                .send()
                .await
                .map_err(|e| BrowserError::RetrievalFailed {
                    url: trimmed_url.to_string(),
                    reason: format!("HTTP request failed: {e}"),
                })?;

        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/html")
            .to_string();

        let raw_body = resp
            .text()
            .await
            .map_err(|e| BrowserError::RetrievalFailed {
                url: trimmed_url.to_string(),
                reason: format!("Failed to read response body: {e}"),
            })?;

        let markdown = if content_type.contains("text/html") || raw_body.contains("<html") {
            ContentExtractor::html_to_clean_markdown(&raw_body)
        } else {
            ContentExtractor::bound_output(&raw_body, 12_000)
        };

        let char_count = markdown.len();

        Ok(FetchResult {
            url: trimmed_url.to_string(),
            status_code: status,
            content_type,
            content_markdown: markdown,
            character_count: char_count,
        })
    }
}
