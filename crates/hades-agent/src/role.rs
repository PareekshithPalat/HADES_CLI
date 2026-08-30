use serde::{Deserialize, Serialize};

/// Specialized logical agent role defining responsibilities, tool access boundaries, and system instructions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// Breaks down user objectives into structured, dependency-aware plans.
    Planner,
    /// Researches project information, dependencies, and external domain knowledge.
    Researcher,
    /// Analyzes codebase structure, architectural patterns, and performance characteristics.
    Analyst,
    /// Explores repository layouts, finds relevant files, and locates declarations.
    Explorer,
    /// Implements code modifications, bugfixes, refactorings, and features.
    Implementer,
    /// Inspects implementation work, detects edge cases, and verifies correctness.
    Reviewer,
    /// Runs test suites, validates outputs, and diagnoses failures.
    Tester,
    /// Investigates error logs, reproduces bugs, and pinpoints root causes.
    Debugger,
    /// Audits code and configurations for secrets, path escapes, and vulnerabilities.
    SecurityReviewer,
    /// Investigates specific file structures, line ranges, and AST snippets.
    FileInvestigator,
    /// Inspects host platform, processes, networking, and system diagnostics.
    SystemInvestigator,
    /// General multi-disciplinary specialist for composite subtasks.
    GeneralSpecialist,
    /// User-defined or dynamic custom agent role.
    Custom(String),
}

impl AgentRole {
    /// Returns the human-readable display name of this agent role.
    pub fn name(&self) -> &str {
        match self {
            Self::Planner => "Planner",
            Self::Researcher => "Researcher",
            Self::Analyst => "Analyst",
            Self::Explorer => "Explorer",
            Self::Implementer => "Implementer",
            Self::Reviewer => "Reviewer",
            Self::Tester => "Tester",
            Self::Debugger => "Debugger",
            Self::SecurityReviewer => "Security Reviewer",
            Self::FileInvestigator => "File Investigator",
            Self::SystemInvestigator => "System Investigator",
            Self::GeneralSpecialist => "General Specialist",
            Self::Custom(name) => name.as_str(),
        }
    }

    /// Returns a concise description of this role's purpose.
    pub fn description(&self) -> &str {
        match self {
            Self::Planner => "Analyzes complex requests and decomposes them into executable tasks.",
            Self::Researcher => {
                "Gathers external documentation, dependencies, and project context."
            }
            Self::Analyst => {
                "Examines system architecture, structural dependencies, and data flow."
            }
            Self::Explorer => {
                "Locates relevant files, identifiers, and configuration assets in the workspace."
            }
            Self::Implementer => "Authoritatively writes, modifies, and refactors source code.",
            Self::Reviewer => {
                "Conducts rigorous code reviews, verifying logic and detecting defects."
            }
            Self::Tester => "Executes test suites and validates runtime assertions.",
            Self::Debugger => "Analyzes stack traces, reproduces errors, and isolates root causes.",
            Self::SecurityReviewer => {
                "Audits for sensitive data leakage, path traversal, and unsafe operations."
            }
            Self::FileInvestigator => {
                "Performs surgical line-by-line inspection of targeted source files."
            }
            Self::SystemInvestigator => {
                "Gathers system telemetry, active process states, and network metrics."
            }
            Self::GeneralSpecialist => "Performs focused multi-step investigation and reasoning.",
            Self::Custom(_) => "Custom specialized domain agent.",
        }
    }

    /// Returns the default primary objective for this agent role.
    pub fn default_objective(&self) -> &str {
        match self {
            Self::Planner => "Analyze the user objective and construct an optimal execution plan.",
            Self::Researcher => {
                "Gather all necessary background context and technical documentation."
            }
            Self::Analyst => "Provide deep architectural and algorithmic analysis of the code.",
            Self::Explorer => "Map the workspace and discover relevant source files.",
            Self::Implementer => "Safely implement the requested changes within workspace bounds.",
            Self::Reviewer => "Review the proposed code changes for bugs, regressions, and style.",
            Self::Tester => "Execute automated tests and verify behavioral invariants.",
            Self::Debugger => "Pinpoint the exact cause of the failure and suggest fixes.",
            Self::SecurityReviewer => {
                "Verify that no secrets, unsafe patterns, or security risks exist."
            }
            Self::FileInvestigator => "Examine the designated files and extract critical details.",
            Self::SystemInvestigator => {
                "Inspect host system environment, platform, and runtime health."
            }
            Self::GeneralSpecialist => "Execute the assigned specialized objective accurately.",
            Self::Custom(_) => "Execute custom specialized task.",
        }
    }

    /// Returns whether this agent role is authorized to perform mutating operations (e.g. file writing).
    pub fn is_mutating_allowed(&self) -> bool {
        match self {
            Self::Implementer | Self::GeneralSpecialist => true,
            Self::Planner
            | Self::Researcher
            | Self::Analyst
            | Self::Explorer
            | Self::Reviewer
            | Self::Tester
            | Self::Debugger
            | Self::SecurityReviewer
            | Self::FileInvestigator
            | Self::SystemInvestigator => false,
            Self::Custom(_) => false,
        }
    }

