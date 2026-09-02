// crates/hades-storage/src/export.rs

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::StorageError;
use crate::model::{MessageRole, SessionRecord};

/// Supported export serialization formats for session transcripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Markdown,
    Json,
}

impl FromStr for ExportFormat {
    type Err = StorageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "md" | "markdown" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            other => Err(StorageError::UnsupportedFormat(format!(
                "Unsupported export format '{other}'. Supported formats: markdown (md), json"
            ))),
        }
    }
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Markdown => write!(f, "markdown"),
            Self::Json => write!(f, "json"),
        }
    }
}

/// Exporter responsible for serializing conversation sessions into various export formats.
pub struct SessionExporter;

impl SessionExporter {
    /// Generates a clean, formatted Markdown document from a session record.
    pub fn export_to_markdown(session: &SessionRecord) -> String {
        let mut out = String::with_capacity(2048);

        // Header metadata block
        out.push_str(&format!("# Session: {}\n\n", session.metadata.title));
        out.push_str(&format!("- **Session ID**: `{}`\n", session.metadata.id));

        let model_display = match (
            &session.metadata.active_provider,
            &session.metadata.active_model,
        ) {
            (Some(prov), Some(model)) => format!("{prov}/{model}"),
            (None, Some(model)) => model.clone(),
            _ => "Not configured".to_string(),
        };
        out.push_str(&format!("- **Model**: {model_display}\n"));
        out.push_str(&format!(
            "- **Created**: {}\n",
            session.metadata.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        out.push_str(&format!(
            "- **Exported**: {}\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        out.push_str(&format!(
            "- **Messages**: {}\n",
            session.metadata.message_count
        ));
        if session.metadata.total_tokens > 0 {
            out.push_str(&format!(
                "- **Total Tokens**: {} (Input: {}, Output: {})\n",
                session.metadata.total_tokens,
                session.metadata.total_input_tokens,
                session.metadata.total_output_tokens
            ));
        }

        out.push_str("\n---\n\n");

        // Message transcript turns
        for message in &session.messages {
            let role_header = match message.role {
                MessageRole::User => "## User",
                MessageRole::Assistant => "## Assistant",
                MessageRole::System => "## System",
                MessageRole::Tool => "## Tool Result",
                MessageRole::Error => "## Error",
            };

            out.push_str(role_header);

            if let Some(ref model) = message.metadata.model {
                out.push_str(&format!(" *(model: {model})*"));
            }
            out.push_str("\n\n");

            out.push_str(message.content.trim());
            out.push_str("\n\n");

            if let Some(ref tool_calls) = message.metadata.tool_calls {
                out.push_str("```json\n");
                out.push_str(tool_calls.trim());
                out.push_str("\n```\n\n");
            }
        }

        out
    }

    /// Serializes the complete session record into pretty-printed JSON.
    pub fn export_to_json(session: &SessionRecord) -> String {
        serde_json::to_string_pretty(session)
            .unwrap_or_else(|e| format!(r#"{{"error": "Failed to serialize session: {}"}}"#, e))
    }

    /// Writes the exported session transcript to the specified target path.
    pub fn save_export(
        session: &SessionRecord,
        format: ExportFormat,
        target_path: &Path,
    ) -> Result<PathBuf, StorageError> {
        let content = match format {
            ExportFormat::Markdown => Self::export_to_markdown(session),
            ExportFormat::Json => Self::export_to_json(session),
        };

        if let Some(parent) = target_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| StorageError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
        }

        fs::write(target_path, content.as_bytes()).map_err(|e| StorageError::Io {
            path: target_path.to_path_buf(),
            source: e,
        })?;

        Ok(target_path.to_path_buf())
    }
}

/// Standalone helper to generate Markdown export.
pub fn export_to_markdown(session: &SessionRecord) -> String {
    SessionExporter::export_to_markdown(session)
}

/// Standalone helper to generate JSON export.
pub fn export_to_json(session: &SessionRecord) -> String {
    SessionExporter::export_to_json(session)
}

/// Standalone helper to save session export to a file.
pub fn save_export(
    session: &SessionRecord,
    format: ExportFormat,
    target_path: &Path,
) -> Result<PathBuf, StorageError> {
    SessionExporter::save_export(session, format, target_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Message, MessageRole};
    use tempfile::tempdir;

    fn create_test_session() -> SessionRecord {
        let mut session = SessionRecord::new(
            Some("Quantum Architecture".to_string()),
            Some("groq".to_string()),
            Some("llama-3.3-70b-versatile".to_string()),
        );

        let msg1 = Message::user(&session.metadata.id, "Explain quantum superposition.");
        session.add_message(msg1);

        let mut msg2 = Message::assistant(
            &session.metadata.id,
            "Superposition allows a system to be in multiple states at once.",
            Some("groq".to_string()),
            Some("llama-3.3-70b-versatile".to_string()),
        );
        msg2.metadata.input_tokens = Some(10);
        msg2.metadata.output_tokens = Some(20);
        msg2.metadata.total_tokens = Some(30);
        session.add_message(msg2);

        session
    }

    #[test]
    fn test_export_to_markdown_structure() {
        let session = create_test_session();
        let md = export_to_markdown(&session);

        assert!(md.contains("# Session: Quantum Architecture"));
        assert!(md.contains("- **Model**: groq/llama-3.3-70b-versatile"));
        assert!(md.contains("- **Messages**: 2"));
        assert!(md.contains("## User"));
        assert!(md.contains("Explain quantum superposition."));
        assert!(md.contains("## Assistant"));
        assert!(md.contains("Superposition allows a system to be in multiple states at once."));
    }

    #[test]
    fn test_export_to_json_validity() {
        let session = create_test_session();
        let json_str = export_to_json(&session);

        let deserialized: SessionRecord =
            serde_json::from_str(&json_str).expect("Valid session JSON");
        assert_eq!(deserialized.metadata.title, "Quantum Architecture");
        assert_eq!(deserialized.messages.len(), 2);
        assert_eq!(deserialized.messages[0].role, MessageRole::User);
        assert_eq!(deserialized.messages[1].role, MessageRole::Assistant);
    }

    #[test]
    fn test_save_export_to_disk() {
        let session = create_test_session();
        let dir = tempdir().expect("temp dir");

        let md_path = dir.path().join("exports").join("session.md");
        let saved_md =
            save_export(&session, ExportFormat::Markdown, &md_path).expect("save markdown");
        assert_eq!(saved_md, md_path);
        assert!(md_path.exists());

        let read_md = fs::read_to_string(&md_path).expect("read md");
        assert!(read_md.contains("Quantum Architecture"));

        let json_path = dir.path().join("exports").join("session.json");
        let saved_json = save_export(&session, ExportFormat::Json, &json_path).expect("save json");
        assert_eq!(saved_json, json_path);
        assert!(json_path.exists());

        let read_json = fs::read_to_string(&json_path).expect("read json");
        let parsed: SessionRecord = serde_json::from_str(&read_json).expect("parse json");
        assert_eq!(parsed.metadata.title, "Quantum Architecture");
    }

    #[test]
    fn test_export_format_from_str() {
        assert_eq!(
            "md".parse::<ExportFormat>().unwrap(),
            ExportFormat::Markdown
        );
        assert_eq!(
            "markdown".parse::<ExportFormat>().unwrap(),
            ExportFormat::Markdown
        );
        assert_eq!("json".parse::<ExportFormat>().unwrap(), ExportFormat::Json);
        assert!("invalid".parse::<ExportFormat>().is_err());
    }
}
