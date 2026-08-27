use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Command;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;
use sysinfo::{Networks, Pid, ProcessesToUpdate, System};

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};

/// Tool listing host network interfaces and addresses.
pub struct SystemNetworkInterfacesTool;

#[async_trait]
impl Tool for SystemNetworkInterfacesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.network.interfaces",
            "Lists all network interfaces on the host machine including interface names, MAC addresses, and IP addresses.",
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
        let networks = Networks::new_with_refreshed_list();

        if networks.is_empty() {
            return ToolResult::success(
                call_id,
                "system.network.interfaces",
                "No active network interfaces detected.",
            );
        }

        let mut output = format!("Host Network Interfaces ({} total):\n", networks.len());
        output.push_str(&format!(
            "{:<20} {:<18} {:<30} {}\n",
            "INTERFACE", "MAC ADDRESS", "IP ADDRESSES", "TRAFFIC (RX / TX)"
        ));
        output.push_str(&format!("{}\n", "─".repeat(80)));

        for (interface_name, data) in &networks {
            let mac = data.mac_address().to_string();
            let ips: Vec<String> = data
                .ip_networks()
                .iter()
                .map(|net| net.addr.to_string())
                .collect();
            let ips_str = if ips.is_empty() {
                "None".to_string()
            } else {
                ips.join(", ")
            };
            let rx_mb = (data.total_received() as f64) / (1024.0 * 1024.0);
            let tx_mb = (data.total_transmitted() as f64) / (1024.0 * 1024.0);

            output.push_str(&format!(
                "{:<20} {:<18} {:<30} {:.2} MB / {:.2} MB\n",
                interface_name, mac, ips_str, rx_mb, tx_mb
            ));
        }

        ToolResult::success(call_id, "system.network.interfaces", output)
    }
}

/// Tool checking if a specific TCP port is currently in use or open.
pub struct SystemNetworkPortCheckTool;

#[async_trait]
impl Tool for SystemNetworkPortCheckTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.network.port_check",
            "Checks if a specific TCP port (e.g. 3000, 8080, 5432) is currently in use / listening on the local host.",
            json!({
                "type": "object",
                "properties": {
                    "port": {
                        "type": "integer",
                        "description": "TCP port number to check (1-65535)"
                    }
                },
                "required": ["port"],
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
        let port = match input.get("port").and_then(|v| v.as_u64()) {
            Some(p) if (1..=65535).contains(&p) => p as u16,
            _ => {
                return ToolResult::invalid_input(
                    call_id,
                    "system.network.port_check",
                    "Parameter 'port' must be a valid integer between 1 and 65535.",
                );
            }
        };

        // Try connecting to 127.0.0.1:port with a short timeout
        let addr_v4: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
        let is_listening = match TcpStream::connect_timeout(&addr_v4, Duration::from_millis(300)) {
            Ok(_) => true,
            Err(_) => {
                // If connect failed, check if binding fails (means port is occupied or restricted)
                match TcpListener::bind(format!("127.0.0.1:{port}")) {
                    Ok(listener) => {
                        drop(listener);
                        false
                    }
                    Err(_) => true,
                }
            }
        };

        let status_desc = if is_listening {
            format!("Port {port} is currently IN USE (active listening socket detected on 127.0.0.1:{port}).")
        } else {
            format!("Port {port} is AVAILABLE (no active listener detected on 127.0.0.1:{port}).")
        };

        ToolResult::success(call_id, "system.network.port_check", status_desc)
    }
}

/// Tool identifying which process is listening on or bound to a specific port.
pub struct SystemNetworkPortProcessTool;

#[async_trait]
impl Tool for SystemNetworkPortProcessTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.network.port_process",
            "Discovers which process and PID is actively using or listening on a specific network port.",
            json!({
                "type": "object",
                "properties": {
                    "port": {
                        "type": "integer",
                        "description": "Port number to inspect (1-65535)"
                    }
                },
                "required": ["port"],
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
        let port = match input.get("port").and_then(|v| v.as_u64()) {
            Some(p) if (1..=65535).contains(&p) => p as u16,
            _ => {
                return ToolResult::invalid_input(
                    call_id,
                    "system.network.port_process",
                    "Parameter 'port' must be a valid integer between 1 and 65535.",
                );
            }
        };

        let mut sys = System::new_all();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        // Find PID occupying the port using platform commands
        let target_pid = find_pid_for_port(port);

        match target_pid {
            Some(pid_num) => {
                let proc_info = if let Some(p) = sys.process(Pid::from_u32(pid_num)) {
                    let name = p.name().to_string_lossy();
                    let mem_mb = (p.memory() as f64) / (1024.0 * 1024.0);
                    let cmdline = p
                        .cmd()
                        .iter()
                        .map(|s| s.to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!(
                        "Process using port {port}:\n\
                         - PID: {pid_num}\n\
                         - Name: {name}\n\
                         - Memory: {mem_mb:.2} MB\n\
                         - Command: {cmdline}"
                    )
                } else {
                    format!("Port {port} is occupied by Process ID (PID): {pid_num}.")
                };

                ToolResult::success(call_id, "system.network.port_process", proc_info)
            }
            None => {
                // If direct PID lookup didn't find anything, check if port is open or available
                match TcpListener::bind(format!("127.0.0.1:{port}")) {
                    Ok(l) => {
                        drop(l);
                        ToolResult::success(
                            call_id,
                            "system.network.port_process",
                            format!("Port {port} is currently free/available. No process is using it."),
                        )
                    }
                    Err(_) => ToolResult::success(
                        call_id,
                        "system.network.port_process",
                        format!("Port {port} is currently bound/in use, but process ownership could not be determined without elevated privileges."),
                    ),
                }
            }
        }
    }
}

/// Tool listing active network connections and listening ports.
pub struct SystemNetworkConnectionsTool;

#[async_trait]
impl Tool for SystemNetworkConnectionsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.network.connections",
            "Lists active listening TCP sockets and established connections on the host machine.",
            json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of connections to display (default: 30)"
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
        let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(30) as usize;

        let connections = list_active_connections(limit);
        ToolResult::success(call_id, "system.network.connections", connections)
    }
}

