use serde_json::json;
use std::collections::VecDeque;

use crate::cdp::CdpClient;
use crate::error::BrowserError;
use crate::types::PageSnapshot;

/// Tracks recent browser actions to detect runaway loops.
#[derive(Debug, Clone)]
pub struct ActionLoopDetector {
    recent_actions: VecDeque<(String, String)>,
    max_history: usize,
}

impl ActionLoopDetector {
    pub fn new(max_history: usize) -> Self {
        Self {
            recent_actions: VecDeque::with_capacity(max_history),
            max_history,
        }
    }

    /// Records an action and returns an error if a repetitive loop is detected.
    pub fn record_action(&mut self, action: &str, target: &str) -> Result<(), BrowserError> {
        let entry = (action.to_string(), target.to_string());
        self.recent_actions.push_back(entry.clone());
        if self.recent_actions.len() > self.max_history {
            self.recent_actions.pop_front();
        }

        // Detect 4 consecutive identical actions
        if self.recent_actions.len() >= 4 {
            let last_four: Vec<_> = self.recent_actions.iter().rev().take(4).collect();
            if last_four.iter().all(|item| *item == &entry) {
                return Err(BrowserError::BrowserLoopDetected {
                    action: action.to_string(),
                    element_ref: target.to_string(),
                });
            }
        }

        Ok(())
    }
}

impl Default for ActionLoopDetector {
    fn default() -> Self {
        Self::new(10)
    }
}

/// Dispatches structured accessibility and DOM actions to the active page.
pub struct InteractionEngine;

impl InteractionEngine {
    /// Clicks on an element by accessibility reference (`ref_001`) or fallback selector.
    pub async fn click(
        client: &CdpClient,
        target: &str,
        snapshot: Option<&PageSnapshot>,
    ) -> Result<String, BrowserError> {
        let (selector, name) = Self::resolve_target(target, snapshot)?;

        let script = format!(
            r#"
            (function() {{
                let el = document.querySelector('{selector}');
                if (!el && '{target}'.startsWith('ref_')) {{
                    el = document.querySelector('[data-hades-ref="{target}"]');
                }}
                if (!el) return {{ success: false, error: 'Element not found' }};
                
                el.scrollIntoView({{ block: 'center', inline: 'center', behavior: 'instant' }});
                
                // Trigger focus and click
                el.focus();
                el.click();
                
                return {{
                    success: true,
                    tagName: el.tagName,
                    text: el.innerText || el.textContent || ''
                }};
            }})();
            "#
        );

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
        if val["success"].as_bool().unwrap_or(false) {
            Ok(format!("Successfully clicked on '{}' ({})", name, target))
        } else {
            let err = val["error"].as_str().unwrap_or("Failed to click element");
            Err(BrowserError::ElementNotFound {
                target: format!("{target} ({err})"),
            })
        }
    }

