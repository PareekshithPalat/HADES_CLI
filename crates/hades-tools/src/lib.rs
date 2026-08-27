pub mod context;
pub mod definition;
pub mod environment;
pub mod error;
pub mod filesystem;
pub mod process;
pub mod registry;
pub mod security;
pub mod shell;
pub mod system;
pub mod workspace;
pub mod workspace_tools;

pub use context::ToolContext;
pub use definition::{DynTool, RiskLevel, Tool, ToolCall, ToolDefinition, ToolResult, ToolStatus};
pub use error::ToolError;
pub use registry::ToolRegistry;
pub use security::{
    ApprovalDecision, EvaluationResult, PathSecurity, PermissionEngine, SecretRedactor,
};
pub use system::{
    format_uptime, SystemArchitectureTool, SystemHostnameTool, SystemInfoTool,
    SystemNetworkConnectionsTool, SystemNetworkInterfacesTool, SystemNetworkPortCheckTool,
    SystemNetworkPortProcessTool, SystemPlatformTool, SystemProcessFindTool,
    SystemProcessInspectTool, SystemProcessListTool, SystemProcessTerminateTool,
    SystemRuntimeVersionTool, SystemRuntimeWhichTool, SystemUptimeTool,
};
pub use workspace::{ProjectType, WorkspaceDetector, WorkspaceMetadata};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_default_registry_contains_all_standard_tools() {
        let registry = ToolRegistry::default_registry();
        assert_eq!(registry.count(), 29);
        assert!(registry.contains("filesystem.list"));
        assert!(registry.contains("filesystem.read"));
        assert!(registry.contains("filesystem.create"));
        assert!(registry.contains("filesystem.write"));
        assert!(registry.contains("filesystem.edit"));
        assert!(registry.contains("filesystem.delete"));
        assert!(registry.contains("filesystem.mkdir"));
        assert!(registry.contains("shell.execute"));
        assert!(registry.contains("environment.get"));
        assert!(registry.contains("environment.list"));
        assert!(registry.contains("environment.set"));
        assert!(registry.contains("environment.unset"));
        assert!(registry.contains("workspace.inspect"));
        assert!(registry.contains("workspace.detect"));
        assert!(registry.contains("system.info"));
        assert!(registry.contains("system.platform"));
        assert!(registry.contains("system.architecture"));
        assert!(registry.contains("system.hostname"));
        assert!(registry.contains("system.uptime"));
        assert!(registry.contains("system.process.list"));
        assert!(registry.contains("system.process.inspect"));
        assert!(registry.contains("system.process.find"));
        assert!(registry.contains("system.process.terminate"));
        assert!(registry.contains("system.network.interfaces"));
        assert!(registry.contains("system.network.port_check"));
        assert!(registry.contains("system.network.port_process"));
        assert!(registry.contains("system.network.connections"));
        assert!(registry.contains("system.runtime.which"));
        assert!(registry.contains("system.runtime.version"));
    }

    #[test]
    fn test_path_normalization_and_traversal_protection() {
        let dir = tempdir().expect("create temp dir");
        let base = dir.path();

        // 1. Normal relative path
        let res = PathSecurity::resolve_path(base, "src/main.rs").unwrap();
        assert_eq!(res, base.join("src/main.rs"));

        // 2. Traversal within boundary
        let res2 = PathSecurity::resolve_path(base, "src/../src/lib.rs").unwrap();
        assert_eq!(res2, base.join("src/lib.rs"));

        // 3. Traversal escaping root
        let res3 = PathSecurity::resolve_path(base, "../../secret.txt");
        assert!(res3.is_err());

        // 4. Null byte injection
        let res4 = PathSecurity::resolve_path(base, "src/file\0evil.rs");
        assert!(res4.is_err());
    }

    #[test]
    fn test_boundary_checks_prevent_escapes() {
        let dir = tempdir().expect("create temp dir");
        let workspace = dir.path().join("workspace");
        let sibling = dir.path().join("workspace-evil");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&sibling).unwrap();

        let inside_file = workspace.join("src/main.rs");
        let outside_file = sibling.join("hack.rs");

        assert!(PathSecurity::is_inside_boundary(&inside_file, &workspace));
        assert!(!PathSecurity::is_inside_boundary(&outside_file, &workspace));
    }

    #[test]
    fn test_sensitive_path_detection() {
        assert!(PathSecurity::is_sensitive_path(std::path::Path::new(
            ".env"
        )));
        assert!(PathSecurity::is_sensitive_path(std::path::Path::new(
            ".env.local"
        )));
        assert!(PathSecurity::is_sensitive_path(std::path::Path::new(
            "id_rsa"
        )));
        assert!(PathSecurity::is_sensitive_path(std::path::Path::new(
            "server.key"
        )));
        assert!(PathSecurity::is_sensitive_path(std::path::Path::new(
            "cert.pem"
        )));
        assert!(PathSecurity::is_sensitive_path(std::path::Path::new(
            ".ssh/authorized_keys"
        )));
        assert!(!PathSecurity::is_sensitive_path(std::path::Path::new(
            "src/main.rs"
        )));
        assert!(!PathSecurity::is_sensitive_path(std::path::Path::new(
            "Cargo.toml"
        )));
    }

    #[test]
    fn test_secret_redaction() {
        // Redacts secret keys
        let sample = "Using key sk-1234567890abcdef1234567890 for auth";
        let redacted = SecretRedactor::redact_text(sample);
        assert!(redacted.contains("sk-[REDACTED]"));
        assert!(!redacted.contains("1234567890abcdef1234567890"));

        // Redacts env vars
        let env_redacted = SecretRedactor::redact_env_var("OPENAI_API_KEY", "sk-proj-999999999999");
        assert!(env_redacted.contains("[REDACTED]"));

        let normal_env = SecretRedactor::redact_env_var("RUST_LOG", "debug");
        assert_eq!(normal_env, "debug");
    }

    #[test]
    fn test_permission_engine_risk_classification() {
        let engine = PermissionEngine::new();
        let dir = tempdir().unwrap();
        let context = ToolContext::new("s1", dir.path(), dir.path());

        // Safe tool inside workspace
        let call_safe = ToolCall::new(
            "1",
            "filesystem.read",
            serde_json::json!({ "path": "Cargo.toml" }),
        );
        let def_safe = ToolDefinition::new(
            "filesystem.read",
            "desc",
            serde_json::json!({}),
            RiskLevel::Safe,
            false,
        );
        let eval_safe = engine.evaluate(&call_safe, &def_safe, &context);
        assert!(matches!(
            eval_safe,
            EvaluationResult::Permitted {
                risk: RiskLevel::Safe
            }
        ));

        // Mutating tool inside workspace requires approval by default
        let call_edit = ToolCall::new(
            "2",
            "filesystem.edit",
            serde_json::json!({ "path": "src/lib.rs", "old_content": "a", "new_content": "b" }),
        );
        let def_edit = ToolDefinition::new(
            "filesystem.edit",
            "desc",
            serde_json::json!({}),
            RiskLevel::Medium,
            true,
        );
        let eval_edit = engine.evaluate(&call_edit, &def_edit, &context);
        assert!(matches!(
            eval_edit,
            EvaluationResult::RequiresApproval {
                risk: RiskLevel::Medium,
                ..
            }
        ));

        // Dangerous command
        let call_rm = ToolCall::new(
            "3",
            "shell.execute",
            serde_json::json!({ "executable": "rm", "arguments": ["-rf", "/"] }),
        );
        let def_rm = ToolDefinition::new(
            "shell.execute",
            "desc",
            serde_json::json!({}),
            RiskLevel::High,
            true,
        );
        let eval_rm = engine.evaluate(&call_rm, &def_rm, &context);
        assert!(matches!(
            eval_rm,
            EvaluationResult::RequiresApproval {
                risk: RiskLevel::Critical,
                ..
            }
        ));
    }

    #[test]
    fn test_permission_engine_session_allow() {
        let mut engine = PermissionEngine::new();
        let dir = tempdir().unwrap();
        let context = ToolContext::new("s1", dir.path(), dir.path());

        let call = ToolCall::new(
            "1",
            "filesystem.write",
            serde_json::json!({ "path": "test.txt", "content": "hello" }),
        );
        let def = ToolDefinition::new(
            "filesystem.write",
            "desc",
            serde_json::json!({}),
            RiskLevel::Medium,
            true,
        );

        // Before grant -> requires approval
        let eval1 = engine.evaluate(&call, &def, &context);
        assert!(matches!(eval1, EvaluationResult::RequiresApproval { .. }));

        // Grant session permission
        engine.grant_session_permission("filesystem.write");

        // After grant -> permitted
        let eval2 = engine.evaluate(&call, &def, &context);
        assert!(matches!(eval2, EvaluationResult::Permitted { .. }));
    }

    #[tokio::test]
    async fn test_filesystem_create_read_edit_write_delete_lifecycle() {
        let dir = tempdir().unwrap();
        let context = ToolContext::new("s1", dir.path(), dir.path());
        let registry = ToolRegistry::default_registry();

        // 1. Create file
        let create_tool = registry.get("filesystem.create").unwrap();
        let res_create = create_tool
            .execute(
                "c1",
                serde_json::json!({
                    "path": "hello.txt",
                    "content": "Hello World!\nInitial line.\n"
                }),
                &context,
            )
            .await;
        assert_eq!(res_create.status, ToolStatus::Success);
        assert!(dir.path().join("hello.txt").exists());

        // 2. Read file
        let read_tool = registry.get("filesystem.read").unwrap();
        let res_read = read_tool
            .execute("c2", serde_json::json!({ "path": "hello.txt" }), &context)
            .await;
        assert_eq!(res_read.status, ToolStatus::Success);
        assert!(res_read.output.contains("Hello World!"));

        // 3. Edit file
        let edit_tool = registry.get("filesystem.edit").unwrap();
        let res_edit = edit_tool
            .execute(
                "c3",
                serde_json::json!({
                    "path": "hello.txt",
                    "old_content": "Initial line.",
                    "new_content": "Replaced line."
                }),
                &context,
            )
            .await;
        assert_eq!(res_edit.status, ToolStatus::Success);

        let content_after_edit = fs::read_to_string(dir.path().join("hello.txt")).unwrap();
        assert!(content_after_edit.contains("Replaced line."));
        assert!(!content_after_edit.contains("Initial line."));

        // 4. Overwrite via write tool
        let write_tool = registry.get("filesystem.write").unwrap();
        let res_write = write_tool
            .execute(
                "c4",
                serde_json::json!({
                    "path": "hello.txt",
                    "content": "Completely new content."
                }),
                &context,
            )
            .await;
        assert_eq!(res_write.status, ToolStatus::Success);

        // 5. Delete file
        let delete_tool = registry.get("filesystem.delete").unwrap();
        let res_delete = delete_tool
            .execute("c5", serde_json::json!({ "path": "hello.txt" }), &context)
            .await;
        assert_eq!(res_delete.status, ToolStatus::Success);
        assert!(!dir.path().join("hello.txt").exists());
    }

    #[tokio::test]
    async fn test_filesystem_edit_conflict_handling() {
        let dir = tempdir().unwrap();
        let context = ToolContext::new("s1", dir.path(), dir.path());
        let edit_tool = registry::ToolRegistry::default_registry()
            .get("filesystem.edit")
            .unwrap();

        fs::write(dir.path().join("code.rs"), "fn foo() { 123 }\n").unwrap();

        // Non-existent snippet -> conflict failure
        let res_missing = edit_tool
            .execute(
                "c1",
                serde_json::json!({
                    "path": "code.rs",
                    "old_content": "fn bar()",
                    "new_content": "fn baz()"
                }),
                &context,
            )
            .await;
        assert_eq!(res_missing.status, ToolStatus::Failure);
        assert!(res_missing
            .error
            .unwrap()
            .contains("Expected old_content snippet was not found"));
    }

    #[tokio::test]
    async fn test_workspace_detector_markers() {
        let dir = tempdir().unwrap();
        let project_dir = dir.path().join("my-rust-project");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(
            project_dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();

        let sub_dir = project_dir.join("src/nested");
        fs::create_dir_all(&sub_dir).unwrap();

        let meta = WorkspaceDetector::detect(&sub_dir);
        assert_eq!(meta.project_type, ProjectType::RustCargo);
        assert_eq!(meta.root, project_dir.canonicalize().unwrap());
    }
}