/// Cross-platform helper to discover PID occupying a given port.
fn find_pid_for_port(port: u16) -> Option<u32> {
    #[cfg(target_os = "windows")]
    {
        // On Windows, run `netstat -ano -p tcp`
        if let Ok(output) = Command::new("netstat").args(["-ano", "-p", "tcp"]).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            let port_str = format!(":{port}");
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 && parts[0].eq_ignore_ascii_case("TCP") {
                    let local_addr = parts[1];
                    let state = parts[3];
                    if local_addr.ends_with(&port_str)
                        && (state.eq_ignore_ascii_case("LISTENING")
                            || state.eq_ignore_ascii_case("ESTABLISHED"))
                    {
                        if let Ok(pid) = parts[4].parse::<u32>() {
                            if pid > 0 {
                                return Some(pid);
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Linux / macOS, try `lsof -i :<port> -t`
        if let Ok(output) = Command::new("lsof")
            .args(["-i", &format!(":{port}"), "-t"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(first_line) = text.lines().next() {
                if let Ok(pid) = first_line.trim().parse::<u32>() {
                    return Some(pid);
                }
            }
        }

        // Fallback: `ss -lptn`
        if let Ok(output) = Command::new("ss")
            .args(["-lptn", &format!("sport = :{port}")])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(pid_idx) = text.find("pid=") {
                let remainder = &text[pid_idx + 4..];
                let num_str: String = remainder
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(pid) = num_str.parse::<u32>() {
                    return Some(pid);
                }
            }
        }
    }

    None
}

/// Cross-platform helper listing active listening and established connections.
fn list_active_connections(limit: usize) -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = Command::new("netstat").args(["-ano", "-p", "tcp"]).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut lines_out = Vec::new();
            lines_out.push(format!(
                "{:<6} {:<24} {:<24} {:<14} {}",
                "PROTO", "LOCAL ADDRESS", "FOREIGN ADDRESS", "STATE", "PID"
            ));
            lines_out.push("─".repeat(80));

            let mut count = 0;
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 && parts[0].eq_ignore_ascii_case("TCP") {
                    lines_out.push(format!(
                        "{:<6} {:<24} {:<24} {:<14} {}",
                        parts[0], parts[1], parts[2], parts[3], parts[4]
                    ));
                    count += 1;
                    if count >= limit {
                        break;
                    }
                }
            }

            if count > 0 {
                return lines_out.join("\n");
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = Command::new("ss").args(["-tuln"]).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = text.lines().take(limit + 1).collect();
            if !lines.is_empty() {
                return lines.join("\n");
            }
        }
    }

    "Active connections could not be queried or permission was restricted.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_network_interfaces() {
        let ctx = ToolContext::new("test-session", ".", ".");
        let tool = SystemNetworkInterfacesTool;

        let res = tool.execute("n1", json!({}), &ctx).await;
        assert_eq!(res.status, crate::definition::ToolStatus::Success);
        assert!(
            res.output.contains("Host Network Interfaces")
                || res.output.contains("No active network")
        );
    }

    #[tokio::test]
    async fn test_system_network_port_check() {
        let ctx = ToolContext::new("test-session", ".", ".");
        let tool = SystemNetworkPortCheckTool;

        // Check random ephemeral port
        let res = tool.execute("n2", json!({ "port": 59123 }), &ctx).await;
        assert_eq!(res.status, crate::definition::ToolStatus::Success);
        assert!(res.output.contains("Port 59123 is"));

        // Check invalid port
        let res_err = tool.execute("n3", json!({ "port": 999999 }), &ctx).await;
        assert_eq!(res_err.status, crate::definition::ToolStatus::InvalidInput);
    }

    #[tokio::test]
    async fn test_system_network_connections() {
        let ctx = ToolContext::new("test-session", ".", ".");
        let tool = SystemNetworkConnectionsTool;

        let res = tool.execute("n4", json!({ "limit": 10 }), &ctx).await;
        assert_eq!(res.status, crate::definition::ToolStatus::Success);
    }
}
