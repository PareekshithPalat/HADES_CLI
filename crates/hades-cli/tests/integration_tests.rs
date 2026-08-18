use hades_config::{ActiveModelConfig, ConfigService};
use hades_core::{AppState, CommandOutput, HadesApp, APP_VERSION};
use hades_events::{EventBus, HadesEvent};
use hades_provider::{Credential, CredentialBackend, FileCredentialBackend};
use hades_storage::StorageService;
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
