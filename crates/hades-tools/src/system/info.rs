use async_trait::async_trait;
use serde_json::json;
use sysinfo::System;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};

/// Formats seconds into human-readable duration (e.g. "3 days, 4 hours, 12 minutes, 5 seconds").
pub fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days} day{}", if days == 1 { "" } else { "s" }));
    }
    if hours > 0 {
        parts.push(format!("{hours} hour{}", if hours == 1 { "" } else { "s" }));
    }
    if minutes > 0 {
        parts.push(format!(
            "{minutes} minute{}",
            if minutes == 1 { "" } else { "s" }
        ));
    }
    parts.push(format!("{secs} second{}", if secs == 1 { "" } else { "s" }));

    parts.join(", ")
}

/// Tool providing comprehensive system diagnostic information (OS, CPU, memory, uptime, hostname).
pub struct SystemInfoTool;

#[async_trait]
impl Tool for SystemInfoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.info",
            "Returns a comprehensive diagnostic overview of the host machine including OS name, kernel, architecture, hostname, uptime, memory, and CPU core counts.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        _input: serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResult {
        let mut sys = System::new_all();
        sys.refresh_all();

        let os_name = System::name().unwrap_or_else(|| std::env::consts::OS.to_string());
        let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
        let hostname = System::host_name().unwrap_or_else(|| "Unknown".to_string());
        let arch = std::env::consts::ARCH;
        let uptime_secs = System::uptime();
        let uptime_str = format_uptime(uptime_secs);

        let total_mem_mb = sys.total_memory() / (1024 * 1024);
        let used_mem_mb = sys.used_memory() / (1024 * 1024);
        let avail_mem_mb = sys.available_memory() / (1024 * 1024);
        let cpu_count = sys.cpus().len();

        let output = format!(
            "System Diagnostic Overview:\n\
             - Operating System: {os_name} (Version: {os_version}, Kernel: {kernel_version})\n\
             - Architecture: {arch}\n\
             - Hostname: {hostname}\n\
             - Uptime: {uptime_str} ({uptime_secs}s)\n\
             - CPU Cores: {cpu_count}\n\
             - Total Memory: {total_mem_mb} MB\n\
             - Used Memory: {used_mem_mb} MB\n\
             - Available Memory: {avail_mem_mb} MB"
        );

        ToolResult::success(call_id, "system.info", output)
    }
}

/// Tool reporting operating system platform and kernel version.
pub struct SystemPlatformTool;

#[async_trait]
impl Tool for SystemPlatformTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.platform",
            "Reports the operating system platform name, release version, and kernel version.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        _input: serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResult {
        let os_name = System::name().unwrap_or_else(|| std::env::consts::OS.to_string());
        let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());

        let output = format!(
            "Platform: {os_name}\n\
             Version: {os_version}\n\
             Kernel: {kernel_version}\n\
             Family: {}",
            std::env::consts::FAMILY
        );

        ToolResult::success(call_id, "system.platform", output)
    }
}

/// Tool reporting CPU architecture.
pub struct SystemArchitectureTool;

#[async_trait]
impl Tool for SystemArchitectureTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.architecture",
            "Reports the CPU instruction set architecture of the host machine.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        _input: serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResult {
        let output = format!(
            "CPU Architecture: {}\n\
             Pointer Width: {} bits",
            std::env::consts::ARCH,
            std::mem::size_of::<usize>() * 8
        );

        ToolResult::success(call_id, "system.architecture", output)
    }
}

/// Tool reporting host machine network name.
pub struct SystemHostnameTool;

#[async_trait]
impl Tool for SystemHostnameTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.hostname",
            "Reports the computer network host name.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        _input: serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResult {
        let hostname = System::host_name().unwrap_or_else(|| "Unknown".to_string());
        ToolResult::success(call_id, "system.hostname", format!("Hostname: {hostname}"))
    }
}

/// Tool reporting system uptime.
pub struct SystemUptimeTool;

#[async_trait]
impl Tool for SystemUptimeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.uptime",
            "Reports the total system uptime duration since last boot.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        _input: serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResult {
        let uptime_secs = System::uptime();
        let formatted = format_uptime(uptime_secs);
        let output = format!("System Uptime: {formatted} ({uptime_secs} total seconds)");
        ToolResult::success(call_id, "system.uptime", output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_uptime_formatting() {
        assert_eq!(format_uptime(45), "45 seconds");
        assert_eq!(format_uptime(65), "1 minute, 5 seconds");
        assert_eq!(format_uptime(3665), "1 hour, 1 minute, 5 seconds");
        assert_eq!(format_uptime(90065), "1 day, 1 hour, 1 minute, 5 seconds");
        assert_eq!(format_uptime(180000), "2 days, 2 hours, 0 seconds");
    }

    #[tokio::test]
    async fn test_system_info_tools_execution() {
        let ctx = ToolContext::new("test-session", ".", ".");

        let info_tool = SystemInfoTool;
        let res = info_tool.execute("c1", json!({}), &ctx).await;
        assert_eq!(res.status, crate::definition::ToolStatus::Success);
        assert!(res.output.contains("System Diagnostic Overview"));
        assert!(res.output.contains("Operating System:"));

        let platform_tool = SystemPlatformTool;
        let res2 = platform_tool.execute("c2", json!({}), &ctx).await;
        assert_eq!(res2.status, crate::definition::ToolStatus::Success);
        assert!(res2.output.contains("Platform:"));

        let arch_tool = SystemArchitectureTool;
        let res3 = arch_tool.execute("c3", json!({}), &ctx).await;
        assert_eq!(res3.status, crate::definition::ToolStatus::Success);
        assert!(res3.output.contains("CPU Architecture:"));

        let hostname_tool = SystemHostnameTool;
        let res4 = hostname_tool.execute("c4", json!({}), &ctx).await;
        assert_eq!(res4.status, crate::definition::ToolStatus::Success);
        assert!(res4.output.contains("Hostname:"));

        let uptime_tool = SystemUptimeTool;
        let res5 = uptime_tool.execute("c5", json!({}), &ctx).await;
        assert_eq!(res5.status, crate::definition::ToolStatus::Success);
        assert!(res5.output.contains("System Uptime:"));
    }
}
