pub mod app;
pub mod command;
pub mod error;
pub mod state;

pub use app::{HadesApp, APP_VERSION};
pub use command::{
    Command, CommandContext, CommandInfo, CommandOutput, CommandRegistry, ExitCommand, HelpCommand,
    HelpEntry, ModelCommand, StatusCommand, StatusInfo,
};
pub use error::{CommandError, CoreError};
pub use state::AppState;

#[cfg(test)]
mod tests {
    use super::*;
    use hades_config::ActiveModelConfig;
    use tempfile::tempdir;

    fn create_test_app() -> (HadesApp, tempfile::TempDir) {
        let dir = tempdir().expect("create temp dir");
        let config_path = dir.path().join("config.toml");
        let storage_path = dir.path().join("data");

        let config_service = hades_config::ConfigService::with_path(config_path);
        let storage_service = hades_storage::StorageService::with_root(storage_path);
        let event_bus = hades_events::EventBus::new();

        let app = HadesApp::new(config_service, storage_service, event_bus);
        (app, dir)
    }

    #[test]
    fn test_app_initialization_unconfigured_opens_provider_select() {
        let (mut app, _dir) = create_test_app();
        assert_eq!(app.state(), AppState::Startup);

        app.init().expect("app init");
        assert_eq!(app.state(), AppState::ProviderSelect);
    }

    #[test]
    fn test_app_initialization_with_model_enters_running() {
        let dir = tempdir().expect("create temp dir");
        let config_path = dir.path().join("config.toml");
        let storage_path = dir.path().join("data");

        let config_service = hades_config::ConfigService::with_path(&config_path);
        let config = hades_config::HadesConfig {
            model: Some(ActiveModelConfig::new("openai", "gpt-4o")),
            ..Default::default()
        };
        config_service.save(&config).expect("save config");

        let storage_service = hades_storage::StorageService::with_root(storage_path);
        let event_bus = hades_events::EventBus::new();

        let mut app = HadesApp::new(config_service, storage_service, event_bus);
        app.init().expect("app init");
        assert_eq!(app.state(), AppState::Running);
        assert_eq!(app.active_model_display(), "openai/gpt-4o");
    }

    #[test]
    fn test_state_machine_valid_and_invalid_transitions() {
        assert!(AppState::Startup.can_transition_to(AppState::Running));
        assert!(AppState::Startup.can_transition_to(AppState::ProviderSelect));
        assert!(AppState::Startup.can_transition_to(AppState::ShuttingDown));

        assert!(AppState::Running.can_transition_to(AppState::CommandPalette));
        assert!(AppState::Running.can_transition_to(AppState::ProviderSelect));
        assert!(AppState::Running.can_transition_to(AppState::ShuttingDown));

        assert!(AppState::ProviderSelect.can_transition_to(AppState::ModelSelect));
        assert!(AppState::ProviderSelect.can_transition_to(AppState::Running));
        assert!(AppState::ModelSelect.can_transition_to(AppState::ModelInfo));
        assert!(AppState::ModelInfo.can_transition_to(AppState::CredentialInput));
        assert!(AppState::CredentialInput.can_transition_to(AppState::Verifying));
        assert!(AppState::Verifying.can_transition_to(AppState::Running));
        assert!(AppState::Verifying.can_transition_to(AppState::VerificationFailed));
        assert!(AppState::VerificationFailed.can_transition_to(AppState::CredentialInput));

        assert!(AppState::ShuttingDown.can_transition_to(AppState::Exited));
        assert!(!AppState::ShuttingDown.can_transition_to(AppState::Running));
        assert!(!AppState::Exited.can_transition_to(AppState::Running));
    }

    #[test]
    fn test_help_command_execution() {
        let (mut app, _dir) = create_test_app();
        app.init().expect("app init");

        let output = app.execute_command("/help").expect("execute /help");
        match output {
            CommandOutput::Help(entries) => {
                let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
                assert!(names.contains(&"/help".to_string()));
                assert!(names.contains(&"/status".to_string()));
                assert!(names.contains(&"/model".to_string()));
                assert!(names.contains(&"/exit".to_string()));
            }
            _ => panic!("Expected Help output"),
        }
    }

    #[test]
    fn test_status_command_execution() {
        let (mut app, _dir) = create_test_app();
        app.init().expect("app init");
        app.transition_to(AppState::Running)
            .expect("transition to running");

        let output = app.execute_command("/status").expect("execute /status");
        match output {
            CommandOutput::Status(status) => {
                assert_eq!(status.application, "Running");
                assert_eq!(status.version, APP_VERSION);
                assert_eq!(status.model, "Not configured");
                assert_eq!(status.mode, "Simple");
                assert_eq!(status.storage_status, "Ready");
                assert_eq!(status.config_status, "Ready");
            }
            _ => panic!("Expected Status output"),
        }
    }

    #[test]
    fn test_model_command_triggers_provider_select() {
        let (mut app, _dir) = create_test_app();
        app.init().expect("app init");
        app.transition_to(AppState::Running)
            .expect("transition to running");

        let output = app.execute_command("/model").expect("execute /model");
        assert_eq!(output, CommandOutput::OpenModelSetup);
        assert_eq!(app.state(), AppState::ProviderSelect);
    }

    #[test]
    fn test_exit_command_triggers_shutdown() {
        let (mut app, _dir) = create_test_app();
        app.init().expect("app init");

        let output = app.execute_command("/exit").expect("execute /exit");
        assert_eq!(output, CommandOutput::Exit);
        assert_eq!(app.state(), AppState::Exited);
    }

    #[test]
    fn test_unknown_command() {
        let (mut app, _dir) = create_test_app();
        app.init().expect("app init");

        let result = app.execute_command("/doesnotexist");
        assert!(result.is_err());
        match result {
            Err(CoreError::Command(CommandError::UnknownCommand(cmd))) => {
                assert_eq!(cmd, "/doesnotexist");
            }
            _ => panic!("Expected UnknownCommand error"),
        }
    }

    #[test]
    fn test_empty_command() {
        let (mut app, _dir) = create_test_app();
        app.init().expect("app init");

        let result = app.execute_command("   ");
        assert!(result.is_err());
        match result {
            Err(CoreError::Command(CommandError::EmptyInput)) => {}
            _ => panic!("Expected EmptyInput error"),
        }
    }

    #[test]
    fn test_command_aliases() {
        let (mut app, _dir) = create_test_app();
        app.init().expect("app init");

        let output = app.execute_command("/h").expect("execute alias /h");
        assert!(matches!(output, CommandOutput::Help(_)));

        let output_provider = app
            .execute_command("/provider")
            .expect("execute alias /provider");
        assert_eq!(output_provider, CommandOutput::OpenModelSetup);
    }
}
