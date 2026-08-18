use hades_config::{ActiveModelConfig, ConfigService};
use hades_core::{AppState, CommandOutput, HadesApp, APP_VERSION};
use hades_events::{EventBus, HadesEvent};
use hades_provider::{Credential, CredentialBackend, FileCredentialBackend, Usage};
use hades_storage::{FileSessionRepository, Message, SessionRepository, StorageService};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_full_application_lifecycle_and_events() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let event_bus = EventBus::new();
    let mut event_rx = event_bus.subscribe();

    let mut app = HadesApp::new(config_service, storage_service, event_bus);

    // Initial state
    assert_eq!(app.state(), AppState::Startup);

    // Initialize application on fresh start -> transitions to ProviderSelect
    app.init().expect("initialize app");
    assert_eq!(app.state(), AppState::ProviderSelect);
    assert!(config_path.exists());
    assert!(data_dir.exists());

    // Transition to Running for command tests
    app.transition_to(AppState::Running)
        .expect("transition to running");

    // Verify events were emitted
    let mut received_events = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        received_events.push(event);
    }

    assert!(received_events
        .iter()
        .any(|e| matches!(e, HadesEvent::ConfigLoaded { .. })));
    assert!(received_events
        .iter()
        .any(|e| matches!(e, HadesEvent::ApplicationStarted { .. })));

    // Test /status command
    let status_output = app.execute_command("/status").expect("execute status");
    match status_output {
        CommandOutput::Status(status) => {
            assert_eq!(status.version, APP_VERSION);
            assert_eq!(status.application, "Running");
            assert_eq!(status.model, "Not configured");
        }
        _ => panic!("Expected Status output"),
    }

    // Test /help command
    let help_output = app.execute_command("/help").expect("execute help");
    match help_output {
        CommandOutput::Help(entries) => {
            assert!(entries.iter().any(|e| e.name == "/help"));
            assert!(entries.iter().any(|e| e.name == "/status"));
            assert!(entries.iter().any(|e| e.name == "/model"));
            assert!(entries.iter().any(|e| e.name == "/switch"));
            assert!(entries.iter().any(|e| e.name == "/new"));
            assert!(entries.iter().any(|e| e.name == "/sessions"));
            assert!(entries.iter().any(|e| e.name == "/tools"));
            assert!(entries.iter().any(|e| e.name == "/workspace"));
            assert!(entries.iter().any(|e| e.name == "/permissions"));
            assert!(entries.iter().any(|e| e.name == "/exit"));
        }
        _ => panic!("Expected Help output"),
    }

    // Test /model command
    let model_output = app.execute_command("/model").expect("execute model");
    assert_eq!(model_output, CommandOutput::OpenModelSetup);
    assert_eq!(app.state(), AppState::ProviderSelect);

    // Test /exit command
    let exit_output = app.execute_command("/exit").expect("execute exit");
    assert_eq!(exit_output, CommandOutput::Exit);
    assert_eq!(app.state(), AppState::Exited);
}

#[tokio::test]
async fn test_configured_model_startup_restoration() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let creds_path = dir.path().join("credentials.json");

    let config_service = ConfigService::with_path(&config_path);
    let config = hades_config::HadesConfig {
        model: Some(ActiveModelConfig::new("openai", "gpt-4o")),
        ..Default::default()
    };
    config_service.save(&config).expect("save config");

    let credential_backend = Arc::new(FileCredentialBackend::with_path(&creds_path));
    credential_backend
        .store_credential(&Credential::with_api_key("openai", "test-key-123"))
        .await
        .expect("store credential");

    let storage_service = StorageService::with_root(&data_dir);
    let event_bus = EventBus::new();

    let mut app = HadesApp::with_credential_backend(
        config_service,
        storage_service,
        event_bus,
        credential_backend,
    );

    app.init().expect("initialize app");
    assert_eq!(app.state(), AppState::Running);
    assert_eq!(app.active_model_display(), "openai/gpt-4o");

    let status_output = app.execute_command("/status").expect("execute status");
    match status_output {
        CommandOutput::Status(status) => {
            assert_eq!(status.model, "openai/gpt-4o");
        }
        _ => panic!("Expected Status output"),
    }
}

