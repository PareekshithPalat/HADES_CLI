// crates/hades-storage/src/import.rs

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::StorageError;
use crate::model::{generate_session_title, Message, MessageMetadata, MessageRole, SessionRecord};

/// Supported source formats recognized by the importer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportSourceFormat {
    Hades,
    ChatGPT,
    Claude,
    Markdown,
}

impl std::fmt::Display for ImportSourceFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hades => write!(f, "hades"),
            Self::ChatGPT => write!(f, "chatgpt"),
            Self::Claude => write!(f, "claude"),
            Self::Markdown => write!(f, "markdown"),
        }
    }
}

// ---------------------------------------------------------
// Internal Deserialization Models for ChatGPT & Claude
// ---------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ChatGptConversation {
    title: Option<String>,
    create_time: Option<f64>,
    mapping: Option<HashMap<String, ChatGptNode>>,
    current_node: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatGptNode {
    #[allow(dead_code)]
    id: Option<String>,
    message: Option<ChatGptMessage>,
    parent: Option<String>,
    #[allow(dead_code)]
    children: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ChatGptMessage {
    #[allow(dead_code)]
    id: Option<String>,
    author: Option<ChatGptAuthor>,
    create_time: Option<f64>,
    content: Option<ChatGptContent>,
    recipient: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatGptAuthor {
    role: Option<String>,
    #[allow(dead_code)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatGptContent {
    #[allow(dead_code)]
    content_type: Option<String>,
    parts: Option<Vec<Value>>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeConversation {
    #[allow(dead_code)]
    uuid: Option<String>,
    name: Option<String>,
    created_at: Option<String>,
    chat_messages: Option<Vec<ClaudeMessage>>,
}

#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    #[allow(dead_code)]
    uuid: Option<String>,
    text: Option<String>,
    sender: Option<String>,
    created_at: Option<String>,
    content: Option<Vec<ClaudeContentBlock>>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContentBlock {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    block_type: Option<String>,
    text: Option<String>,
}

// ---------------------------------------------------------
// SessionImporter Implementation
// ---------------------------------------------------------

/// Importer capable of detecting, parsing, and converting external chat session formats into Hades `SessionRecord`s.
pub struct SessionImporter;

impl SessionImporter {
    /// Inspects the content payload using JSON schema sniffing and heuristic matching to identify the source format.
    pub fn detect_format(content: &str) -> ImportSourceFormat {
        let trimmed = content.trim();

        // 1. Attempt JSON parsing
        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
        {
            if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
                match value {
                    Value::Object(ref obj) => {
                        if obj.contains_key("schema_version")
                            || (obj.contains_key("metadata") && obj.contains_key("messages"))
                        {
                            return ImportSourceFormat::Hades;
                        }
                        if obj.contains_key("mapping")
                            || (obj.contains_key("title") && obj.contains_key("current_node"))
                        {
                            return ImportSourceFormat::ChatGPT;
                        }
                        if obj.contains_key("chat_messages")
                            || (obj.contains_key("uuid") && obj.contains_key("name"))
                        {
                            return ImportSourceFormat::Claude;
                        }
                    }
                    Value::Array(ref arr) => {
                        if let Some(first) = arr.first().and_then(|v| v.as_object()) {
                            if first.contains_key("schema_version")
                                || (first.contains_key("metadata")
                                    && first.contains_key("messages"))
                            {
                                return ImportSourceFormat::Hades;
                            }
                            if first.contains_key("mapping") || first.contains_key("current_node") {
                                return ImportSourceFormat::ChatGPT;
                            }
                            if first.contains_key("chat_messages")
                                || (first.contains_key("sender")
                                    && (first.contains_key("text")
                                        || first.contains_key("content")))
                            {
                                return ImportSourceFormat::Claude;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // 2. Fallback to Markdown
        ImportSourceFormat::Markdown
    }

    /// Deserializes a native Hades session record from JSON.
    pub fn import_hades(content: &str) -> Result<SessionRecord, StorageError> {
        let trimmed = content.trim();
        if let Ok(record) = serde_json::from_str::<SessionRecord>(trimmed) {
            return Ok(record);
        }

        // If array of Hades records, take the first
        if let Ok(mut records) = serde_json::from_str::<Vec<SessionRecord>>(trimmed) {
            if let Some(record) = records.pop() {
                return Ok(record);
            }
        }

        Err(StorageError::ImportError(
            "Failed to parse Hades SessionRecord JSON schema".to_string(),
        ))
    }

    /// Deserializes an OpenAI ChatGPT conversation export (`conversations.json`).
    pub fn import_chatgpt(content: &str) -> Result<SessionRecord, StorageError> {
        let trimmed = content.trim();

        let conversation = if let Ok(conv) = serde_json::from_str::<ChatGptConversation>(trimmed) {
            conv
        } else if let Ok(mut list) = serde_json::from_str::<Vec<ChatGptConversation>>(trimmed) {
            if list.is_empty() {
                return Err(StorageError::ImportError(
                    "ChatGPT export array contains no conversation records".to_string(),
                ));
            }
            list.remove(0)
        } else {
            return Err(StorageError::ImportError(
                "Invalid ChatGPT JSON format".to_string(),
            ));
        };

        let title = conversation.title.clone();
        let mut session = SessionRecord::new(
            title,
            Some("openai".to_string()),
            Some("chatgpt".to_string()),
        );

        if let Some(created_f64) = conversation.create_time {
            let secs = created_f64 as i64;
            let nsecs = ((created_f64 - secs as f64) * 1_000_000_000.0) as u32;
            if let Some(dt) = Utc.timestamp_opt(secs, nsecs).single() {
                session.metadata.created_at = dt;
                session.metadata.updated_at = dt;
            }
        }

        if let Some(mapping) = conversation.mapping {
            // Collect message nodes and sort by create_time
            let mut message_entries: Vec<(&ChatGptNode, &ChatGptMessage, f64)> = Vec::new();

            for node in mapping.values() {
                if let Some(ref msg) = node.message {
                    let timestamp = msg.create_time.or(conversation.create_time).unwrap_or(0.0);
                    message_entries.push((node, msg, timestamp));
                }
            }

            // If current_node exists, we can trace chain from leaf to root for exact ordering
            let mut ordered_messages: Vec<&ChatGptMessage> = Vec::new();
            if let Some(ref current_id) = conversation.current_node {
                let mut curr = Some(current_id.as_str());
                let mut chain = Vec::new();
                while let Some(node_id) = curr {
                    if let Some(node) = mapping.get(node_id) {
                        if let Some(ref msg) = node.message {
                            chain.push(msg);
                        }
                        curr = node.parent.as_deref();
                    } else {
                        break;
                    }
                }
                chain.reverse();
                if !chain.is_empty() {
                    ordered_messages = chain;
                }
            }

            if ordered_messages.is_empty() {
                // Fallback: sort by create_time
                message_entries
                    .sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
                for (_, msg, _) in message_entries {
                    ordered_messages.push(msg);
                }
            }

            for msg in ordered_messages {
                let role_str = msg
                    .author
                    .as_ref()
                    .and_then(|a| a.role.as_deref())
                    .unwrap_or("user");

                let role = match role_str.to_lowercase().as_str() {
                    "user" => MessageRole::User,
                    "assistant" => MessageRole::Assistant,
                    "system" => MessageRole::System,
                    "tool" => MessageRole::Tool,
                    _ => {
                        if msg.recipient.as_deref() == Some("all") {
                            MessageRole::Assistant
                        } else {
                            MessageRole::User
                        }
                    }
                };

                let mut text_parts = Vec::new();
                if let Some(ref content) = msg.content {
                    if let Some(ref t) = content.text {
                        if !t.trim().is_empty() {
                            text_parts.push(t.clone());
                        }
                    }
                    if let Some(ref parts) = content.parts {
                        for part in parts {
                            match part {
                                Value::String(s) => {
                                    if !s.trim().is_empty() {
                                        text_parts.push(s.clone());
                                    }
                                }
                                Value::Object(obj) => {
                                    if let Some(Value::String(s)) = obj.get("text") {
                                        if !s.trim().is_empty() {
                                            text_parts.push(s.clone());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                let content_str = text_parts.join("\n").trim().to_string();
                if content_str.is_empty() && role == MessageRole::System {
                    continue;
                }

                let mut message = match role {
                    MessageRole::User => Message::user(&session.metadata.id, content_str),
                    MessageRole::Assistant => Message::assistant(
                        &session.metadata.id,
                        content_str,
                        Some("openai".to_string()),
                        Some("chatgpt".to_string()),
                    ),
                    MessageRole::System => Message::system(&session.metadata.id, content_str),
                    MessageRole::Tool => Message::tool(&session.metadata.id, content_str),
                    MessageRole::Error => Message::error(&session.metadata.id, content_str),
                };

                if let Some(created_f64) = msg.create_time {
                    let secs = created_f64 as i64;
                    let nsecs = ((created_f64 - secs as f64) * 1_000_000_000.0) as u32;
                    if let Some(dt) = Utc.timestamp_opt(secs, nsecs).single() {
                        message.created_at = dt;
                    }
                }

                session.add_message(message);
            }
        }

        if session.messages.is_empty() {
            return Err(StorageError::ImportError(
                "No readable conversation turns found in ChatGPT payload".to_string(),
            ));
        }

        Ok(session)
    }

    /// Deserializes an Anthropic Claude transcript JSON export.
    pub fn import_claude(content: &str) -> Result<SessionRecord, StorageError> {
        let trimmed = content.trim();

        let conversation = if let Ok(conv) = serde_json::from_str::<ClaudeConversation>(trimmed) {
            conv
        } else if let Ok(mut list) = serde_json::from_str::<Vec<ClaudeConversation>>(trimmed) {
            if list.is_empty() {
                return Err(StorageError::ImportError(
                    "Claude export array contains no conversation records".to_string(),
                ));
            }
            list.remove(0)
        } else if let Ok(direct_messages) = serde_json::from_str::<Vec<ClaudeMessage>>(trimmed) {
            ClaudeConversation {
                uuid: None,
                name: None,
                created_at: None,
                chat_messages: Some(direct_messages),
            }
        } else {
            return Err(StorageError::ImportError(
                "Invalid Claude JSON format".to_string(),
            ));
        };

        let title = conversation.name.clone();
        let mut session = SessionRecord::new(
            title,
            Some("anthropic".to_string()),
            Some("claude".to_string()),
        );

        if let Some(ref created_str) = conversation.created_at {
            if let Ok(dt) = DateTime::parse_from_rfc3339(created_str) {
                session.metadata.created_at = dt.with_timezone(&Utc);
                session.metadata.updated_at = dt.with_timezone(&Utc);
            }
        }

        if let Some(chat_messages) = conversation.chat_messages {
            for raw_msg in chat_messages {
                let sender_str = raw_msg.sender.as_deref().unwrap_or("human");
                let role = match sender_str.to_lowercase().as_str() {
                    "human" | "user" => MessageRole::User,
                    "assistant" | "claude" => MessageRole::Assistant,
                    "system" => MessageRole::System,
                    _ => MessageRole::User,
                };

                let mut body = String::new();
                if let Some(ref t) = raw_msg.text {
                    body.push_str(t);
                }
                if let Some(ref blocks) = raw_msg.content {
                    for b in blocks {
                        if let Some(ref text) = b.text {
                            if !body.is_empty() {
                                body.push('\n');
                            }
                            body.push_str(text);
                        }
                    }
                }

                let text_content = body.trim().to_string();
                if text_content.is_empty() {
                    continue;
                }

                let mut message = match role {
                    MessageRole::User => Message::user(&session.metadata.id, text_content),
                    MessageRole::Assistant => Message::assistant(
                        &session.metadata.id,
                        text_content,
                        Some("anthropic".to_string()),
                        Some("claude".to_string()),
                    ),
                    MessageRole::System => Message::system(&session.metadata.id, text_content),
                    MessageRole::Tool => Message::tool(&session.metadata.id, text_content),
                    MessageRole::Error => Message::error(&session.metadata.id, text_content),
                };

                if let Some(ref dt_str) = raw_msg.created_at {
                    if let Ok(dt) = DateTime::parse_from_rfc3339(dt_str) {
                        message.created_at = dt.with_timezone(&Utc);
                    }
                }

                session.add_message(message);
            }
        }

        if session.messages.is_empty() {
            return Err(StorageError::ImportError(
                "No readable conversation turns found in Claude payload".to_string(),
            ));
        }

        Ok(session)
    }

    /// Parses Markdown transcripts with user/assistant headers into structured messages.
    pub fn import_markdown(content: &str) -> Result<SessionRecord, StorageError> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err(StorageError::ImportError(
                "Markdown transcript content is empty".to_string(),
            ));
        }

        let lines: Vec<&str> = trimmed.lines().collect();

        // Regex patterns for section headers
        let title_re = Regex::new(r"^#\s+(?:Session:\s*)?(.*)$")
            .map_err(|e| StorageError::ImportError(format!("Regex initialization failed: {e}")))?;
        let user_re = Regex::new(r"^(?i)(?:##|###)?\s*(?:\*\*)?User(?:\*\*)?:?\s*(?:\*(?:.*)\*)?$")
            .map_err(|e| StorageError::ImportError(format!("Regex error: {e}")))?;
        let assistant_re = Regex::new(
            r"^(?i)(?:##|###)?\s*(?:\*\*)?(?:Assistant|Claude|ChatGPT|AI|Hades)(?:\*\*)?:?\s*(?:\*(?:.*)\*)?$",
        )
        .map_err(|e| StorageError::ImportError(format!("Regex error: {e}")))?;
        let system_re =
            Regex::new(r"^(?i)(?:##|###)?\s*(?:\*\*)?System(?:\*\*)?:?\s*(?:\*(?:.*)\*)?$")
                .map_err(|e| StorageError::ImportError(format!("Regex error: {e}")))?;
        let tool_re = Regex::new(
            r"^(?i)(?:##|###)?\s*(?:\*\*)?(?:Tool Result|Tool)(?:\*\*)?:?\s*(?:\*(?:.*)\*)?$",
        )
        .map_err(|e| StorageError::ImportError(format!("Regex error: {e}")))?;

        let mut title: Option<String> = None;
        let mut current_role: Option<MessageRole> = None;
        let mut current_lines: Vec<String> = Vec::new();
        let mut extracted_turns: Vec<(MessageRole, String)> = Vec::new();

        for line in lines {
            let line_trimmed = line.trim();

            // Extract document title from top-level heading if available
            if title.is_none() {
                if let Some(caps) = title_re.captures(line_trimmed) {
                    if let Some(m) = caps.get(1) {
                        let t = m.as_str().trim();
                        if !t.is_empty() {
                            title = Some(t.to_string());
                            continue;
                        }
                    }
                }
            }

            // Ignore markdown horizontal rule divider
            if line_trimmed == "---" || line_trimmed == "***" || line_trimmed == "___" {
                continue;
            }

            // Ignore header bullets (e.g. "- **Model**: ...", "- **Messages**: ...")
            if current_role.is_none() && line_trimmed.starts_with("- **") {
                continue;
            }

            if user_re.is_match(line_trimmed) {
                if let Some(role) = current_role {
                    let text = current_lines.join("\n").trim().to_string();
                    if !text.is_empty() {
                        extracted_turns.push((role, text));
                    }
                    current_lines.clear();
                }
                current_role = Some(MessageRole::User);
            } else if assistant_re.is_match(line_trimmed) {
                if let Some(role) = current_role {
                    let text = current_lines.join("\n").trim().to_string();
                    if !text.is_empty() {
                        extracted_turns.push((role, text));
                    }
                    current_lines.clear();
                }
                current_role = Some(MessageRole::Assistant);
            } else if system_re.is_match(line_trimmed) {
                if let Some(role) = current_role {
                    let text = current_lines.join("\n").trim().to_string();
                    if !text.is_empty() {
                        extracted_turns.push((role, text));
                    }
                    current_lines.clear();
                }
                current_role = Some(MessageRole::System);
            } else if tool_re.is_match(line_trimmed) {
                if let Some(role) = current_role {
                    let text = current_lines.join("\n").trim().to_string();
                    if !text.is_empty() {
                        extracted_turns.push((role, text));
                    }
                    current_lines.clear();
                }
                current_role = Some(MessageRole::Tool);
            } else if current_role.is_some() {
                current_lines.push(line.to_string());
            } else if !line_trimmed.is_empty() && !line_trimmed.starts_with('#') {
                // If text precedes explicit headers, default to User turn
                current_role = Some(MessageRole::User);
                current_lines.push(line.to_string());
            }
        }

        // Flush remaining buffer
        if let Some(role) = current_role {
            let text = current_lines.join("\n").trim().to_string();
            if !text.is_empty() {
                extracted_turns.push((role, text));
            }
        }

        if extracted_turns.is_empty() {
            // Entire file treated as a single user prompt
            let full_text = trimmed.to_string();
            let mut session =
                SessionRecord::new(Some(generate_session_title(&full_text)), None, None);
            session.add_message(Message::user(&session.metadata.id, full_text));
            return Ok(session);
        }

        let session_title = title.unwrap_or_else(|| {
            extracted_turns
                .iter()
                .find(|(r, _)| *r == MessageRole::User)
                .map(|(_, content)| generate_session_title(content))
                .unwrap_or_else(|| "Imported Markdown Session".to_string())
        });

        let mut session = SessionRecord::new(Some(session_title), None, None);

        for (role, content_text) in extracted_turns {
            let msg = Message {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session.metadata.id.clone(),
                role,
                content: content_text,
                created_at: Utc::now(),
                metadata: MessageMetadata::default(),
            };
            session.add_message(msg);
        }

        Ok(session)
    }

    /// Automatically sniffs the format of a string payload and converts it into a `SessionRecord`.
    pub fn import_from_str(content: &str) -> Result<SessionRecord, StorageError> {
        let format = Self::detect_format(content);
        match format {
            ImportSourceFormat::Hades => Self::import_hades(content),
            ImportSourceFormat::ChatGPT => Self::import_chatgpt(content),
            ImportSourceFormat::Claude => Self::import_claude(content),
            ImportSourceFormat::Markdown => Self::import_markdown(content),
        }
    }

    /// Reads a file from disk and parses it using automatic format detection.
    pub fn import_from_file(path: &Path) -> Result<SessionRecord, StorageError> {
        let content = fs::read_to_string(path).map_err(|e| StorageError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::import_from_str(&content)
    }
}

/// Standalone helper for format sniffing.
pub fn detect_format(content: &str) -> ImportSourceFormat {
    SessionImporter::detect_format(content)
}

/// Standalone helper for importing from a raw string.
pub fn import_from_str(content: &str) -> Result<SessionRecord, StorageError> {
    SessionImporter::import_from_str(content)
}

/// Standalone helper for importing from a file path.
pub fn import_from_file(path: &Path) -> Result<SessionRecord, StorageError> {
    SessionImporter::import_from_file(path)
}

// ---------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const CHATGPT_RAW_JSON: &str = r#"[
      {
        "title": "Async Rust Discussion",
        "create_time": 1709299200.0,
        "mapping": {
          "node-root": {
            "id": "node-root",
            "message": null,
            "parent": null,
            "children": ["node-1"]
          },
          "node-1": {
            "id": "node-1",
            "message": {
              "id": "msg-1",
              "author": { "role": "user", "name": null },
              "create_time": 1709299210.0,
              "content": {
                "content_type": "text",
                "parts": ["How does Tokio work under the hood?"]
              }
            },
            "parent": "node-root",
            "children": ["node-2"]
          },
          "node-2": {
            "id": "node-2",
            "message": {
              "id": "msg-2",
              "author": { "role": "assistant", "name": null },
              "create_time": 1709299220.0,
              "content": {
                "content_type": "text",
                "parts": ["Tokio uses an event loop with non-blocking I/O driven by mio (epoll/kqueue)."]
              }
            },
            "parent": "node-1",
            "children": []
          }
        },
        "current_node": "node-2"
      }
    ]"#;

    const CLAUDE_RAW_JSON: &str = r#"[
      {
        "uuid": "claude-session-42",
        "name": "Design Systems in Ratatui",
        "created_at": "2024-03-01T10:00:00Z",
        "chat_messages": [
          {
            "uuid": "msg-c1",
            "text": "What is the best layout strategy for Ratatui apps?",
            "sender": "human",
            "created_at": "2024-03-01T10:00:05Z"
          },
          {
            "uuid": "msg-c2",
            "text": "Constraint-based vertical and horizontal splits using Layout::default().",
            "sender": "assistant",
            "created_at": "2024-03-01T10:00:15Z"
          }
        ]
      }
    ]"#;

    const MARKDOWN_RAW: &str = r#"# Session: Performance Tuning

- **Model**: groq/llama-3.3-70b-versatile
- **Created**: 2024-03-01 10:00:00 UTC
- **Messages**: 2

---

## User
How do we eliminate allocations in hot paths?

## Assistant
Use stack arrays, SmallVec, or borrow references instead of cloning.
"#;

    #[test]
    fn test_format_detection() {
        assert_eq!(
            SessionImporter::detect_format(CHATGPT_RAW_JSON),
            ImportSourceFormat::ChatGPT
        );
        assert_eq!(
            SessionImporter::detect_format(CLAUDE_RAW_JSON),
            ImportSourceFormat::Claude
        );
        assert_eq!(
            SessionImporter::detect_format(MARKDOWN_RAW),
            ImportSourceFormat::Markdown
        );
    }

    #[test]
    fn test_import_chatgpt_json() {
        let session = SessionImporter::import_chatgpt(CHATGPT_RAW_JSON).expect("parse chatgpt");
        assert_eq!(session.metadata.title, "Async Rust Discussion");
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, MessageRole::User);
        assert_eq!(
            session.messages[0].content,
            "How does Tokio work under the hood?"
        );
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert!(session.messages[1]
            .content
            .contains("Tokio uses an event loop"));
    }

    #[test]
    fn test_import_claude_json() {
        let session = SessionImporter::import_claude(CLAUDE_RAW_JSON).expect("parse claude");
        assert_eq!(session.metadata.title, "Design Systems in Ratatui");
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, MessageRole::User);
        assert_eq!(
            session.messages[0].content,
            "What is the best layout strategy for Ratatui apps?"
        );
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert!(session.messages[1]
            .content
            .contains("Constraint-based vertical and horizontal splits"));
    }

    #[test]
    fn test_import_markdown() {
        let session = SessionImporter::import_markdown(MARKDOWN_RAW).expect("parse markdown");
        assert_eq!(session.metadata.title, "Performance Tuning");
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, MessageRole::User);
        assert_eq!(
            session.messages[0].content,
            "How do we eliminate allocations in hot paths?"
        );
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert_eq!(
            session.messages[1].content,
            "Use stack arrays, SmallVec, or borrow references instead of cloning."
        );
    }

    #[test]
    fn test_roundtrip_hades_export_import() {
        let mut original = SessionRecord::new(
            Some("Roundtrip Test".to_string()),
            Some("groq".to_string()),
            Some("llama-3.3-70b-versatile".to_string()),
        );
        original.add_message(Message::user(&original.metadata.id, "Hello Hades!"));
        original.add_message(Message::assistant(
            &original.metadata.id,
            "Greetings! How may I assist you today?",
            Some("groq".to_string()),
            Some("llama-3.3-70b-versatile".to_string()),
        ));

        let json_data = crate::export::export_to_json(&original);
        assert_eq!(
            SessionImporter::detect_format(&json_data),
            ImportSourceFormat::Hades
        );

        let imported = SessionImporter::import_from_str(&json_data).expect("import hades");
        assert_eq!(imported.metadata.id, original.metadata.id);
        assert_eq!(imported.metadata.title, "Roundtrip Test");
        assert_eq!(imported.messages.len(), 2);
        assert_eq!(imported.messages[0].content, "Hello Hades!");
        assert_eq!(
            imported.messages[1].content,
            "Greetings! How may I assist you today?"
        );
    }
}
