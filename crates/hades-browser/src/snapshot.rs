use chrono::Utc;
use serde_json::json;

use crate::cdp::CdpClient;
use crate::error::BrowserError;
use crate::types::{ElementRef, PageSnapshot};

/// Generator for structured, accessibility-first page snapshots.
pub struct SnapshotEngine;

impl SnapshotEngine {
    /// Injects an accessibility-extraction script into the active page context and builds a `PageSnapshot`.
    pub async fn capture_snapshot(
        client: &CdpClient,
        tab_id: &str,
        generation: u64,
    ) -> Result<PageSnapshot, BrowserError> {
        let script = r#"
        (function() {
            const elements = [];
            let refCount = 1;

            function isVisible(el) {
                if (!el) return false;
                const style = window.getComputedStyle(el);
                if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') return false;
                const rect = el.getBoundingClientRect();
                return rect.width > 0 && rect.height > 0;
            }

            function getAccessibleRole(el) {
                const explicitRole = el.getAttribute('role');
                if (explicitRole) return explicitRole.toLowerCase();

                const tag = el.tagName.toLowerCase();
                if (tag === 'button') return 'button';
                if (tag === 'a' && el.hasAttribute('href')) return 'link';
                if (tag === 'input') {
                    const type = (el.getAttribute('type') || 'text').toLowerCase();
                    if (type === 'button' || type === 'submit' || type === 'reset') return 'button';
                    if (type === 'checkbox') return 'checkbox';
                    if (type === 'radio') return 'radio';
                    return 'textbox';
                }
                if (tag === 'textarea') return 'textbox';
                if (tag === 'select') return 'combobox';
                if (/^h[1-6]$/.test(tag)) return 'heading';
                return null;
            }

            function getAccessibleName(el) {
                const ariaLabel = el.getAttribute('aria-label');
                if (ariaLabel && ariaLabel.trim()) return ariaLabel.trim();

                const ariaLabelledBy = el.getAttribute('aria-labelledby');
                if (ariaLabelledBy) {
                    const labelEl = document.getElementById(ariaLabelledBy);
                    if (labelEl && labelEl.textContent) return labelEl.textContent.trim();
                }

                if (el.tagName.toLowerCase() === 'input' || el.tagName.toLowerCase() === 'textarea') {
                    if (el.placeholder && el.placeholder.trim()) return el.placeholder.trim();
                }

                const title = el.getAttribute('title');
                if (title && title.trim()) return title.trim();

                const text = el.innerText || el.textContent || '';
                return text.replace(/\s+/g, ' ').trim();
            }

            function getCssSelector(el) {
                if (el.id) return '#' + CSS.escape(el.id);
                if (el.getAttribute('name')) return el.tagName.toLowerCase() + '[name="' + CSS.escape(el.getAttribute('name')) + '"]';
                let path = [];
                let curr = el;
                while (curr && curr.nodeType === Node.ELEMENT_NODE && curr !== document.body && curr !== document.documentElement) {
                    let selector = curr.tagName.toLowerCase();
                    let sibling = curr;
                    let nth = 1;
                    while (sibling = sibling.previousElementSibling) {
                        if (sibling.tagName.toLowerCase() === selector) nth++;
                    }
                    if (nth > 1) selector += `:nth-of-type(${nth})`;
                    path.unshift(selector);
                    curr = curr.parentElement;
                }
                return path.join(' > ');
            }

            // Walk DOM tree
            const walker = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_ELEMENT);
            let currentNode = walker.currentNode;

            while (currentNode && elements.length < 150) {
                if (currentNode instanceof HTMLElement && isVisible(currentNode)) {
                    const role = getAccessibleRole(currentNode);
                    if (role) {
                        let name = getAccessibleName(currentNode);
                        if (name.length > 80) name = name.substring(0, 77) + '...';
                        const selector = getCssSelector(currentNode);
                        const refId = 'ref_' + String(refCount).padStart(3, '0');
                        refCount++;

                        let value = null;
                        if (currentNode.value !== undefined) {
                            value = String(currentNode.value);
                            if (value.length > 50) value = value.substring(0, 47) + '...';
                        }

                        // Attach marker attribute for reliable lookup
                        currentNode.setAttribute('data-hades-ref', refId);

                        elements.push({
                            id: refId,
                            role: role,
                            name: name,
                            value: value,
                            selector: selector,
                            is_interactable: true
                        });
                    }
                }
                currentNode = walker.nextNode();
            }

            const title = document.title || 'Untitled';
            const url = window.location.href;

            return {
                title: title,
                url: url,
                elements: elements
            };
        })();
        "#;

        let eval_result = client
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": script,
                    "returnByValue": true,
                    "awaitPromise": true
                }),
            )
            .await?;

        let value = &eval_result["result"]["value"];
        let page_title = value["title"].as_str().unwrap_or("Untitled").to_string();
        let page_url = value["url"].as_str().unwrap_or("").to_string();

        let raw_elements = value["elements"].as_array();
        let mut elements = Vec::new();
        let mut tree_lines = Vec::new();

        tree_lines.push(format!("PAGE: {} ({})", page_title, page_url));
        tree_lines.push("ACCESSIBILITY ELEMENTS:".to_string());

        if let Some(arr) = raw_elements {
            for item in arr {
                let id = item["id"].as_str().unwrap_or("").to_string();
                let role = item["role"].as_str().unwrap_or("").to_string();
                let name = item["name"].as_str().unwrap_or("").to_string();
                let value_opt = item["value"].as_str().map(|s| s.to_string());
                let selector = item["selector"].as_str().unwrap_or("").to_string();
                let is_interactable = item["is_interactable"].as_bool().unwrap_or(true);

                let line = match &value_opt {
                    Some(v) if !v.is_empty() => {
                        format!("  [{}] {} \"{}\" (value: \"{}\")", id, role, name, v)
                    }
                    _ => format!("  [{}] {} \"{}\"", id, role, name),
                };

                tree_lines.push(line);

                elements.push(ElementRef {
                    id,
                    role,
                    name,
                    value: value_opt,
                    selector,
                    backend_node_id: None,
                    is_interactable,
                });
            }
        }

        if elements.is_empty() {
            tree_lines.push("  (No interactive elements detected on page)".to_string());
        }

        let summary = format!(
            "Page '{}' loaded with {} interactive elements.",
            page_title,
            elements.len()
        );

        Ok(PageSnapshot {
            url: page_url,
            title: page_title,
            tab_id: tab_id.to_string(),
            generation,
            elements,
            content_summary: summary,
            accessibility_tree_text: tree_lines.join("\n"),
            captured_at: Utc::now(),
        })
    }
}
