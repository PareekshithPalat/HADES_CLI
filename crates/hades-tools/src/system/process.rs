use async_trait::async_trait;
use serde_json::json;
use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};

/// Tool listing running host processes with resource usage metrics.
pub struct SystemProcessListTool;

#[async_trait]
impl Tool for SystemProcessListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.process.list",
            "Lists active processes running on the host system including PID, process name, memory usage, and CPU consumption.",
            json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of processes to return (default: 40)"
                    },
                    "sort_by": {
                        "type": "string",
                        "enum": ["cpu", "memory", "pid", "name"],
                        "description": "Metric to sort by (default: 'cpu')"
                    }
                },
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
        _context: &ToolContext,
    ) -> ToolResult {
        let mut sys = System::new_all();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(40) as usize;

        let sort_by = input
            .get("sort_by")
            .and_then(|v| v.as_str())
            .unwrap_or("cpu");

        let mut procs: Vec<_> = sys.processes().values().collect();

        match sort_by {
            "memory" => {
                procs.sort_by_key(|b| std::cmp::Reverse(b.memory()));
            }
            "pid" => {
                procs.sort_by_key(|a| a.pid().as_u32());
            }
            "name" => {
                procs.sort_by(|a, b| a.name().to_string_lossy().cmp(&b.name().to_string_lossy()));
            }
            _ => {
                // Default: sort by CPU descending
                procs.sort_by(|a, b| {
                    b.cpu_usage()
                        .partial_cmp(&a.cpu_usage())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        let total_count = procs.len();
        let selected: Vec<_> = procs.into_iter().take(limit).collect();

        let mut output = format!(
            "Active System Processes (showing {} of {} total, sorted by {}):\n",
            selected.len(),
            total_count,
            sort_by
        );
        output.push_str(&format!(
            "{:<8} {:<30} {:>10} {:>10}\n",
            "PID", "NAME", "MEM (MB)", "CPU (%)"
        ));
        output.push_str(&format!("{}\n", "─".repeat(62)));

        for p in selected {
            let pid = p.pid().as_u32();
            let name_str = p.name().to_string_lossy();
            let truncated_name = if name_str.len() > 30 {
                format!("{}...", &name_str[..27])
            } else {
                name_str.to_string()
            };
            let mem_mb = (p.memory() as f64) / (1024.0 * 1024.0);
            let cpu_pct = p.cpu_usage();

            output.push_str(&format!(
                "{:<8} {:<30} {:>9.2}M {:>9.1}%\n",
                pid, truncated_name, mem_mb, cpu_pct
            ));
        }

        ToolResult::success(call_id, "system.process.list", output)
    }
}

/// Tool inspecting detailed runtime attributes of a single process by PID.
pub struct SystemProcessInspectTool;

#[async_trait]
impl Tool for SystemProcessInspectTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.process.inspect",
            "Inspects full execution details of a specific process by its Process ID (PID).",
            json!({
                "type": "object",
                "properties": {
                    "pid": {
                        "type": "integer",
                        "description": "Process ID (PID) to inspect"
                    }
                },
                "required": ["pid"],
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
        _context: &ToolContext,
    ) -> ToolResult {
        let pid_num = match input.get("pid").and_then(|v| v.as_u64()) {
            Some(p) => p as u32,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "system.process.inspect",
                    "Missing required parameter 'pid'",
                );
            }
        };

        let mut sys = System::new_all();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let target_pid = Pid::from_u32(pid_num);
        match sys.process(target_pid) {
            Some(p) => {
                let name = p.name().to_string_lossy();
                let exe_path = p
                    .exe()
                    .map(|e| e.display().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                let parent_pid = p
                    .parent()
                    .map(|pp| pp.as_u32().to_string())
                    .unwrap_or_else(|| "None".to_string());
                let mem_mb = (p.memory() as f64) / (1024.0 * 1024.0);
                let cpu_pct = p.cpu_usage();
                let run_time_secs = p.run_time();
                let status_str = format!("{:?}", p.status());
                let cmdline = p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(" ");

                let output = format!(
                    "Process Inspection (PID: {pid_num}):\n\
                     - Name: {name}\n\
                     - Executable: {exe_path}\n\
                     - Command Line: {cmdline}\n\
                     - Parent PID: {parent_pid}\n\
                     - Status: {status_str}\n\
                     - Memory Usage: {mem_mb:.2} MB\n\
                     - CPU Usage: {cpu_pct:.1}%\n\
                     - Running Time: {run_time_secs} seconds"
                );

                ToolResult::success(call_id, "system.process.inspect", output)
            }
            None => ToolResult::failure(
                call_id,
                "system.process.inspect",
                format!("Process with PID {pid_num} not found or no longer running."),
            ),
        }
    }
}

/// Tool searching running processes by name or command line substring.
pub struct SystemProcessFindTool;

#[async_trait]
impl Tool for SystemProcessFindTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.process.find",
            "Searches for running processes matching a given name or query pattern.",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Process name or search substring (e.g. 'node', 'python', 'chrome')"
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
        _context: &ToolContext,
    ) -> ToolResult {
        let query = match input.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.trim().is_empty() => q.trim().to_lowercase(),
            _ => {
                return ToolResult::invalid_input(
                    call_id,
                    "system.process.find",
                    "Missing required parameter 'query'",
                );
            }
        };

        let mut sys = System::new_all();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let mut matches = Vec::new();
        for p in sys.processes().values() {
            let name = p.name().to_string_lossy().to_lowercase();
            let cmdline = p
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();

            if name.contains(&query) || cmdline.contains(&query) {
                matches.push(p);
            }
        }

        if matches.is_empty() {
            return ToolResult::success(
                call_id,
                "system.process.find",
                format!("No active processes found matching '{query}'."),
            );
        }

        matches.sort_by_key(|a| a.pid().as_u32());

        let mut output = format!("Found {} process(es) matching '{query}':\n", matches.len());
        output.push_str(&format!(
            "{:<8} {:<28} {:>10} {:>10} {}\n",
            "PID", "NAME", "MEM (MB)", "CPU (%)", "COMMAND"
        ));
        output.push_str(&format!("{}\n", "─".repeat(80)));

        for p in matches.into_iter().take(30) {
            let pid = p.pid().as_u32();
            let name_str = p.name().to_string_lossy();
            let truncated_name = if name_str.len() > 28 {
                format!("{}...", &name_str[..25])
            } else {
                name_str.to_string()
            };
            let mem_mb = (p.memory() as f64) / (1024.0 * 1024.0);
            let cpu_pct = p.cpu_usage();
            let cmdline = p
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let truncated_cmd = if cmdline.len() > 30 {
                format!("{}...", &cmdline[..27])
            } else {
                cmdline
            };

            output.push_str(&format!(
                "{:<8} {:<28} {:>9.2}M {:>9.1}% {}\n",
                pid, truncated_name, mem_mb, cpu_pct, truncated_cmd
            ));
        }

        ToolResult::success(call_id, "system.process.find", output)
    }
}

