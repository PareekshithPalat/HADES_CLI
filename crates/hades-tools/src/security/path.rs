use std::path::{Component, Path, PathBuf};

use crate::error::ToolError;

/// Safe path resolver and boundary verification engine protecting against path traversal and symlink escapes.
pub struct PathSecurity;

impl PathSecurity {
    /// Normalizes and resolves a user- or model-provided path string relative to a base directory.
    pub fn resolve_path(base_dir: &Path, input: &str) -> Result<PathBuf, ToolError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ToolError::InvalidPath("Path cannot be empty".to_string()));
        }

        // Reject null bytes and illegal control characters
        if trimmed.contains('\0') {
            return Err(ToolError::InvalidPath(
                "Path contains null byte character".to_string(),
            ));
        }

        let input_path = Path::new(trimmed);

        if input_path.is_absolute() {
            let mut normalized = PathBuf::new();
            for component in input_path.components() {
                match component {
                    Component::Prefix(prefix) => {
                        normalized = PathBuf::from(prefix.as_os_str());
                    }
                    Component::RootDir => {
                        if !normalized.is_absolute() {
                            normalized.push(Component::RootDir);
                        }
                    }
                    Component::CurDir => {}
                    Component::ParentDir => {
                        normalized.pop();
                    }
                    Component::Normal(part) => {
                        normalized.push(part);
                    }
                }
            }
            return Ok(normalized);
        }

        // Relative path resolution: ensure it cannot pop above base_dir
        let mut rel_normalized = PathBuf::new();
        for component in input_path.components() {
            match component {
                Component::Prefix(_) | Component::RootDir => {
                    return Err(ToolError::InvalidPath(format!(
                        "Invalid component in relative path: '{trimmed}'"
                    )));
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    if !rel_normalized.pop() {
                        return Err(ToolError::InvalidPath(format!(
                            "Path traversal escapes workspace boundary: '{trimmed}'"
                        )));
                    }
                }
                Component::Normal(part) => {
                    rel_normalized.push(part);
                }
            }
        }

        Ok(base_dir.join(rel_normalized))
    }

    /// Verifies whether `target` is strictly enclosed inside `boundary` without string prefix vulnerabilities.
    pub fn is_inside_boundary(target: &Path, boundary: &Path) -> bool {
        let canonical_boundary = match boundary.canonicalize() {
            Ok(p) => p,
            Err(_) => Self::lexical_normalize(boundary),
        };

        let canonical_target = if target.exists() {
            match target.canonicalize() {
                Ok(p) => p,
                Err(_) => Self::lexical_normalize(target),
            }
        } else {
            // For files not yet created, canonicalize the closest existing ancestor
            let mut curr = target.to_path_buf();
            let mut sub_parts = Vec::new();
            while !curr.exists() {
                if let Some(name) = curr.file_name() {
                    sub_parts.push(name.to_os_string());
                }
                if !curr.pop() {
                    break;
                }
            }

            let mut resolved_base = match curr.canonicalize() {
                Ok(p) => p,
                Err(_) => Self::lexical_normalize(&curr),
            };

            for part in sub_parts.into_iter().rev() {
                resolved_base.push(part);
            }
            resolved_base
        };

        // Standard component-based prefix matching
        canonical_target.starts_with(&canonical_boundary)
    }

    /// Checks whether a path points to sensitive credentials or secret configurations.
    pub fn is_sensitive_path(path: &Path) -> bool {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_lowercase();

        // 1. Sensitive filenames
        if file_name == ".env"
            || file_name.starts_with(".env.")
            || file_name == "credentials.json"
            || file_name == "secrets.json"
            || file_name == "secrets.toml"
            || file_name == "id_rsa"
            || file_name == "id_ed25519"
            || file_name == "id_ecdsa"
            || file_name == "id_dsa"
            || file_name == "shadow"
            || file_name == "master.key"
        {
            return true;
        }

        // 2. Sensitive extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if ext_lower == "pem"
                || ext_lower == "key"
                || ext_lower == "pfx"
                || ext_lower == "p12"
                || ext_lower == "kdbx"
                || ext_lower == "pkcs12"
            {
                return true;
            }
        }

        // 3. Sensitive directories in path components
        for component in path.components() {
            if let Component::Normal(comp) = component {
                let comp_str = comp.to_string_lossy().to_lowercase();
                if comp_str == ".ssh"
                    || comp_str == ".aws"
                    || comp_str == ".azure"
                    || comp_str == ".kube"
                    || comp_str == ".gnupg"
                    || comp_str == ".gemini"
                {
                    return true;
                }
            }
        }

        false
    }

    /// Checks whether a path targets system-critical operating system directories.
    pub fn is_system_path(path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        // Windows system directories
        if path_str.starts_with("c:\\windows")
            || path_str.starts_with("c:\\program files")
            || path_str.starts_with("c:\\program files (x86)")
            || path_str.starts_with("c:\\system32")
            || path_str.starts_with("c:\\recovery")
            || path_str.starts_with("c:\\boot")
        {
            return true;
        }

        // Unix system directories
        if path_str.starts_with("/etc")
            || path_str.starts_with("/usr")
            || path_str.starts_with("/bin")
            || path_str.starts_with("/sbin")
            || path_str.starts_with("/var/root")
            || path_str.starts_with("/sys")
            || path_str.starts_with("/proc")
            || path_str.starts_with("/dev")
            || path_str.starts_with("/system")
        {
            return true;
        }

        false
    }

    /// Helper performing purely lexical component normalization without I/O.
    fn lexical_normalize(path: &Path) -> PathBuf {
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normalized.pop();
                }
                c => normalized.push(c),
            }
        }
        normalized
    }
}