    /// Returns tool pattern whitelist prefixes recommended for this agent role.
    pub fn allowed_tool_patterns(&self) -> Vec<&'static str> {
        match self {
            Self::Planner => vec!["workspace.detect", "workspace.inspect", "filesystem.list"],
            Self::Researcher => vec![
                "workspace.*",
                "filesystem.read",
                "filesystem.list",
                "system.runtime.*",
            ],
            Self::Analyst => vec!["workspace.*", "filesystem.read", "filesystem.list"],
            Self::Explorer => vec!["workspace.*", "filesystem.list", "filesystem.read"],
            Self::Implementer => vec![
                "filesystem.*",
                "workspace.*",
                "shell.execute",
                "environment.*",
            ],
            Self::Reviewer => vec!["workspace.*", "filesystem.read", "filesystem.list"],
            Self::Tester => vec![
                "shell.execute",
                "filesystem.read",
                "filesystem.list",
                "system.runtime.*",
            ],
            Self::Debugger => vec![
                "filesystem.read",
                "filesystem.list",
                "shell.execute",
                "system.process.*",
                "system.runtime.*",
            ],
            Self::SecurityReviewer => vec![
                "filesystem.read",
                "filesystem.list",
                "workspace.*",
                "environment.list",
                "system.network.*",
            ],
            Self::FileInvestigator => vec!["filesystem.read", "filesystem.list", "workspace.*"],
            Self::SystemInvestigator => vec!["system.*", "environment.*", "shell.execute"],
            Self::GeneralSpecialist => vec!["filesystem.*", "workspace.*", "system.*", "shell.*"],
            Self::Custom(_) => vec!["workspace.*", "filesystem.read", "filesystem.list"],
        }
    }

    /// Returns the specialized role-specific instruction for prompt construction.
    pub fn system_instruction(&self) -> &str {
        match self {
            Self::Planner => {
                "You are the PLANNER subagent. Your responsibility is to analyze requirements, identify dependencies, and formulate structured task plans. Do not perform direct code mutations unless explicitly asked."
            }
            Self::Researcher => {
                "You are the RESEARCHER subagent. Your responsibility is to gather relevant project information, dependency structures, and domain knowledge. Provide clear, factual findings."
            }
            Self::Analyst => {
                "You are the ANALYST subagent. Your responsibility is to inspect architectures, data flows, and code patterns to provide precise diagnostic assessments."
            }
            Self::Explorer => {
                "You are the EXPLORER subagent. Your responsibility is to search the workspace, locate relevant files, and identify declarations. Focus on accurate path and symbol discovery."
            }
            Self::Implementer => {
                "You are the IMPLEMENTER subagent. Your responsibility is to write high-quality, robust, and clean code modifications within workspace boundaries adhering to project conventions."
            }
            Self::Reviewer => {
                "You are the REVIEWER subagent. Your responsibility is to inspect code changes, verify logic, check for edge cases, and ensure no regressions or style violations were introduced."
            }
            Self::Tester => {
                "You are the TESTER subagent. Your responsibility is to run automated test commands, observe stdout/stderr, and report exact test statuses, failures, and coverage."
            }
            Self::Debugger => {
                "You are the DEBUGGER subagent. Your responsibility is to trace errors, analyze failure logs, and isolate the exact root cause of bugs with actionable remediations."
            }
            Self::SecurityReviewer => {
                "You are the SECURITY REVIEWER subagent. Your responsibility is to audit code for leaked secrets, path traversal vulnerabilities, insecure commands, and permission misconfigurations."
            }
            Self::FileInvestigator => {
                "You are the FILE INVESTIGATOR subagent. Your responsibility is to carefully read and analyze designated source files, extracting critical sections and symbols."
            }
            Self::SystemInvestigator => {
                "You are the SYSTEM INVESTIGATOR subagent. Your responsibility is to inspect host operating system health, runtime versions, process states, and network connectivity."
            }
            Self::GeneralSpecialist => {
                "You are a GENERAL SPECIALIST subagent. Your responsibility is to perform the designated task accurately, concisely, and safely within bounds."
            }
            Self::Custom(_) => {
                "You are a CUSTOM specialized subagent. Execute the designated task accurately and report structured conclusions back to the primary Hades orchestrator."
            }
        }
    }

    /// Returns default execution timeout duration in seconds for this role.
    pub fn default_timeout_secs(&self) -> u64 {
        match self {
            Self::Planner => 30,
            Self::Researcher => 45,
            Self::Analyst => 45,
            Self::Explorer => 30,
            Self::Implementer => 90,
            Self::Reviewer => 45,
            Self::Tester => 120,
            Self::Debugger => 60,
            Self::SecurityReviewer => 45,
            Self::FileInvestigator => 30,
            Self::SystemInvestigator => 30,
            Self::GeneralSpecialist => 60,
            Self::Custom(_) => 45,
        }
    }
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