    /// Fills input or textarea with text.
    pub async fn fill(
        client: &CdpClient,
        target: &str,
        text: &str,
        snapshot: Option<&PageSnapshot>,
    ) -> Result<String, BrowserError> {
        let (selector, name) = Self::resolve_target(target, snapshot)?;
        let escaped_text = serde_json::to_string(text).unwrap_or_else(|_| format!("\"{text}\""));

        let script = format!(
            r#"
            (function() {{
                let el = document.querySelector('{selector}');
                if (!el && '{target}'.startsWith('ref_')) {{
                    el = document.querySelector('[data-hades-ref="{target}"]');
                }}
                if (!el) return {{ success: false, error: 'Element not found' }};

                el.scrollIntoView({{ block: 'center', inline: 'center', behavior: 'instant' }});
                el.focus();
                el.value = {escaped_text};

                // Dispatch standard DOM input and change events
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));

                return {{ success: true }};
            }})();
            "#
        );

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
        if val["success"].as_bool().unwrap_or(false) {
            Ok(format!("Successfully filled '{}' with text.", name))
        } else {
            let err = val["error"].as_str().unwrap_or("Failed to fill element");
            Err(BrowserError::ElementNotFound {
                target: format!("{target} ({err})"),
            })
        }
    }

    /// Selects an option in a `<select>` dropdown.
    pub async fn select_option(
        client: &CdpClient,
        target: &str,
        option_value: &str,
        snapshot: Option<&PageSnapshot>,
    ) -> Result<String, BrowserError> {
        let (selector, name) = Self::resolve_target(target, snapshot)?;
        let escaped_val = serde_json::to_string(option_value).unwrap_or_default();

        let script = format!(
            r#"
            (function() {{
                let el = document.querySelector('{selector}');
                if (!el && '{target}'.startsWith('ref_')) {{
                    el = document.querySelector('[data-hades-ref="{target}"]');
                }}
                if (!el || el.tagName.toLowerCase() !== 'select') {{
                    return {{ success: false, error: 'Select dropdown element not found' }};
                }}

                el.value = {escaped_val};
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return {{ success: true }};
            }})();
            "#
        );

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
        if val["success"].as_bool().unwrap_or(false) {
            Ok(format!("Selected option '{option_value}' in '{name}'"))
        } else {
            Err(BrowserError::ElementNotFound {
                target: target.to_string(),
            })
        }
    }

    /// Scrolls the active viewport.
    pub async fn scroll(
        client: &CdpClient,
        direction: &str,
        amount_pixels: i32,
    ) -> Result<String, BrowserError> {
        let delta_y = match direction.to_lowercase().as_str() {
            "down" => amount_pixels.abs(),
            "up" => -amount_pixels.abs(),
            "top" => -100_000,
            "bottom" => 100_000,
            _ => amount_pixels,
        };

        let script = format!("window.scrollBy({{ top: {delta_y}, behavior: 'instant' }});");
        let _ = client
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": script,
                    "returnByValue": true
                }),
            )
            .await?;

        Ok(format!(
            "Scrolled window {} by {}px",
            direction,
            amount_pixels.abs()
        ))
    }

    /// Hovers over a target element.
    pub async fn hover(
        client: &CdpClient,
        target: &str,
        snapshot: Option<&PageSnapshot>,
    ) -> Result<String, BrowserError> {
        let (selector, name) = Self::resolve_target(target, snapshot)?;
        let script = format!(
            r#"
            (function() {{
                let el = document.querySelector('{selector}');
                if (!el && '{target}'.startsWith('ref_')) {{
                    el = document.querySelector('[data-hades-ref="{target}"]');
                }}
                if (!el) return false;
                el.dispatchEvent(new MouseEvent('mouseover', {{ bubbles: true }}));
                el.dispatchEvent(new MouseEvent('mouseenter', {{ bubbles: true }}));
                return true;
            }})();
            "#
        );

        let res = client
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": script,
                    "returnByValue": true
                }),
            )
            .await?;

        if res["result"]["value"].as_bool().unwrap_or(false) {
            Ok(format!("Hovered over '{name}'"))
        } else {
            Err(BrowserError::ElementNotFound {
                target: target.to_string(),
            })
        }
    }

    /// Sends a keyboard key press.
    pub async fn press_key(client: &CdpClient, key: &str) -> Result<String, BrowserError> {
        client
            .call(
                "Input.dispatchKeyEvent",
                json!({
                    "type": "keyDown",
                    "key": key
                }),
            )
            .await?;

        client
            .call(
                "Input.dispatchKeyEvent",
                json!({
                    "type": "keyUp",
                    "key": key
                }),
            )
            .await?;

        Ok(format!("Pressed key '{key}'"))
    }

    fn resolve_target(
        target: &str,
        snapshot: Option<&PageSnapshot>,
    ) -> Result<(String, String), BrowserError> {
        let target_trimmed = target.trim();
        if target_trimmed.starts_with("ref_") {
            if let Some(snap) = snapshot {
                if let Some(el) = snap.elements.iter().find(|e| e.id == target_trimmed) {
                    let selector = format!("[data-hades-ref=\"{}\"]", el.id);
                    let label = if el.name.is_empty() {
                        el.role.clone()
                    } else {
                        el.name.clone()
                    };
                    return Ok((selector, label));
                }
                return Err(BrowserError::StaleElementReference {
                    element_ref: target_trimmed.to_string(),
                });
            }
            // If no snapshot in context, query by attribute fallback
            return Ok((
                format!("[data-hades-ref=\"{target_trimmed}\"]"),
                target_trimmed.to_string(),
            ));
        }

        // Fallback: direct CSS selector
        Ok((target_trimmed.to_string(), target_trimmed.to_string()))
    }
}