#[tokio::test]
async fn test_session_persistence_and_multi_session_isolation() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let sessions_dir = dir.path().join("sessions");
    let creds_path = dir.path().join("credentials.json");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let session_repo = Arc::new(FileSessionRepository::with_dir(&sessions_dir));
    let credential_backend = Arc::new(FileCredentialBackend::with_path(&creds_path));
    let event_bus = EventBus::new();

    let mut app = HadesApp::with_backends(
        config_service,
        storage_service,
        event_bus,
        credential_backend,
        session_repo.clone(),
    );
    app.init().expect("init");

    // 1. Create Session Alpha
    let alpha = app
        .create_new_session(Some("Alpha Session".to_string()))
        .await
        .expect("create alpha");
    assert_eq!(alpha.metadata.title, "Alpha Session");

    if let Some(s) = app.active_session_mut() {
        s.add_message(Message::user(&alpha.metadata.id, "Prompt in Alpha"));
        s.add_message(Message::assistant(
            &alpha.metadata.id,
            "Response in Alpha",
            Some("openai".to_string()),
            Some("gpt-4o".to_string()),
        ));
    }
    if let Some(s) = app.active_session() {
        session_repo.save_session(s).await.expect("save alpha");
    }

    // 2. Create Session Beta
    let beta = app
        .create_new_session(Some("Beta Session".to_string()))
        .await
        .expect("create beta");
    assert_eq!(beta.metadata.title, "Beta Session");

    if let Some(s) = app.active_session_mut() {
        s.add_message(Message::user(&beta.metadata.id, "Prompt in Beta"));
    }
    if let Some(s) = app.active_session() {
        session_repo.save_session(s).await.expect("save beta");
    }

    // 3. Switch back to Alpha
    let switched_alpha = app
        .switch_session(&alpha.metadata.id)
        .await
        .expect("switch to alpha");
    assert_eq!(switched_alpha.messages.len(), 2);
    assert_eq!(switched_alpha.messages[0].content, "Prompt in Alpha");
    assert_eq!(switched_alpha.messages[1].content, "Response in Alpha");

    // 4. Switch to Beta
    let switched_beta = app
        .switch_session(&beta.metadata.id)
        .await
        .expect("switch to beta");
    assert_eq!(switched_beta.messages.len(), 1);
    assert_eq!(switched_beta.messages[0].content, "Prompt in Beta");

    // 5. Restart application simulation (explicitly resuming active session by ID)
    let config_service2 = ConfigService::with_path(&config_path);
    let storage_service2 = StorageService::with_root(&data_dir);
    let event_bus2 = EventBus::new();
    let mut app2 = HadesApp::with_backends(
        config_service2,
        storage_service2,
        event_bus2,
        Arc::new(FileCredentialBackend::with_path(&creds_path)),
        session_repo,
    );
    app2.init().expect("init app2");
    app2.init_session(Some(&beta.metadata.id))
        .await
        .expect("init session app2");

    assert_eq!(app2.active_session().unwrap().metadata.id, beta.metadata.id);
    assert_eq!(app2.active_session().unwrap().messages.len(), 1);
}

#[tokio::test]
async fn test_streaming_persistence_and_crash_recovery() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let sessions_dir = dir.path().join("sessions");
    let creds_path = dir.path().join("credentials.json");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let session_repo = Arc::new(FileSessionRepository::with_dir(&sessions_dir));
    let credential_backend = Arc::new(FileCredentialBackend::with_path(&creds_path));
    let event_bus = EventBus::new();

    let mut app = HadesApp::with_backends(
        config_service,
        storage_service,
        event_bus,
        credential_backend,
        session_repo.clone(),
    );
    app.init().expect("init");

    let session = app
        .create_new_session(Some("Streaming Test".to_string()))
        .await
        .expect("create session");

    // Create active assistant message in streaming state
    let mut assistant_msg = Message::assistant(
        &session.metadata.id,
        "",
        Some("groq".to_string()),
        Some("llama-3.3-70b-versatile".to_string()),
    );
    assistant_msg.metadata.streaming_complete = false;
    let msg_id = assistant_msg.id.clone();

    if let Some(s) = app.active_session_mut() {
        s.add_message(assistant_msg);
    }
    if let Some(s) = app.active_session() {
        session_repo.save_session(s).await.expect("save session");
    }

    // Finalize stream successfully
    let usage = Usage {
        input_tokens: Some(10),
        output_tokens: Some(30),
        total_tokens: Some(40),
    };
    app.finalize_streaming_response(
        &msg_id,
        "Here is the complete streamed response.",
        Some(usage),
        false,
    )
    .await
    .expect("finalize stream");

    // Verify stored record
    let reloaded = session_repo
        .get_session(&session.metadata.id)
        .await
        .expect("get session")
        .expect("must exist");

    assert_eq!(reloaded.messages.len(), 1);
    let final_msg = &reloaded.messages[0];
    assert_eq!(final_msg.content, "Here is the complete streamed response.");
    assert!(final_msg.metadata.streaming_complete);
    assert!(!final_msg.metadata.is_interrupted);
    assert_eq!(final_msg.metadata.total_tokens, Some(40));
}

