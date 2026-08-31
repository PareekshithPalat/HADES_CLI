use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::debug;

use crate::error::BrowserError;
use crate::types::{BrowserInfo, BrowserType};

/// Cross-platform detector for local Chromium-based browser binaries.
pub struct BrowserDetector;

impl BrowserDetector {
    /// Detects all available browser installations on the system, ordered by preference.
    pub fn detect_all() -> Vec<BrowserInfo> {
        let mut detected = Vec::new();

        // 1. Google Chrome
        if let Some(info) = Self::detect_chrome() {
            detected.push(info);
        }

        // 2. Chromium
        if let Some(info) = Self::detect_chromium() {
            detected.push(info);
        }

        // 3. Microsoft Edge
        if let Some(info) = Self::detect_edge() {
            detected.push(info);
        }

        // 4. Brave
        if let Some(info) = Self::detect_brave() {
            detected.push(info);
        }

        detected
    }

    /// Selects the best browser binary according to explicit path or preference.
    pub fn select_browser(
        explicit_path: Option<&str>,
        preference: &str,
    ) -> Result<BrowserInfo, BrowserError> {
        // 1. Explicit path given
        if let Some(p) = explicit_path {
            let path = PathBuf::from(p.trim());
            if path.exists() && path.is_file() {
                let version = Self::probe_version(&path);
                return Ok(BrowserInfo {
                    browser_type: BrowserType::Custom,
                    name: "Custom Browser".to_string(),
                    version,
                    binary_path: path,
                    is_available: true,
                });
            } else if !p.trim().is_empty() {
                return Err(BrowserError::BrowserNotFound(format!(
                    "Configured browser binary does not exist at '{p}'"
                )));
            }
        }

        let all = Self::detect_all();
        if all.is_empty() {
            return Err(BrowserError::BrowserNotFound(
                "No supported Chromium-based browser (Chrome, Chromium, Edge, Brave) was found on the system. Please install Chrome or specify binary_path in config.toml.".to_string(),
            ));
        }

        // 2. Check preference
        let pref_lower = preference.trim().to_lowercase();
        if pref_lower != "auto" && !pref_lower.is_empty() {
            let target_type = match pref_lower.as_str() {
                "chrome" | "google-chrome" => Some(BrowserType::Chrome),
                "chromium" => Some(BrowserType::Chromium),
                "edge" | "msedge" => Some(BrowserType::Edge),
                "brave" => Some(BrowserType::Brave),
                _ => None,
            };

            if let Some(tt) = target_type {
                if let Some(matched) = all.iter().find(|b| b.browser_type == tt) {
                    return Ok(matched.clone());
                }
            }
        }

        // 3. Default fallback: first detected
        Ok(all[0].clone())
    }

