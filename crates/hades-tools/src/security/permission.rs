use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::context::ToolContext;
use crate::definition::{RiskLevel, ToolCall, ToolDefinition};
use crate::security::path::PathSecurity;

/// User or policy authorization decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApprovalDecision {
    /// Allow this single execution only.
    AllowOnce,
    /// Allow all subsequent invocations of this tool within the current session.
    AllowSession,
    /// Deny this tool execution request.
    Deny,
    /// Cancel the entire operation / prompt flow.
    Cancel,
}

impl std::fmt::Display for ApprovalDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllowOnce => write!(f, "ALLOW_ONCE"),
            Self::AllowSession => write!(f, "ALLOW_SESSION"),
            Self::Deny => write!(f, "DENY"),
            Self::Cancel => write!(f, "CANCEL"),
        }
    }
}

/// Evaluation verdict produced by the PermissionEngine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationResult {
    /// Safe to execute immediately without prompting the user.
    Permitted { risk: RiskLevel },
    /// Requires explicit interactive user approval before proceeding.
    RequiresApproval {
        risk: RiskLevel,
        summary: String,
        details: String,
    },
    /// Hard denial - operation violates invariant safety constraints.
    Denied { reason: String },
}

/// Dynamic permission engine evaluating tool calls against safety policies and session authorizations.
#[derive(Debug, Clone)]
pub struct PermissionEngine {
    /// Set of tool names explicitly allowed for the duration of the current session.
    session_allowed_tools: HashSet<String>,
}

impl Default for PermissionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionEngine {
    /// Creates a fresh permission engine with clean session state.
    pub fn new() -> Self {
        Self {
            session_allowed_tools: HashSet::new(),
        }
    }

    /// Records a session-wide authorization decision.
    pub fn grant_session_permission(&mut self, tool_name: &str) {
        self.session_allowed_tools.insert(tool_name.to_string());
    }

    /// Clears all session authorizations.
    pub fn clear_session_permissions(&mut self) {
        self.session_allowed_tools.clear();
    }

    /// Evaluates a tool call against the security boundary and current permission rules.
    pub fn evaluate(
        &self,
        call: &ToolCall,
        definition: &ToolDefinition,
        context: &ToolContext,
    ) -> EvaluationResult {
        let mut computed_risk = definition.risk_level;

        // 1. Analyze path arguments if tool involves files or directories
        if let Some(target_path_str) = call.arguments.get("path").and_then(|v| v.as_str()) {
            match PathSecurity::resolve_path(&context.working_directory, target_path_str) {
                Ok(resolved_path) => {
                    // Check system path protection
                    if PathSecurity::is_system_path(&resolved_path) {
                        if definition.is_mutating {
                            return EvaluationResult::Denied {
                                reason: format!(
                                    "Modifications to operating system directory '{}' are prohibited.",
                                    resolved_path.display()
                                ),
                            };
                        }
                        computed_risk = RiskLevel::Critical;
                    }

                    // Check sensitive file protection (.env, private keys, etc.)
                    if PathSecurity::is_sensitive_path(&resolved_path) {
                        computed_risk = RiskLevel::Critical;
                    }

                    // Check workspace boundary
                    if !PathSecurity::is_inside_boundary(&resolved_path, &context.workspace_root) {
                        computed_risk = if definition.is_mutating {
                            RiskLevel::Critical
                        } else {
                            RiskLevel::High
                        };
                    }
                }
                Err(e) => {
                    return EvaluationResult::Denied {
                        reason: format!("Path resolution error: {e}"),
                    };
                }
            }
        }

        // 2. Analyze shell/process command arguments
        if call.tool_name == "shell.execute" || call.tool_name == "process.start" {
            let executable = call
                .arguments
                .get("executable")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            computed_risk = Self::classify_command_risk(executable);
        }

        // 3. Recursive deletion is always Critical
        if call.tool_name == "filesystem.delete"
            && call
                .arguments
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            computed_risk = RiskLevel::Critical;
        }

        // 4. Check if tool has session-wide grant (only applicable to Low/Medium/High, NEVER Critical)
        if computed_risk < RiskLevel::Critical
            && self.session_allowed_tools.contains(&call.tool_name)
        {
            return EvaluationResult::Permitted {
                risk: computed_risk,
            };
        }

        // 5. Apply default conservative policy
        match computed_risk {
            RiskLevel::Safe => EvaluationResult::Permitted {
                risk: RiskLevel::Safe,
            },
            RiskLevel::Low => {
                // Safe inside workspace
                EvaluationResult::Permitted {
                    risk: RiskLevel::Low,
                }
            }
            RiskLevel::Medium => {
                let summary = format!("Execute tool '{}'", call.tool_name);
                let details = Self::format_call_details(call, context);
                EvaluationResult::RequiresApproval {
                    risk: RiskLevel::Medium,
                    summary,
                    details,
                }
            }
            RiskLevel::High => {
                let summary = format!("Execute high-risk tool '{}'", call.tool_name);
                let details = Self::format_call_details(call, context);
                EvaluationResult::RequiresApproval {
                    risk: RiskLevel::High,
                    summary,
                    details,
                }
            }
            RiskLevel::Critical => {
                let summary = format!("CRITICAL: Execute sensitive tool '{}'", call.tool_name);
                let details = Self::format_call_details(call, context);
                EvaluationResult::RequiresApproval {
                    risk: RiskLevel::Critical,
                    summary,
                    details,
                }
            }
        }
    }

    fn classify_command_risk(executable: &str) -> RiskLevel {
        let exe_lower = executable.to_lowercase();
        let name = Path::new(&exe_lower)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&exe_lower);

        // Low-risk read-only commands
        if name == "git"
            || name == "cargo"
            || name == "npm"
            || name == "python"
            || name == "node"
            || name == "rustc"
            || name == "go"
            || name == "pytest"
            || name == "echo"
        {
            RiskLevel::Medium
        } else if name == "rm"
            || name == "del"
            || name == "rmdir"
            || name == "format"
            || name == "shutdown"
            || name == "reboot"
            || name == "reg"
            || name == "sudo"
            || name == "chmod"
            || name == "chown"
            || name == "mkfs"
            || name == "dd"
        {
            RiskLevel::Critical
        } else {
            RiskLevel::High
        }
    }

    fn format_call_details(call: &ToolCall, context: &ToolContext) -> String {
        let mut details = format!("Tool: {}\n", call.tool_name);
        if let Some(path) = call.arguments.get("path").and_then(|v| v.as_str()) {
            details.push_str(&format!("Target Path: {path}\n"));
            let is_inside =
                PathSecurity::is_inside_boundary(Path::new(path), &context.workspace_root);
            details.push_str(&format!("Inside Workspace: {is_inside}\n"));
        }
        if let Some(exe) = call.arguments.get("executable").and_then(|v| v.as_str()) {
            details.push_str(&format!("Executable: {exe}\n"));
            if let Some(args) = call.arguments.get("arguments").and_then(|v| v.as_array()) {
                let args_str = args
                    .iter()
                    .map(|a| a.as_str().unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(" ");
                details.push_str(&format!("Arguments: {args_str}\n"));
            }
            details.push_str(&format!(
                "Directory: {}\n",
                context.working_directory.display()
            ));
        }
        details
    }
}