#[tokio::test]
async fn test_context_truncation_preserves_complete_stored_history() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let sessions_dir = dir.path().join("sessions");
    let creds_path = dir.path().join("credentials.json");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let session_repo = Arc::new(FileSessionRepository::with_dir(&sessions_dir));
    let credential_backend = Arc::new(FileCredentialBackend::with_path(&creds_path));
    let event_bus = EventBus::new();

    let mut app = HadesApp::with_backends(
        config_service,
        storage_service,
        event_bus,
        credential_backend,
        session_repo.clone(),
    );
    app.init().expect("init");

    // Register tiny context limit for test model
    app.context_manager_mut()
        .register_model_limit("tiny-model", 80);

    let session = app
        .create_new_session(Some("Truncation Test".to_string()))
        .await
        .expect("create session");

    // Add 10 historical turns to session
    for i in 0..10 {
        if let Some(s) = app.active_session_mut() {
            s.add_message(Message::user(
                &session.metadata.id,
                format!(
                    "User historical message #{} with lengthy text explanation",
                    i
                ),
            ));
            s.add_message(Message::assistant(
                &session.metadata.id,
                format!("Assistant response #{} detailing technical concepts", i),
                Some("openai".to_string()),
                Some("tiny-model".to_string()),
            ));
        }
    }
    if let Some(s) = app.active_session() {
        session_repo.save_session(s).await.expect("save session");
    }

    // Build context with tiny-model
    let history = app.active_session().unwrap().messages.as_slice();
    let (chat_messages, report) = app
        .context_manager()
        .build_context(history, "tiny-model", None, "Current question")
        .expect("build context");

    // Verify context report shows truncation
    assert!(report.was_truncated);
    assert!(report.included_messages < history.len() + 1);
    assert_eq!(
        chat_messages.last().unwrap().content.as_deref(),
        Some("Current question")
    );

    // CRITICAL REQUIREMENT: STORED HISTORY REMAINS COMPLETE (20 messages)
    let reloaded_session = session_repo
        .get_session(&session.metadata.id)
        .await
        .expect("load session")
        .expect("session must exist");

    assert_eq!(reloaded_session.messages.len(), 20);
    assert_eq!(reloaded_session.metadata.message_count, 20);
}

#[tokio::test]
async fn test_phase21_normal_startup_creates_new_session() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let sessions_dir = dir.path().join("sessions");
    let creds_path = dir.path().join("credentials.json");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let session_repo = Arc::new(FileSessionRepository::with_dir(&sessions_dir));
    let credential_backend = Arc::new(FileCredentialBackend::with_path(&creds_path));
    let event_bus = EventBus::new();

    let mut app = HadesApp::with_backends(
        config_service,
        storage_service,
        event_bus,
        credential_backend,
        session_repo.clone(),
    );
    app.init().expect("init");

    // Launch 1 (normal launch -> creates session A)
    app.init_session(None).await.expect("init 1");
    let id_1 = app.active_session().unwrap().metadata.id.clone();

    // Add a message to session A
    if let Some(s) = app.active_session_mut() {
        s.add_message(Message::user(&id_1, "Message in session 1"));
    }
    app.save_active_session().await.expect("save 1");

    // Launch 2 (normal launch -> creates session B, does NOT auto-restore session A)
    app.init_session(None).await.expect("init 2");
    let id_2 = app.active_session().unwrap().metadata.id.clone();

    assert_ne!(id_1, id_2);
    assert_eq!(app.active_session().unwrap().messages.len(), 0);

    // Old session A is still safely persisted in repository
    let stored_1 = session_repo.get_session(&id_1).await.unwrap().unwrap();
    assert_eq!(stored_1.messages.len(), 1);
    assert_eq!(stored_1.messages[0].content, "Message in session 1");
}

