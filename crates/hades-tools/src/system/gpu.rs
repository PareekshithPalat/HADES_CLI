use std::process::Command;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::system::runtime::find_in_path;

const TOOL_NAME: &str = "system.gpu";
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

/// One graphics adapter reported by a platform probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuInfo {
    pub name: String,
    pub vram_total_mb: Option<u64>,
    pub vram_free_mb: Option<u64>,
    pub utilization_percent: Option<u32>,
    /// Integrated or virtual adapter not suited for local model inference.
    pub integrated: bool,
}

impl GpuInfo {
    fn named(name: &str) -> Self {
        let name = name.trim().to_string();
        Self {
            integrated: is_integrated_or_virtual(&name),
            name,
            vram_total_mb: None,
            vram_free_mb: None,
            utilization_percent: None,
        }
    }
}

/// Heuristic for adapters that are not dedicated GPUs.
fn is_integrated_or_virtual(name: &str) -> bool {
    let lower = name.to_lowercase();
    let intel_igpu = lower.contains("intel") && !lower.contains("arc");
    let amd_apu = lower == "amd radeon(tm) graphics" || lower == "amd radeon graphics";
    let virtual_adapter = [
        "basic display",
        "basic render",
        "virtual",
        "vmware",
        "virtualbox",
        "hyper-v",
        "parsec",
        "remote display",
        "llvmpipe",
        "qxl",
        "bochs",
        "cirrus",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    intel_igpu || amd_apu || virtual_adapter
}

/// Parses `nvidia-smi --query-gpu=name,memory.total,memory.free,utilization.gpu
/// --format=csv,noheader,nounits` output.
pub fn parse_nvidia_smi(output: &str) -> Vec<GpuInfo> {
    output
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            let name = fields.first().filter(|n| !n.is_empty())?;
            let mut gpu = GpuInfo::named(name);
            gpu.integrated = false;
            gpu.vram_total_mb = fields.get(1).and_then(|v| v.parse().ok());
            gpu.vram_free_mb = fields.get(2).and_then(|v| v.parse().ok());
            gpu.utilization_percent = fields.get(3).and_then(|v| v.parse().ok());
            Some(gpu)
        })
        .collect()
}

/// Parses `Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM |
/// ConvertTo-Json` output (a single object or an array).
pub fn parse_windows_cim_json(output: &str) -> Vec<GpuInfo> {
    let value: serde_json::Value = match serde_json::from_str(output.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let entries = match value {
        serde_json::Value::Array(items) => items,
        other => vec![other],
    };
    entries
        .iter()
        .filter_map(|entry| {
            let name = entry.get("Name")?.as_str()?;
            let mut gpu = GpuInfo::named(name);
            gpu.vram_total_mb = entry
                .get("AdapterRAM")
                .and_then(|v| v.as_u64())
                .filter(|bytes| *bytes > 0)
                .map(|bytes| bytes / (1024 * 1024));
            Some(gpu)
        })
        .collect()
}

/// Parses `wmic path win32_VideoController get Name,AdapterRAM /format:csv` output.
pub fn parse_wmic_csv(output: &str) -> Vec<GpuInfo> {
    let mut lines = output.lines().map(str::trim).filter(|l| !l.is_empty());
    let header: Vec<String> = match lines.next() {
        Some(h) => h.split(',').map(|c| c.trim().to_lowercase()).collect(),
        None => return Vec::new(),
    };
    let name_idx = header.iter().position(|c| c == "name");
    let ram_idx = header.iter().position(|c| c == "adapterram");
    lines
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(',').collect();
            let name = fields.get(name_idx?)?.trim();
            if name.is_empty() {
                return None;
            }
            let mut gpu = GpuInfo::named(name);
            gpu.vram_total_mb = ram_idx
                .and_then(|i| fields.get(i))
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|bytes| *bytes > 0)
                .map(|bytes| bytes / (1024 * 1024));
            Some(gpu)
        })
        .collect()
}

/// Parses `lspci` output, keeping VGA, 3D and display controllers.
pub fn parse_lspci(output: &str) -> Vec<GpuInfo> {
    const CLASSES: [&str; 3] = [
        "vga compatible controller",
        "3d controller",
        "display controller",
    ];
    output
        .lines()
        .filter(|line| {
            let lower = line.to_lowercase();
            CLASSES.iter().any(|class| lower.contains(class))
        })
        .filter_map(|line| {
            // "01:00.0 VGA compatible controller: NVIDIA Corporation AD102 [GeForce RTX 4090] (rev a1)"
            let (_, description) = line.split_once(": ")?;
            let name = description
                .rsplit_once(" (rev ")
                .map_or(description, |(n, _)| n);
            Some(GpuInfo::named(name))
        })
        .collect()
}