/// Tool terminating a host process by PID (High-Risk, mutating, requires explicit approval).
pub struct SystemProcessTerminateTool;

#[async_trait]
impl Tool for SystemProcessTerminateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.process.terminate",
            "Terminates a running host process by its PID. This is a high-risk mutating operation that requires explicit user approval.",
            json!({
                "type": "object",
                "properties": {
                    "pid": {
                        "type": "integer",
                        "description": "Process ID (PID) to terminate"
                    }
                },
                "required": ["pid"],
                "additionalProperties": false
            }),
            RiskLevel::High,
            true,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        input: serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResult {
        let pid_num = match input.get("pid").and_then(|v| v.as_u64()) {
            Some(p) => p as u32,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "system.process.terminate",
                    "Missing required parameter 'pid'",
                );
            }
        };

        let mut sys = System::new_all();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let target_pid = Pid::from_u32(pid_num);
        match sys.process(target_pid) {
            Some(p) => {
                let name = p.name().to_string_lossy().to_string();
                let killed = p.kill();
                if killed {
                    ToolResult::success(
                        call_id,
                        "system.process.terminate",
                        format!("Successfully terminated process '{name}' (PID: {pid_num})."),
                    )
                } else {
                    ToolResult::failure(
                        call_id,
                        "system.process.terminate",
                        format!("Failed to terminate process '{name}' (PID: {pid_num}). Operation may require elevated/administrator privileges."),
                    )
                }
            }
            None => ToolResult::failure(
                call_id,
                "system.process.terminate",
                format!("Process with PID {pid_num} not found or no longer running."),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_process_list_tool() {
        let ctx = ToolContext::new("test-session", ".", ".");
        let tool = SystemProcessListTool;

        let res = tool
            .execute("p1", json!({ "limit": 10, "sort_by": "memory" }), &ctx)
            .await;
        assert_eq!(res.status, crate::definition::ToolStatus::Success);
        assert!(res.output.contains("Active System Processes"));
        assert!(res.output.contains("PID"));
    }

    #[tokio::test]
    async fn test_system_process_find_and_inspect() {
        let ctx = ToolContext::new("test-session", ".", ".");
        let find_tool = SystemProcessFindTool;

        let current_pid = std::process::id();
        let res = find_tool
            .execute("p2", json!({ "query": "cargo" }), &ctx)
            .await;
        assert_eq!(res.status, crate::definition::ToolStatus::Success);

        let inspect_tool = SystemProcessInspectTool;
        let res2 = inspect_tool
            .execute("p3", json!({ "pid": current_pid }), &ctx)
            .await;
        assert_eq!(res2.status, crate::definition::ToolStatus::Success);
        assert!(res2
            .output
            .contains(&format!("Process Inspection (PID: {current_pid})")));
    }

    #[tokio::test]
    async fn test_system_process_inspect_missing_pid() {
        let ctx = ToolContext::new("test-session", ".", ".");
        let inspect_tool = SystemProcessInspectTool;

        let res = inspect_tool.execute("p4", json!({}), &ctx).await;
        assert_eq!(res.status, crate::definition::ToolStatus::InvalidInput);
    }
}
