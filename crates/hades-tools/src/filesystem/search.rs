use std::fs;
use std::path::Path;

use async_trait::async_trait;
use regex::{Regex, RegexBuilder};
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::path::PathSecurity;

const TOOL_NAME: &str = "filesystem.search";
const DEFAULT_MAX_RESULTS: u64 = 30;
const MAX_RESULTS_CAP: u64 = 200;
/// Files larger than this are skipped to keep searches fast and bounded.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Upper bound on files inspected per search.
const MAX_FILES_SCANNED: usize = 20_000;
/// Matched lines are clipped to this many characters in the output.
const MAX_SNIPPET_CHARS: usize = 200;
/// Directories that are almost always generated or vendored and never worth searching.
const SKIPPED_DIRS: &[&str] = &["node_modules", "target", "__pycache__", "venv"];

/// Tool for searching file contents across the workspace with a regex or literal pattern.
pub struct FileSystemSearchTool;

#[async_trait]
impl Tool for FileSystemSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Searches file contents within the workspace for a regex or literal pattern and returns matching files with line numbers and line snippets. Skips hidden, binary, oversized and sensitive files, and generated directories (node_modules, target, __pycache__, venv).",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Regular expression (or literal text when 'literal' is true) to match against each line."
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory or file to search, relative to the workspace (default: '.')."
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of matching lines to return (default: 30, max: 200)."
                    },
                    "case_sensitive": {
                        "type": "boolean",
                        "description": "Whether matching is case sensitive (default: false)."
                    },
                    "literal": {
                        "type": "boolean",
                        "description": "Treat 'query' as plain text instead of a regular expression (default: false)."
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> ToolResult {
        let query = match input.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q,
            _ => {
                return ToolResult::invalid_input(
                    call_id,
                    TOOL_NAME,
                    "Missing required non-empty 'query' parameter",
                )
            }
        };
        let path_str = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_MAX_RESULTS)
            .clamp(1, MAX_RESULTS_CAP) as usize;
        let case_sensitive = input
            .get("case_sensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let literal = input
            .get("literal")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let pattern = if literal {
            regex::escape(query)
        } else {
            query.to_string()
        };
        let matcher = match RegexBuilder::new(&pattern)
            .case_insensitive(!case_sensitive)
            .size_limit(1 << 20)
            .build()
        {
            Ok(re) => re,
            Err(e) => {
                return ToolResult::invalid_input(
                    call_id,
                    TOOL_NAME,
                    format!("Invalid regular expression '{query}': {e}. Set 'literal' to true to search for plain text."),
                )
            }
        };

        let root = match PathSecurity::resolve_path(&context.working_directory, path_str) {
            Ok(p) => p,
            Err(e) => return ToolResult::permission_denied(call_id, TOOL_NAME, e.to_string()),
        };

        // Hard sandbox: unlike read-only tools that merely raise the risk level, search
        // walks whole trees, so anything outside the workspace is refused outright.
        if !PathSecurity::is_inside_boundary(&root, &context.workspace_root) {
            return ToolResult::permission_denied(
                call_id,
                TOOL_NAME,
                format!(
                    "Search path '{}' is outside the workspace boundary",
                    root.display()
                ),
            );
        }

        if !root.exists() {
            return ToolResult::failure(
                call_id,
                TOOL_NAME,
                format!("Path does not exist: {}", root.display()),
            );
        }

        let mut search = Search {
            matcher: &matcher,
            base: if root.is_dir() {
                root.clone()
            } else {
                root.parent().map(Path::to_path_buf).unwrap_or_default()
            },
            max_results,
            matches: Vec::new(),
            files_scanned: 0,
            files_matched: 0,
            truncated: false,
            context,
        };

        if root.is_dir() {
            search.walk(&root);
        } else {
            search.search_file(&root);
        }

        let mut output = format!(
            "Search for '{query}' in {}: {} match(es) in {} file(s) ({} file(s) scanned)",
            root.display(),
            search.matches.len(),
            search.files_matched,
            search.files_scanned
        );
        if search.truncated {
            output.push_str(&format!(" (truncated at {max_results} results)"));
        }
        output.push('\n');
        if search.matches.is_empty() {
            output.push_str("\nNo matches found.\n");
        } else {
            output.push('\n');
            for line in &search.matches {
                output.push_str(line);
                output.push('\n');
            }
        }

        ToolResult::success(call_id, TOOL_NAME, output)
            .with_metadata(json!({
                "match_count": search.matches.len(),
                "files_matched": search.files_matched,
                "files_scanned": search.files_scanned,
                "is_truncated": search.truncated,
                "path": root.display().to_string(),
            }))
            .with_truncation(search.truncated)
    }
}

struct Search<'a> {
    matcher: &'a Regex,
    base: std::path::PathBuf,
    max_results: usize,
    matches: Vec<String>,
    files_scanned: usize,
    files_matched: usize,
    truncated: bool,
    context: &'a ToolContext,
}