#[tokio::test]
async fn test_phase21_explicit_session_resume_and_invalid_fallback() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let sessions_dir = dir.path().join("sessions");
    let creds_path = dir.path().join("credentials.json");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let session_repo = Arc::new(FileSessionRepository::with_dir(&sessions_dir));
    let credential_backend = Arc::new(FileCredentialBackend::with_path(&creds_path));
    let event_bus = EventBus::new();

    let mut app = HadesApp::with_backends(
        config_service,
        storage_service,
        event_bus,
        credential_backend,
        session_repo.clone(),
    );
    app.init().expect("init");

    let session = app
        .create_new_session(Some("Research Session".to_string()))
        .await
        .expect("create");
    let sid = session.metadata.id.clone();
    if let Some(s) = app.active_session_mut() {
        s.add_message(Message::user(&sid, "Query to resume"));
    }
    app.save_active_session().await.expect("save");

    // Explicit resume with matching session ID
    let warn = app.init_session(Some(&sid)).await.expect("explicit resume");
    assert!(warn.is_none());
    assert_eq!(app.active_session().unwrap().metadata.id, sid);
    assert_eq!(app.active_session().unwrap().messages.len(), 1);
    assert_eq!(
        app.active_session().unwrap().messages[0].content,
        "Query to resume"
    );

    // Explicit resume with non-existent session ID -> returns clear error message and usable fresh session
    let missing_warn = app
        .init_session(Some("missing-session-uuid"))
        .await
        .expect("missing resume");
    assert!(missing_warn.is_some());
    assert!(missing_warn
        .unwrap()
        .contains("Hades could not find session: missing-session-uuid"));
    assert!(app.active_session().is_some());
    assert_ne!(
        app.active_session().unwrap().metadata.id,
        "missing-session-uuid"
    );
}

#[tokio::test]
async fn test_phase21_session_rename_and_deletion_safety() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let sessions_dir = dir.path().join("sessions");
    let creds_path = dir.path().join("credentials.json");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let session_repo = Arc::new(FileSessionRepository::with_dir(&sessions_dir));
    let credential_backend = Arc::new(FileCredentialBackend::with_path(&creds_path));
    let event_bus = EventBus::new();

    let mut app = HadesApp::with_backends(
        config_service,
        storage_service,
        event_bus,
        credential_backend,
        session_repo.clone(),
    );
    app.init().expect("init");

    let s1 = app
        .create_new_session(Some("Session 1".to_string()))
        .await
        .expect("create 1");
    let sid1 = s1.metadata.id.clone();

    let s2 = app
        .create_new_session(Some("Session 2".to_string()))
        .await
        .expect("create 2");
    let sid2 = s2.metadata.id.clone();

    // Rename session 1
    app.rename_session(&sid1, "Renamed Session 1")
        .await
        .expect("rename 1");
    let stored1 = session_repo.get_session(&sid1).await.unwrap().unwrap();
    assert_eq!(stored1.metadata.title, "Renamed Session 1");

    // Delete session 2 (which is currently active)
    let deleted = app.delete_session(&sid2).await.expect("delete 2");
    assert!(deleted);
    assert!(session_repo.get_session(&sid2).await.unwrap().is_none());

    // Active session was replaced by a fresh session
    assert!(app.active_session().is_some());
    assert_ne!(app.active_session().unwrap().metadata.id, sid2);

    // Session 1 is intact
    let list = session_repo.list_sessions().await.unwrap();
    assert!(list
        .iter()
        .any(|s| s.id == sid1 && s.title == "Renamed Session 1"));
}

#[tokio::test]
async fn test_phase21_resume_with_unavailable_model_graceful_recovery() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let sessions_dir = dir.path().join("sessions");
    let creds_path = dir.path().join("credentials.json");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let session_repo = Arc::new(FileSessionRepository::with_dir(&sessions_dir));
    let credential_backend = Arc::new(FileCredentialBackend::with_path(&creds_path));
    let event_bus = EventBus::new();

    let mut app = HadesApp::with_backends(
        config_service,
        storage_service,
        event_bus,
        credential_backend,
        session_repo.clone(),
    );
    app.init().expect("init");

    // Create session with a deprecated/unavailable model
    let session = app
        .session_repository()
        .create_session(
            Some("Deprecated Model Session".to_string()),
            Some("non-existent-provider".to_string()),
            Some("deprecated-model-v0".to_string()),
        )
        .await
        .expect("create");
    let sid = session.metadata.id.clone();

    // Explicit resume does NOT crash; it restores the session and returns a warning
    let warning = app
        .init_session(Some(&sid))
        .await
        .expect("resume session with missing model");
    assert!(warning.is_some());
    assert!(warning
        .unwrap()
        .contains("Original model unavailable: non-existent-provider/deprecated-model-v0"));
    assert_eq!(app.active_session().unwrap().metadata.id, sid);
}

