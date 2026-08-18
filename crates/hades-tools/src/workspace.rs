use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Categorical project/ecosystem type detected from workspace markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectType {
    RustCargo,
    NodeJs,
    Python,
    Go,
    Java,
    DotNet,
    Cpp,
    Generic,
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RustCargo => write!(f, "Rust (Cargo)"),
            Self::NodeJs => write!(f, "JavaScript / TypeScript (Node.js)"),
            Self::Python => write!(f, "Python"),
            Self::Go => write!(f, "Go"),
            Self::Java => write!(f, "Java (Maven/Gradle)"),
            Self::DotNet => write!(f, ".NET / C#"),
            Self::Cpp => write!(f, "C / C++ (CMake/Make)"),
            Self::Generic => write!(f, "Generic Project"),
        }
    }
}

/// Metadata and structural layout describing the active project workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    /// Canonical root path of the project workspace.
    pub root: PathBuf,
    /// Current working directory when launching or interacting with Hades.
    pub current_dir: PathBuf,
    /// Primary detected project type.
    pub project_type: ProjectType,
    /// Whether Git version control is initialized in or above the workspace.
    pub has_git: bool,
    /// Active Git branch name if Git is initialized.
    pub git_branch: Option<String>,
    /// Languages detected in the workspace based on project files.
    pub detected_languages: Vec<String>,
    /// Top-level directory files/folders (bounded to 20 entries).
    pub top_level_entries: Vec<String>,
}

impl WorkspaceMetadata {
    /// Returns the short folder name of the workspace root.
    pub fn name(&self) -> String {
        self.root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace")
            .to_string()
    }
}

/// Detector for identifying project roots, VCS status, and project characteristics.
pub struct WorkspaceDetector;

impl WorkspaceDetector {
    /// Detects workspace metadata starting from the provided starting directory.
    pub fn detect(start_dir: &Path) -> WorkspaceMetadata {
        let current_dir = start_dir
            .canonicalize()
            .unwrap_or_else(|_| start_dir.to_path_buf());

        // 1. Search upwards for project markers
        let (root, project_type) = Self::find_project_root(&current_dir);

        // 2. Detect Git repository and branch
        let (has_git, git_branch) = Self::detect_git(&root);

        // 3. Detect languages based on files in root
        let detected_languages = Self::detect_languages(&root, project_type);

        // 4. Sample top-level directory entries (bounded)
        let top_level_entries = Self::sample_top_level_entries(&root);

        WorkspaceMetadata {
            root,
            current_dir,
            project_type,
            has_git,
            git_branch,
            detected_languages,
            top_level_entries,
        }
    }

    fn find_project_root(start_dir: &Path) -> (PathBuf, ProjectType) {
        let mut curr = Some(start_dir);

        while let Some(dir) = curr {
            if dir.join("Cargo.toml").is_file() {
                return (dir.to_path_buf(), ProjectType::RustCargo);
            }
            if dir.join("package.json").is_file() {
                return (dir.to_path_buf(), ProjectType::NodeJs);
            }
            if dir.join("pyproject.toml").is_file()
                || dir.join("requirements.txt").is_file()
                || dir.join("Pipfile").is_file()
            {
                return (dir.to_path_buf(), ProjectType::Python);
            }
            if dir.join("go.mod").is_file() {
                return (dir.to_path_buf(), ProjectType::Go);
            }
            if dir.join("pom.xml").is_file()
                || dir.join("build.gradle").is_file()
                || dir.join("settings.gradle").is_file()
            {
                return (dir.to_path_buf(), ProjectType::Java);
            }
            if dir.join("CMakeLists.txt").is_file() || dir.join("Makefile").is_file() {
                return (dir.to_path_buf(), ProjectType::Cpp);
            }
            if dir.join(".git").exists() {
                return (dir.to_path_buf(), ProjectType::Generic);
            }

            curr = dir.parent();
        }

        // Fallback to the starting directory as generic workspace
        (start_dir.to_path_buf(), ProjectType::Generic)
    }

    fn detect_git(root: &Path) -> (bool, Option<String>) {
        let mut curr = Some(root);
        while let Some(dir) = curr {
            let git_dir = dir.join(".git");
            if git_dir.exists() {
                let head_file = if git_dir.is_dir() {
                    git_dir.join("HEAD")
                } else if git_dir.is_file() {
                    // Git worktree submodule file: gitdir: ...
                    if let Ok(content) = fs::read_to_string(&git_dir) {
                        if let Some(gitdir) = content.strip_prefix("gitdir: ") {
                            PathBuf::from(gitdir.trim()).join("HEAD")
                        } else {
                            git_dir.clone()
                        }
                    } else {
                        git_dir.clone()
                    }
                } else {
                    git_dir.clone()
                };

                let branch = if let Ok(head_content) = fs::read_to_string(head_file) {
                    let trimmed = head_content.trim();
                    if let Some(branch_ref) = trimmed.strip_prefix("ref: refs/heads/") {
                        Some(branch_ref.to_string())
                    } else if trimmed.len() >= 7 {
                        Some(trimmed[..7].to_string()) // Detached commit hash
                    } else {
                        None
                    }
                } else {
                    None
                };

                return (true, branch);
            }
            curr = dir.parent();
        }

        (false, None)
    }

    fn detect_languages(root: &Path, project_type: ProjectType) -> Vec<String> {
        let mut langs = Vec::new();
        match project_type {
            ProjectType::RustCargo => langs.push("Rust".to_string()),
            ProjectType::NodeJs => {
                langs.push("JavaScript".to_string());
                if root.join("tsconfig.json").is_file() {
                    langs.push("TypeScript".to_string());
                }
            }
            ProjectType::Python => langs.push("Python".to_string()),
            ProjectType::Go => langs.push("Go".to_string()),
            ProjectType::Java => langs.push("Java".to_string()),
            ProjectType::DotNet => langs.push("C#".to_string()),
            ProjectType::Cpp => {
                langs.push("C++".to_string());
                langs.push("C".to_string());
            }
            ProjectType::Generic => {}
        }

        if langs.is_empty() {
            langs.push("Unknown".to_string());
        }

        langs
    }

    fn sample_top_level_entries(root: &Path) -> Vec<String> {
        let mut entries = Vec::new();
        if let Ok(dir_entries) = fs::read_dir(root) {
            for entry in dir_entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') {
                    if entry.path().is_dir() {
                        entries.push(format!("{name}/"));
                    } else {
                        entries.push(name);
                    }
                }
                if entries.len() >= 20 {
                    break;
                }
            }
        }
        entries.sort();
        entries
    }
}