impl Search<'_> {
    fn should_stop(&mut self) -> bool {
        if self.matches.len() >= self.max_results || self.files_scanned >= MAX_FILES_SCANNED {
            self.truncated = true;
            return true;
        }
        self.context.is_cancelled()
    }

    fn walk(&mut self, dir: &Path) {
        let Ok(read_dir) = fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = read_dir.flatten().collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            if self.should_stop() {
                return;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            // `DirEntry::file_type` does not follow symlinks; skipping them prevents
            // escaping the workspace through a link that points outside it.
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                if !SKIPPED_DIRS.contains(&name.as_ref()) {
                    self.walk(&path);
                }
            } else if file_type.is_file() {
                self.search_file(&path);
            }
        }
    }

    fn search_file(&mut self, path: &Path) {
        if PathSecurity::is_sensitive_path(path) {
            return;
        }
        match fs::metadata(path) {
            Ok(m) if m.len() <= MAX_FILE_BYTES => {}
            _ => return,
        }
        let Ok(bytes) = fs::read(path) else {
            return;
        };
        self.files_scanned += 1;
        if bytes.iter().take(8192).any(|b| *b == 0) {
            return; // binary file
        }
        let Ok(text) = std::str::from_utf8(&bytes) else {
            return;
        };

        let relative = path
            .strip_prefix(&self.base)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let mut matched = false;
        for (index, line) in text.lines().enumerate() {
            if self.matches.len() >= self.max_results {
                self.truncated = true;
                break;
            }
            if self.matcher.is_match(line) {
                matched = true;
                self.matches
                    .push(format!("{relative}:{}: {}", index + 1, snippet(line)));
            }
        }
        if matched {
            self.files_matched += 1;
        }
    }
}

fn snippet(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.chars().count() <= MAX_SNIPPET_CHARS {
        trimmed.to_string()
    } else {
        let clipped: String = trimmed.chars().take(MAX_SNIPPET_CHARS).collect();
        format!("{clipped}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::ToolStatus;
    use tempfile::tempdir;

    fn workspace() -> tempfile::TempDir {
        let dir = tempdir().expect("temp dir");
        let root = dir.path();
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::write(
            root.join("src/main.rs"),
            "fn main() {\n    run_server();\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("src/nested/server.rs"),
            "pub fn run_server() {}\n// TODO: Run_Server docs\n",
        )
        .unwrap();
        fs::write(root.join("notes.txt"), "price is $5 (approx)\n").unwrap();
        dir
    }

    async fn search(dir: &Path, input: serde_json::Value) -> ToolResult {
        let ctx = ToolContext::new("s", dir, dir);
        FileSystemSearchTool.execute("call", input, &ctx).await
    }

    #[tokio::test]
    async fn test_finds_matches_with_line_numbers_and_snippets() {
        let dir = workspace();
        let res = search(
            dir.path(),
            json!({ "query": "run_server", "case_sensitive": true }),
        )
        .await;

        assert_eq!(res.status, ToolStatus::Success);
        assert!(res.output.contains("src/main.rs:2: run_server();"));
        assert!(res
            .output
            .contains("src/nested/server.rs:1: pub fn run_server() {}"));
        assert!(!res.output.contains("Run_Server"));
        assert_eq!(res.metadata["match_count"], 2);
        assert_eq!(res.metadata["files_matched"], 2);
    }

    #[tokio::test]
    async fn test_case_insensitive_by_default() {
        let dir = workspace();
        let res = search(dir.path(), json!({ "query": "run_server" })).await;
        assert_eq!(res.metadata["match_count"], 3);
        assert!(res.output.contains("src/nested/server.rs:2:"));
    }

    #[tokio::test]
    async fn test_regex_and_literal_modes() {
        let dir = workspace();
        let regex = search(dir.path(), json!({ "query": r"fn \w+_server" })).await;
        assert_eq!(regex.metadata["match_count"], 1);

        let literal = search(
            dir.path(),
            json!({ "query": "$5 (approx)", "literal": true }),
        )
        .await;
        assert_eq!(literal.metadata["match_count"], 1);
        assert!(literal.output.contains("notes.txt:1:"));

        let invalid = search(dir.path(), json!({ "query": "fn (" })).await;
        assert_eq!(invalid.status, ToolStatus::InvalidInput);
    }

    #[tokio::test]
    async fn test_respects_max_results_and_subdirectory_path() {
        let dir = workspace();
        let limited = search(dir.path(), json!({ "query": "server", "max_results": 1 })).await;
        assert_eq!(limited.metadata["match_count"], 1);
        assert!(limited.is_truncated);

        let scoped = search(
            dir.path(),
            json!({ "query": "server", "path": "src/nested" }),
        )
        .await;
        assert_eq!(scoped.metadata["files_matched"], 1);
        assert!(scoped.output.contains("server.rs:1:"));
        assert!(!scoped.output.contains("main.rs"));
    }

    #[tokio::test]
    async fn test_blocks_paths_outside_workspace() {
        let outer = tempdir().expect("outer");
        let workspace = outer.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(outer.path().join("secret.txt"), "needle\n").unwrap();

        let traversal = search(&workspace, json!({ "query": "needle", "path": "../" })).await;
        assert_eq!(traversal.status, ToolStatus::PermissionDenied);

        let absolute = search(
            &workspace,
            json!({ "query": "needle", "path": outer.path().to_string_lossy() }),
        )
        .await;
        assert_eq!(absolute.status, ToolStatus::PermissionDenied);
        assert!(!absolute.output.contains("needle"));
    }

    #[tokio::test]
    async fn test_skips_hidden_generated_sensitive_and_binary_files() {
        let dir = workspace();
        let root = dir.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), "needle\n").unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), "needle\n").unwrap();
        fs::write(root.join("deploy.pem"), "needle\n").unwrap();
        fs::write(root.join("blob.bin"), b"needle\0\x01\x02").unwrap();
        fs::write(root.join("visible.md"), "needle\n").unwrap();

        let res = search(root, json!({ "query": "needle" })).await;
        assert_eq!(res.metadata["match_count"], 1);
        assert!(res.output.contains("visible.md:1: needle"));
    }

    #[tokio::test]
    async fn test_rejects_missing_query() {
        let dir = workspace();
        let res = search(dir.path(), json!({ "path": "." })).await;
        assert_eq!(res.status, ToolStatus::InvalidInput);
    }
}