#[tokio::test]
async fn test_phase3_workspace_and_tools_commands() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let event_bus = EventBus::new();

    let mut app = HadesApp::new(config_service, storage_service, event_bus);
    app.init().expect("init");
    app.transition_to(AppState::Running).expect("running");

    // 1. /workspace command
    let ws_out = app
        .execute_command("/workspace")
        .expect("execute /workspace");
    match ws_out {
        CommandOutput::Text(t) => {
            assert!(t.contains("WORKSPACE OVERVIEW"));
            assert!(t.contains("Root Path:"));
            assert!(t.contains("Project Type:"));
        }
        _ => panic!("Expected text output for /workspace"),
    }

    // 2. /tools command
    let tools_out = app.execute_command("/tools").expect("execute /tools");
    match tools_out {
        CommandOutput::Text(t) => {
            assert!(t.contains("HADES TOOLS & CAPABILITIES"));
            assert!(t.contains("filesystem.read"));
            assert!(t.contains("filesystem.edit"));
            assert!(t.contains("shell.execute"));
            assert!(t.contains("environment.get"));
        }
        _ => panic!("Expected text output for /tools"),
    }

    // 3. /permissions command
    let perm_out = app
        .execute_command("/permissions")
        .expect("execute /permissions");
    match perm_out {
        CommandOutput::Text(t) => {
            assert!(t.contains("SECURITY & PERMISSION POLICY"));
            assert!(t.contains("SAFE:"));
            assert!(t.contains("CRITICAL:"));
        }
        _ => panic!("Expected text output for /permissions"),
    }
}