/// Parses `system_profiler SPDisplaysDataType` output.
pub fn parse_system_profiler(output: &str) -> Vec<GpuInfo> {
    let mut gpus: Vec<GpuInfo> = Vec::new();
    for line in output.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix("Chipset Model:") {
            let mut gpu = GpuInfo::named(name);
            // Apple Silicon GPUs share unified memory and are fully usable for Metal inference.
            if gpu.name.starts_with("Apple") {
                gpu.integrated = false;
            }
            gpus.push(gpu);
        } else if let Some(vram) = line
            .strip_prefix("VRAM (Total):")
            .or_else(|| line.strip_prefix("VRAM (Dynamic, Max):"))
        {
            if let Some(gpu) = gpus.last_mut() {
                gpu.vram_total_mb = parse_size_mb(vram);
            }
        }
    }
    gpus
}

/// Parses sizes such as "8 GB" or "1536 MB" into megabytes.
fn parse_size_mb(text: &str) -> Option<u64> {
    let mut parts = text.split_whitespace();
    let amount: u64 = parts.next()?.parse().ok()?;
    match parts.next()?.to_ascii_uppercase().as_str() {
        "GB" => Some(amount * 1024),
        "MB" => Some(amount),
        _ => None,
    }
}

/// Formats probe results into the tool's human-readable report.
pub fn format_report(gpus: &[GpuInfo], source: &str) -> String {
    let dedicated: Vec<&GpuInfo> = gpus.iter().filter(|g| !g.integrated).collect();
    let mut out = if dedicated.is_empty() {
        let mut msg = "No dedicated GPU detected.".to_string();
        if !gpus.is_empty() {
            let names: Vec<&str> = gpus.iter().map(|g| g.name.as_str()).collect();
            msg.push_str(&format!(
                " Integrated/virtual graphics only: {}.",
                names.join(", ")
            ));
        }
        msg.push_str(" Local models will run on the CPU unless a GPU runtime is available.\n");
        msg
    } else {
        format!("Detected {} dedicated GPU(s):\n", dedicated.len())
    };

    for (i, gpu) in gpus.iter().enumerate() {
        out.push_str(&format!("\nGPU {i}: {}", gpu.name));
        if gpu.integrated {
            out.push_str(" (integrated/virtual)");
        }
        out.push('\n');
        if let Some(total) = gpu.vram_total_mb {
            out.push_str(&format!("  VRAM total: {}\n", format_mb(total)));
        }
        if let Some(free) = gpu.vram_free_mb {
            out.push_str(&format!("  VRAM free:  {}\n", format_mb(free)));
        }
        if let Some(util) = gpu.utilization_percent {
            out.push_str(&format!("  Utilization: {util}%\n"));
        }
    }
    out.push_str(&format!("\nSource: {source}\n"));
    out
}

fn format_mb(mb: u64) -> String {
    if mb >= 1024 {
        format!("{:.1} GB", mb as f64 / 1024.0)
    } else {
        format!("{mb} MB")
    }
}

/// Runs a probe command with a timeout, returning stdout on success.
async fn run_probe(program: &str, args: &[&str]) -> Option<String> {
    let path = find_in_path(program)?;
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let output = tokio::time::timeout(
        PROBE_TIMEOUT,
        tokio::task::spawn_blocking(move || Command::new(path).args(&args).output()),
    )
    .await
    .ok()?
    .ok()?
    .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Probes the host for GPUs, trying NVIDIA first and then the platform's native tooling.
async fn probe_gpus() -> (Vec<GpuInfo>, &'static str) {
    if let Some(out) = run_probe(
        "nvidia-smi",
        &[
            "--query-gpu=name,memory.total,memory.free,utilization.gpu",
            "--format=csv,noheader,nounits",
        ],
    )
    .await
    {
        let gpus = parse_nvidia_smi(&out);
        if !gpus.is_empty() {
            return (gpus, "nvidia-smi");
        }
    }

    if cfg!(target_os = "windows") {
        if let Some(out) = run_probe(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM | ConvertTo-Json -Compress",
            ],
        )
        .await
        {
            let gpus = parse_windows_cim_json(&out);
            if !gpus.is_empty() {
                return (gpus, "Win32_VideoController (CIM)");
            }
        }
        if let Some(out) = run_probe(
            "wmic",
            &[
                "path",
                "win32_VideoController",
                "get",
                "Name,AdapterRAM",
                "/format:csv",
            ],
        )
        .await
        {
            return (parse_wmic_csv(&out), "wmic");
        }
    } else if cfg!(target_os = "macos") {
        if let Some(out) = run_probe("system_profiler", &["SPDisplaysDataType"]).await {
            return (parse_system_profiler(&out), "system_profiler");
        }
    } else if let Some(out) = run_probe("lspci", &[]).await {
        return (parse_lspci(&out), "lspci");
    }

    (Vec::new(), "no GPU probe available")
}

/// Tool reporting GPU model, VRAM and utilization for local inference planning.
pub struct SystemGpuTool;