    /// Probes for Google Chrome.
    pub fn detect_chrome() -> Option<BrowserInfo> {
        let candidates = if cfg!(target_os = "windows") {
            vec![
                PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
                PathBuf::from(r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"),
                dirs::data_local_dir()
                    .map(|p| p.join(r"Google\Chrome\Application\chrome.exe"))
                    .unwrap_or_default(),
            ]
        } else if cfg!(target_os = "macos") {
            vec![
                PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
                dirs::home_dir()
                    .map(|p| p.join("Applications/Google Chrome.app/Contents/MacOS/Google Chrome"))
                    .unwrap_or_default(),
            ]
        } else {
            vec![
                PathBuf::from("/usr/bin/google-chrome"),
                PathBuf::from("/usr/bin/google-chrome-stable"),
                PathBuf::from("/usr/local/bin/google-chrome"),
                PathBuf::from("/opt/google/chrome/chrome"),
            ]
        };

        Self::check_candidates(
            candidates,
            "google-chrome",
            BrowserType::Chrome,
            "Google Chrome",
        )
    }

    /// Probes for Chromium.
    pub fn detect_chromium() -> Option<BrowserInfo> {
        let candidates = if cfg!(target_os = "windows") {
            vec![
                PathBuf::from(r"C:\Program Files\Chromium\Application\chrome.exe"),
                PathBuf::from(r"C:\Program Files (x86)\Chromium\Application\chrome.exe"),
                dirs::data_local_dir()
                    .map(|p| p.join(r"Chromium\Application\chrome.exe"))
                    .unwrap_or_default(),
            ]
        } else if cfg!(target_os = "macos") {
            vec![PathBuf::from(
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
            )]
        } else {
            vec![
                PathBuf::from("/usr/bin/chromium"),
                PathBuf::from("/usr/bin/chromium-browser"),
                PathBuf::from("/usr/local/bin/chromium"),
                PathBuf::from("/snap/bin/chromium"),
            ]
        };

        Self::check_candidates(candidates, "chromium", BrowserType::Chromium, "Chromium")
    }

    /// Probes for Microsoft Edge.
    pub fn detect_edge() -> Option<BrowserInfo> {
        let candidates = if cfg!(target_os = "windows") {
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
                PathBuf::from(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
                dirs::data_local_dir()
                    .map(|p| p.join(r"Microsoft\Edge\Application\msedge.exe"))
                    .unwrap_or_default(),
            ]
        } else if cfg!(target_os = "macos") {
            vec![PathBuf::from(
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            )]
        } else {
            vec![
                PathBuf::from("/usr/bin/microsoft-edge"),
                PathBuf::from("/usr/bin/microsoft-edge-stable"),
                PathBuf::from("/usr/local/bin/microsoft-edge"),
            ]
        };

        Self::check_candidates(candidates, "msedge", BrowserType::Edge, "Microsoft Edge")
    }

    /// Probes for Brave.
    pub fn detect_brave() -> Option<BrowserInfo> {
        let candidates = if cfg!(target_os = "windows") {
            vec![
                PathBuf::from(
                    r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
                ),
                PathBuf::from(
                    r"C:\Program Files (x86)\BraveSoftware\Brave-Browser\Application\brave.exe",
                ),
                dirs::data_local_dir()
                    .map(|p| p.join(r"BraveSoftware\Brave-Browser\Application\brave.exe"))
                    .unwrap_or_default(),
            ]
        } else if cfg!(target_os = "macos") {
            vec![PathBuf::from(
                "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
            )]
        } else {
            vec![
                PathBuf::from("/usr/bin/brave-browser"),
                PathBuf::from("/usr/bin/brave"),
                PathBuf::from("/snap/bin/brave"),
            ]
        };

        Self::check_candidates(candidates, "brave", BrowserType::Brave, "Brave")
    }

    fn check_candidates(
        candidates: Vec<PathBuf>,
        which_cmd: &str,
        browser_type: BrowserType,
        name: &str,
    ) -> Option<BrowserInfo> {
        // 1. Check direct standard paths
        for path in candidates {
            if path.exists() && path.is_file() {
                debug!(name, path = ?path, "Found browser at standard path");
                let version = Self::probe_version(&path);
                return Some(BrowserInfo {
                    browser_type,
                    name: name.to_string(),
                    version,
                    binary_path: path,
                    is_available: true,
                });
            }
        }

        // 2. Check PATH
        if let Some(path) = Self::which_binary(which_cmd) {
            if path.exists() && path.is_file() {
                debug!(name, path = ?path, "Found browser in PATH");
                let version = Self::probe_version(&path);
                return Some(BrowserInfo {
                    browser_type,
                    name: name.to_string(),
                    version,
                    binary_path: path,
                    is_available: true,
                });
            }
        }

        None
    }

    fn which_binary(name: &str) -> Option<PathBuf> {
        let cmd_name = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };
        let output = Command::new(cmd_name).arg(name).output().ok()?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(first_line) = stdout.lines().next() {
                let trimmed = first_line.trim();
                if !trimmed.is_empty() {
                    let p = PathBuf::from(trimmed);
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    fn probe_version(path: &Path) -> String {
        if cfg!(target_os = "windows") {
            return "Detected (Windows)".to_string();
        }

        let output = Command::new(path).arg("--version").output();
        if let Ok(out) = output {
            if out.status.success() {
                let txt = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !txt.is_empty() {
                    return txt;
                }
            }
        }
        "Detected".to_string()
    }
}
