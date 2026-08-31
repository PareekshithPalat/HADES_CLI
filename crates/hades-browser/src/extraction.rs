use regex::Regex;
use serde_json::json;

use crate::cdp::CdpClient;
use crate::error::BrowserError;

/// Utilities for extracting text, clean Markdown, and DOM elements from web pages.
pub struct ContentExtractor;

impl ContentExtractor {
    /// Extracts readable, rendered text from the active page.
    pub async fn extract_text(client: &CdpClient) -> Result<String, BrowserError> {
        let script = r#"
        (function() {
            return document.body ? (document.body.innerText || document.body.textContent || '') : '';
        })();
        "#;

        let res = client
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": script,
                    "returnByValue": true
                }),
            )
            .await?;

        let raw_text = res["result"]["value"].as_str().unwrap_or("").to_string();
        Ok(Self::bound_output(&raw_text, 10_000))
    }

    /// Extracts clean, structured Markdown from the active page, stripping scripts, styles, and boilerplate.
    pub async fn extract_markdown(client: &CdpClient) -> Result<String, BrowserError> {
        let script = r#"
        (function() {
            function htmlToMarkdown(root) {
                if (!root) return '';

                // Clone root to avoid modifying live page DOM
                const clone = root.cloneNode(true);

                // Strip non-content and style elements
                const removeSelectors = ['script', 'style', 'noscript', 'svg', 'iframe', 'canvas', 'template'];
                removeSelectors.forEach(sel => {
                    const nodes = clone.querySelectorAll(sel);
                    nodes.forEach(n => n.remove());
                });

                function processNode(node) {
                    if (node.nodeType === Node.TEXT_NODE) {
                        return node.textContent.replace(/\s+/g, ' ');
                    }
                    if (node.nodeType !== Node.ELEMENT_NODE) return '';

                    const tag = node.tagName.toLowerCase();
                    let inner = Array.from(node.childNodes).map(processNode).join('');

                    switch (tag) {
                        case 'h1': return '\n# ' + inner.trim() + '\n\n';
                        case 'h2': return '\n## ' + inner.trim() + '\n\n';
                        case 'h3': return '\n### ' + inner.trim() + '\n\n';
                        case 'h4': return '\n#### ' + inner.trim() + '\n\n';
                        case 'h5': return '\n##### ' + inner.trim() + '\n\n';
                        case 'h6': return '\n###### ' + inner.trim() + '\n\n';
                        case 'p': return '\n\n' + inner.trim() + '\n\n';
                        case 'br': return '\n';
                        case 'hr': return '\n---\n';
                        case 'strong':
                        case 'b': return '**' + inner.trim() + '**';
                        case 'em':
                        case 'i': return '*' + inner.trim() + '*';
                        case 'code': return '`' + inner.trim() + '`';
                        case 'pre': return '\n```\n' + inner.trim() + '\n```\n';
                        case 'a': {
                            const href = node.getAttribute('href') || '';
                            const text = inner.trim();
                            if (!text) return '';
                            return '[' + text + '](' + href + ')';
                        }
                        case 'li': return '- ' + inner.trim() + '\n';
                        case 'ul':
                        case 'ol': return '\n' + inner.trim() + '\n';
                        case 'blockquote': return '\n> ' + inner.trim() + '\n';
                        default: return inner;
                    }
                }

                return processNode(clone).replace(/\n{3,}/g, '\n\n').trim();
            }

            const main = document.querySelector('main') || document.querySelector('article') || document.body;
            return htmlToMarkdown(main);
        })();
        "#;

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

        let markdown = res["result"]["value"].as_str().unwrap_or("").to_string();
        Ok(Self::bound_output(&markdown, 12_000))
    }

    /// Converts raw HTML string to clean Markdown (used for search/fetch without browser).
    pub fn html_to_clean_markdown(raw_html: &str) -> String {
        // 1. Strip script and style blocks
        let re_script = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
        let re_style = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
        let re_svg = Regex::new(r"(?is)<svg[^>]*>.*?</svg>").unwrap();

        let s = re_script.replace_all(raw_html, "");
        let s = re_style.replace_all(&s, "");
        let s = re_svg.replace_all(&s, "");

        // 2. Convert common HTML tags
        let re_h1 = Regex::new(r"(?is)<h1[^>]*>(.*?)</h1>").unwrap();
        let re_h2 = Regex::new(r"(?is)<h2[^>]*>(.*?)</h2>").unwrap();
        let re_h3 = Regex::new(r"(?is)<h3[^>]*>(.*?)</h3>").unwrap();
        let re_p = Regex::new(r"(?is)<p[^>]*>(.*?)</p>").unwrap();
        let re_li = Regex::new(r"(?is)<li[^>]*>(.*?)</li>").unwrap();
        let re_a = Regex::new(r#"(?is)<a\s+[^>]*href=["']([^"']*)["'][^>]*>(.*?)</a>"#).unwrap();
        let re_tags = Regex::new(r"<[^>]+>").unwrap();
        let re_spaces = Regex::new(r"[ \t]+").unwrap();
        let re_newlines = Regex::new(r"\n{3,}").unwrap();

        let s = re_h1.replace_all(&s, "\n# $1\n\n");
        let s = re_h2.replace_all(&s, "\n## $1\n\n");
        let s = re_h3.replace_all(&s, "\n### $1\n\n");
        let s = re_p.replace_all(&s, "\n\n$1\n\n");
        let s = re_li.replace_all(&s, "\n- $1");
        let s = re_a.replace_all(&s, "[$2]($1)");

        let s = re_tags.replace_all(&s, "");
        let s = re_spaces.replace_all(&s, " ");
        let s = re_newlines.replace_all(&s, "\n\n");

        Self::bound_output(s.trim(), 12_000)
    }

    /// Retrieves all outbound hyperlinks from active page.
    pub async fn get_links(client: &CdpClient) -> Result<Vec<(String, String)>, BrowserError> {
        let script = r#"
        (function() {
            const links = [];
            const anchors = document.querySelectorAll('a[href]');
            for (let a of anchors) {
                const href = a.href;
                const text = (a.innerText || a.textContent || '').trim().replace(/\s+/g, ' ');
                if (href && !href.startsWith('javascript:')) {
                    links.push({ text: text || 'Link', url: href });
                }
                if (links.length >= 100) break;
            }
            return links;
        })();
        "#;

        let res = client
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": script,
                    "returnByValue": true
                }),
            )
            .await?;

        let mut list = Vec::new();
        if let Some(arr) = res["result"]["value"].as_array() {
            for item in arr {
                let text = item["text"].as_str().unwrap_or("").to_string();
                let url = item["url"].as_str().unwrap_or("").to_string();
                list.push((text, url));
            }
        }

        Ok(list)
    }

    /// Truncates string to a safe maximum size with ellipsis.
    pub fn bound_output(text: &str, max_chars: usize) -> String {
        if text.chars().count() <= max_chars {
            text.to_string()
        } else {
            let truncated: String = text.chars().take(max_chars).collect();
            format!(
                "{}\n\n[Content truncated ({} total characters)]",
                truncated,
                text.len()
            )
        }
    }
}