#[async_trait]
impl Tool for SystemGpuTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Reports GPU hardware (model, total/free VRAM, utilization) using nvidia-smi or the platform's native tools, and whether a dedicated GPU is available for local model inference.",
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
        let (gpus, source) = probe_gpus().await;
        let dedicated = gpus.iter().filter(|g| !g.integrated).count();
        ToolResult::success(call_id, TOOL_NAME, format_report(&gpus, source)).with_metadata(json!({
            "gpu_count": gpus.len(),
            "dedicated_gpu_count": dedicated,
            "source": source,
            "gpus": gpus.iter().map(|g| json!({
                "name": g.name,
                "vram_total_mb": g.vram_total_mb,
                "vram_free_mb": g.vram_free_mb,
                "utilization_percent": g.utilization_percent,
                "integrated": g.integrated,
            })).collect::<Vec<_>>(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::ToolStatus;

    #[test]
    fn test_parse_nvidia_smi() {
        let gpus = parse_nvidia_smi(
            "NVIDIA GeForce RTX 4090, 24564, 23010, 7\nNVIDIA A100-SXM4-80GB, 81920, 81000, 0\n",
        );
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 4090");
        assert_eq!(gpus[0].vram_total_mb, Some(24564));
        assert_eq!(gpus[0].vram_free_mb, Some(23010));
        assert_eq!(gpus[0].utilization_percent, Some(7));
        assert!(!gpus[1].integrated);
        assert!(parse_nvidia_smi("").is_empty());
    }

    #[test]
    fn test_parse_windows_cim_json_single_and_array() {
        let single =
            parse_windows_cim_json(r#"{"Name":"NVIDIA GeForce RTX 3060","AdapterRAM":4293918720}"#);
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].vram_total_mb, Some(4095));
        assert!(!single[0].integrated);

        let many = parse_windows_cim_json(
            r#"[{"Name":"Intel(R) UHD Graphics 620","AdapterRAM":1073741824},{"Name":"Microsoft Basic Display Adapter","AdapterRAM":null}]"#,
        );
        assert_eq!(many.len(), 2);
        assert!(many.iter().all(|g| g.integrated));
        assert_eq!(many[1].vram_total_mb, None);
        assert!(parse_windows_cim_json("not json").is_empty());
    }

    #[test]
    fn test_parse_wmic_csv() {
        let csv = "\r\nNode,AdapterRAM,Name\r\nDESKTOP,4293918720,NVIDIA GeForce GTX 1660\r\nDESKTOP,1073741824,Intel(R) UHD Graphics 630\r\n";
        let gpus = parse_wmic_csv(csv);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "NVIDIA GeForce GTX 1660");
        assert!(!gpus[0].integrated);
        assert!(gpus[1].integrated);
    }

    #[test]
    fn test_parse_lspci() {
        let out =
            "00:02.0 VGA compatible controller: Intel Corporation UHD Graphics 620 (rev 07)\n\
                   00:1f.3 Audio device: Intel Corporation Sunrise Point-LP HD Audio (rev 21)\n\
                   01:00.0 3D controller: NVIDIA Corporation GP108M [GeForce MX150] (rev a1)\n";
        let gpus = parse_lspci(out);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "Intel Corporation UHD Graphics 620");
        assert!(gpus[0].integrated);
        assert_eq!(gpus[1].name, "NVIDIA Corporation GP108M [GeForce MX150]");
        assert!(!gpus[1].integrated);
    }

    #[test]
    fn test_parse_system_profiler() {
        let apple = parse_system_profiler(
            "Graphics/Displays:\n\n    Apple M2 Pro:\n\n      Chipset Model: Apple M2 Pro\n      Type: GPU\n      Total Number of Cores: 19\n",
        );
        assert_eq!(apple.len(), 1);
        assert_eq!(apple[0].name, "Apple M2 Pro");
        assert!(!apple[0].integrated);

        let intel_mac = parse_system_profiler(
            "      Chipset Model: Intel Iris Plus Graphics\n      VRAM (Dynamic, Max): 1536 MB\n      Chipset Model: AMD Radeon Pro 5500M\n      VRAM (Total): 8 GB\n",
        );
        assert_eq!(intel_mac.len(), 2);
        assert!(intel_mac[0].integrated);
        assert_eq!(intel_mac[0].vram_total_mb, Some(1536));
        assert_eq!(intel_mac[1].vram_total_mb, Some(8192));
        assert!(!intel_mac[1].integrated);
    }

    #[test]
    fn test_report_dedicated_and_no_gpu_cases() {
        let report = format_report(
            &parse_nvidia_smi("NVIDIA L4, 23034, 22000, 12"),
            "nvidia-smi",
        );
        assert!(report.starts_with("Detected 1 dedicated GPU(s)"));
        assert!(report.contains("VRAM total: 22.5 GB"));
        assert!(report.contains("Utilization: 12%"));

        let integrated = format_report(&[GpuInfo::named("Intel(R) UHD Graphics")], "lspci");
        assert!(integrated.starts_with("No dedicated GPU detected."));
        assert!(integrated.contains("Intel(R) UHD Graphics"));

        let none = format_report(&[], "no GPU probe available");
        assert!(none.starts_with("No dedicated GPU detected."));
    }

    #[tokio::test]
    async fn test_tool_executes_on_this_host() {
        let ctx = ToolContext::new("s", ".", ".");
        let result = SystemGpuTool.execute("call", json!({}), &ctx).await;
        assert_eq!(result.status, ToolStatus::Success);
        assert!(
            result.output.contains("dedicated GPU"),
            "report states GPU availability: {}",
            result.output
        );
        assert!(result.metadata["gpu_count"].is_u64());
    }
}