#[tokio::test]
async fn test_phase3_tool_execution_and_approval_lifecycle() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let ws_dir = dir.path().join("workspace");
    std::fs::create_dir_all(&ws_dir).expect("create ws");

    let file_to_read = ws_dir.join("main.rs");
    std::fs::write(&file_to_read, "fn main() { println!(\"hello\"); }\n").expect("write file");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let event_bus = EventBus::new();
    let mut rx = event_bus.subscribe();

    let mut app = HadesApp::new(config_service, storage_service, event_bus);
    app.init().expect("init");
    app.set_workspace(&ws_dir);
    let _ = app
        .create_new_session(Some("Tool Test Session".to_string()))
        .await;
    app.transition_to(AppState::Running).expect("running");

    // 1. Execute Safe tool (filesystem.read) -> Auto-permitted
    let read_call = hades_tools::ToolCall::new(
        "call-1",
        "filesystem.read",
        serde_json::json!({ "path": "main.rs" }),
    );
    let read_res = app
        .execute_tool_call(read_call)
        .await
        .expect("execute read");
    assert_eq!(read_res.status, hades_tools::ToolStatus::Success);
    assert!(read_res.output.contains("fn main()"));

    // 2. Execute Mutating tool (filesystem.edit) -> Requires approval
    let edit_call = hades_tools::ToolCall::new(
        "call-2",
        "filesystem.edit",
        serde_json::json!({
            "path": "main.rs",
            "old_content": "println!(\"hello\");",
            "new_content": "println!(\"hades\");"
        }),
    );
    let edit_initial = app
        .execute_tool_call(edit_call)
        .await
        .expect("execute edit");
    assert_eq!(app.state(), AppState::ToolApproval);
    assert_eq!(
        edit_initial.status,
        hades_tools::ToolStatus::PermissionDenied
    );
    assert!(app.pending_approval().is_some());

    // 3. User Approves with AllowOnce
    let approved_res = app
        .resolve_pending_approval(hades_tools::ApprovalDecision::AllowOnce)
        .await
        .expect("resolve approval");
    assert_eq!(approved_res.status, hades_tools::ToolStatus::Success);
    assert_eq!(app.state(), AppState::Running);

    let updated_content = std::fs::read_to_string(&file_to_read).expect("read updated file");
    assert!(updated_content.contains("println!(\"hades\");"));

    // 4. Verify specific audit events received
    let mut events = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        events.push(ev);
    }
    assert!(events
        .iter()
        .any(|e| matches!(e, HadesEvent::ToolRequested { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, HadesEvent::ToolApprovalRequested { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, HadesEvent::ToolApproved { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, HadesEvent::FileModified { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, HadesEvent::ToolCompleted { .. })));
}

#[tokio::test]
async fn test_phase3_security_path_traversal_blocking() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let ws_dir = dir.path().join("workspace");
    std::fs::create_dir_all(&ws_dir).expect("create ws");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let event_bus = EventBus::new();

    let mut app = HadesApp::new(config_service, storage_service, event_bus);
    app.init().expect("init");
    app.set_workspace(&ws_dir);
    let _ = app
        .create_new_session(Some("Security Test".to_string()))
        .await;
    app.transition_to(AppState::Running).expect("running");

    // Attempt path traversal escaping workspace
    let traversal_call = hades_tools::ToolCall::new(
        "call-evil",
        "filesystem.read",
        serde_json::json!({ "path": "../../etc/shadow" }),
    );
    let res = app
        .execute_tool_call(traversal_call)
        .await
        .expect("execute traversal");
    // Traversal must be rejected and denied
    assert_eq!(res.status, hades_tools::ToolStatus::PermissionDenied);
}

#[tokio::test]
async fn test_phase3_real_tool_call_flow_and_session_persistence() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let ws_dir = dir.path().join("workspace");
    std::fs::create_dir_all(&ws_dir).expect("create ws");

    // Create a real file in workspace
    let sample_file = ws_dir.join("sample.txt");
    std::fs::write(&sample_file, "Hades Real Tool Execution Baseline Content")
        .expect("write sample");

    let config_service = ConfigService::with_path(&config_path);
    let storage_service = StorageService::with_root(&data_dir);
    let event_bus = EventBus::new();

    let mut app = HadesApp::new(config_service, storage_service, event_bus);
    app.init().expect("init");
    app.set_workspace(&ws_dir);
    let _session = app
        .create_new_session(Some("Tool Calling Test".to_string()))
        .await
        .expect("create session");
    app.transition_to(AppState::Running).expect("running");

    // 1. Execute Safe Tool: filesystem.read
    let read_call = hades_tools::ToolCall::new(
        "call-read-1",
        "filesystem.read",
        serde_json::json!({ "path": "sample.txt" }),
    );
    let read_result = app
        .execute_tool_call(read_call)
        .await
        .expect("execute read tool");

    assert_eq!(read_result.status, hades_tools::ToolStatus::Success);
    assert!(read_result
        .output
        .contains("Hades Real Tool Execution Baseline Content"));

    // Verify tool result was persisted in active session
    let s = app.active_session().expect("session exists");
    let tool_msg = s
        .messages
        .iter()
        .find(|m| m.role == hades_storage::MessageRole::Tool)
        .expect("tool message recorded");
    assert_eq!(
        tool_msg.metadata.tool_call_id.as_deref(),
        Some("call-read-1")
    );
    assert!(tool_msg
        .content
        .contains("Hades Real Tool Execution Baseline Content"));

    // Verify context manager maps tool result with proper role and tool_call_id
    let (chat_messages, _report) = app
        .context_manager()
        .build_context(&s.messages, "mock-model", None, "")
        .expect("build context");
    let tool_chat_msg = chat_messages
        .iter()
        .find(|m| m.role == hades_provider::MessageRole::Tool)
        .expect("tool role in chat messages");
    assert_eq!(tool_chat_msg.tool_call_id.as_deref(), Some("call-read-1"));
    assert!(tool_chat_msg
        .content
        .as_deref()
        .unwrap()
        .contains("Hades Real Tool Execution Baseline Content"));

    // 2. Execute Mutating Tool: filesystem.delete (Requires Approval)
    let delete_call = hades_tools::ToolCall::new(
        "call-delete-2",
        "filesystem.delete",
        serde_json::json!({ "path": "sample.txt" }),
    );
    let _ = app
        .execute_tool_call(delete_call)
        .await
        .expect("execute delete");
    assert_eq!(app.state(), AppState::ToolApproval);
    assert!(app.pending_approval().is_some());

    // Resolve approval
    let resolved = app
        .resolve_pending_approval(hades_tools::ApprovalDecision::AllowOnce)
        .await
        .expect("resolve approval");
    assert_eq!(resolved.status, hades_tools::ToolStatus::Success);
    assert_eq!(app.state(), AppState::Running);

    // Verify file deleted on disk
    assert!(!sample_file.exists());
}
