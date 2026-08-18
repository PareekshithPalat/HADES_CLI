use std::sync::Arc;
use tracing::{debug, info};

use crate::command::{CommandContext, CommandOutput, CommandRegistry};
use crate::error::CoreError;
use crate::state::AppState;
use hades_config::{ActiveModelConfig, ConfigService, HadesConfig};
use hades_events::{EventBus, HadesEvent};
use hades_provider::{
    CompletionRequest, CompletionResponse, Credential, CredentialBackend, FileCredentialBackend,
    Model, ModelManager, OpenAiProvider, StreamResult,
};
use hades_storage::{StorageHealth, StorageService};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Central core runtime managing application lifecycle, state, commands, providers, and subsystems.
pub struct HadesApp {
    state: AppState,
    config_service: ConfigService,
    config: HadesConfig,
    storage_service: StorageService,
    event_bus: EventBus,
    command_registry: CommandRegistry,
    model_manager: ModelManager,
    credential_backend: Arc<dyn CredentialBackend>,
    version: &'static str,
}

impl HadesApp {
    /// Creates a new `HadesApp` instance with default provider engine registrations.
    pub fn new(
        config_service: ConfigService,
        storage_service: StorageService,
        event_bus: EventBus,
    ) -> Self {
        let mut model_manager = ModelManager::new();

        // Register Phase 1 OpenAI-compatible provider suite
        model_manager.register_provider(Arc::new(OpenAiProvider::openai()));
        model_manager.register_provider(Arc::new(OpenAiProvider::groq()));
        model_manager.register_provider(Arc::new(OpenAiProvider::ollama()));
        model_manager.register_provider(Arc::new(OpenAiProvider::custom()));

        let credential_backend: Arc<dyn CredentialBackend> =
            match FileCredentialBackend::default_location() {
                Ok(backend) => Arc::new(backend),
                Err(_) => Arc::new(FileCredentialBackend::with_path(".hades/credentials.json")),
            };

        Self {
            state: AppState::Startup,
            config_service,
            config: HadesConfig::default(),
            storage_service,
            event_bus,
            command_registry: CommandRegistry::with_defaults(),
            model_manager,
            credential_backend,
            version: APP_VERSION,
        }
    }

    /// Creates a new `HadesApp` instance with a custom credential backend (e.g. for testing).
    pub fn with_credential_backend(
        config_service: ConfigService,
        storage_service: StorageService,
        event_bus: EventBus,
        credential_backend: Arc<dyn CredentialBackend>,
    ) -> Self {
        let mut app = Self::new(config_service, storage_service, event_bus);
        app.credential_backend = credential_backend;
        app
    }

    /// Initializes all underlying subsystems, loads configuration and active model, or opens interactive setup.
    pub fn init(&mut self) -> Result<(), CoreError> {
        info!("Initializing Hades core runtime (version {})", self.version);

        // 1. Initialize storage
        self.storage_service.initialize()?;

        // 2. Load or create configuration
        self.config = self.config_service.load_or_create()?;
        self.event_bus
            .publish(HadesEvent::config_loaded(self.config_service.config_path()));

        // 3. Model & Provider initialization
        let mut model_activated = false;
        if let Some(ref model_cfg) = self.config.model {
            if self
                .model_manager
                .get_provider(&model_cfg.provider_id)
                .is_some()
            {
                self.model_manager
                    .set_active(&model_cfg.provider_id, &model_cfg.model_id);
                self.event_bus.publish(HadesEvent::model_loaded(
                    &model_cfg.provider_id,
                    &model_cfg.model_id,
                ));
                model_activated = true;
            }
        }

        // 4. Initial state determination:
        // If a valid model is already configured and active -> Running
        // Otherwise -> ProviderSelect (interactive setup on startup)
        if model_activated {
            self.transition_to(AppState::Running)?;
        } else {
            self.transition_to(AppState::ProviderSelect)?;
        }

        // 5. Publish startup event
        self.event_bus
            .publish(HadesEvent::app_started(self.version));

        info!(
            "Hades core runtime initialized successfully (state: {:?})",
            self.state
        );
        Ok(())
    }

    /// Returns current application state.
    pub fn state(&self) -> AppState {
        self.state
    }

    /// Returns the active configuration.
    pub fn config(&self) -> &HadesConfig {
        &self.config
    }

    /// Returns the storage service reference.
    pub fn storage(&self) -> &StorageService {
        &self.storage_service
    }

    /// Returns the event bus reference.
    pub fn events(&self) -> &EventBus {
        &self.event_bus
    }

    /// Returns the command registry reference.
    pub fn commands(&self) -> &CommandRegistry {
        &self.command_registry
    }

    /// Returns a mutable reference to the command registry.
    pub fn commands_mut(&mut self) -> &mut CommandRegistry {
        &mut self.command_registry
    }

    /// Returns the model manager reference.
    pub fn model_manager(&self) -> &ModelManager {
        &self.model_manager
    }

    /// Returns a mutable reference to the model manager.
    pub fn model_manager_mut(&mut self) -> &mut ModelManager {
        &mut self.model_manager
    }

    /// Returns the credential backend reference.
    pub fn credential_backend(&self) -> &Arc<dyn CredentialBackend> {
        &self.credential_backend
    }

    /// Returns the application version string.
    pub fn version(&self) -> &'static str {
        self.version
    }

    /// Returns formatted active model display string (e.g. "openai/gpt-4o" or "Not configured").
    pub fn active_model_display(&self) -> String {
        match (
            self.model_manager.active_provider_id(),
            self.model_manager.active_model_id(),
        ) {
            (Some(provider), Some(model)) => format!("{provider}/{model}"),
            _ => "Not configured".to_string(),
        }
    }

    /// Performs an explicit validated state transition.
    pub fn transition_to(&mut self, next_state: AppState) -> Result<(), CoreError> {
        self.state.check_transition(next_state)?;
        debug!(from = ?self.state, to = ?next_state, "Application state transition");
        self.state = next_state;
        Ok(())
    }

    /// Verifies provider authentication & model availability, persists credentials and config, and sets active.
    pub async fn verify_and_persist_active_model(
        &mut self,
        provider_id: &str,
        model_id: &str,
        credential: &Credential,
    ) -> Result<Model, CoreError> {
        info!(provider = %provider_id, model = %model_id, "Verifying and persisting model selection");

        self.event_bus
            .publish(HadesEvent::CredentialVerificationStarted {
                timestamp: chrono::Utc::now(),
                provider_id: provider_id.to_string(),
            });

        // 1. Verify with provider
        let verified_model = match self
            .model_manager
            .verify_provider_and_model(provider_id, model_id, credential)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                self.event_bus
                    .publish(HadesEvent::CredentialVerificationFailed {
                        timestamp: chrono::Utc::now(),
                        provider_id: provider_id.to_string(),
                        error: e.to_string(),
                    });
                return Err(CoreError::Provider(e));
            }
        };

        // 2. Persist credential in secure backend
        self.credential_backend.store_credential(credential).await?;

        // 3. Persist model configuration in config.toml
        self.config.model = Some(ActiveModelConfig {
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
            endpoint: credential.endpoint.clone(),
        });
        self.config_service.save(&self.config)?;
        self.event_bus
            .publish(HadesEvent::config_saved(self.config_service.config_path()));

        // 4. Activate in model manager
        self.model_manager.set_active(provider_id, model_id);

        // 5. Emit events
        self.event_bus
            .publish(HadesEvent::CredentialVerificationSucceeded {
                timestamp: chrono::Utc::now(),
                provider_id: provider_id.to_string(),
                model_id: model_id.to_string(),
            });
        self.event_bus
            .publish(HadesEvent::model_loaded(provider_id, model_id));

        Ok(verified_model)
    }

    /// Submits a user prompt to the active model provider for a single-turn completion.
    pub async fn send_prompt(&mut self, prompt: &str) -> Result<CompletionResponse, CoreError> {
        let (provider_id, model_id) = match (
            self.model_manager.active_provider_id(),
            self.model_manager.active_model_id(),
        ) {
            (Some(p), Some(m)) => (p.to_string(), m.to_string()),
            _ => {
                return Err(CoreError::Runtime(
                    "No active AI model configured. Use /model to configure one.".to_string(),
                ))
            }
        };

        let credential = self
            .credential_backend
            .get_credential(&provider_id)
            .await?
            .unwrap_or_else(|| Credential::with_api_key(&provider_id, ""));

        self.event_bus.publish(HadesEvent::ModelRequestStarted {
            timestamp: chrono::Utc::now(),
            provider_id: provider_id.clone(),
            model_id: model_id.clone(),
        });

        let request = CompletionRequest::single_prompt(&model_id, prompt);
        let response = match self.model_manager.complete(request, &credential).await {
            Ok(resp) => resp,
            Err(e) => {
                self.event_bus.publish(HadesEvent::ProviderErrorOccurred {
                    timestamp: chrono::Utc::now(),
                    provider_id,
                    error: e.to_string(),
                });
                return Err(CoreError::Provider(e));
            }
        };

        let total_tokens = response.usage.and_then(|u| u.total_tokens);
        self.event_bus.publish(HadesEvent::ModelResponseCompleted {
            timestamp: chrono::Utc::now(),
            provider_id,
            model_id,
            total_tokens,
        });

        Ok(response)
    }

    /// Submits a user prompt to the active model provider for a streaming completion.
    pub async fn send_prompt_stream(&mut self, prompt: &str) -> Result<StreamResult, CoreError> {
        let (provider_id, model_id) = match (
            self.model_manager.active_provider_id(),
            self.model_manager.active_model_id(),
        ) {
            (Some(p), Some(m)) => (p.to_string(), m.to_string()),
            _ => {
                return Err(CoreError::Runtime(
                    "No active AI model configured. Use /model to configure one.".to_string(),
                ))
            }
        };

        let credential = self
            .credential_backend
            .get_credential(&provider_id)
            .await?
            .unwrap_or_else(|| Credential::with_api_key(&provider_id, ""));

        self.event_bus.publish(HadesEvent::ModelRequestStarted {
            timestamp: chrono::Utc::now(),
            provider_id: provider_id.clone(),
            model_id: model_id.clone(),
        });

        let request = CompletionRequest::single_prompt(&model_id, prompt).with_stream(true);
        match self
            .model_manager
            .complete_stream(request, &credential)
            .await
        {
            Ok(stream) => Ok(stream),
            Err(e) => {
                self.event_bus.publish(HadesEvent::ProviderErrorOccurred {
                    timestamp: chrono::Utc::now(),
                    provider_id,
                    error: e.to_string(),
                });
                Err(CoreError::Provider(e))
            }
        }
    }

    /// Executes a command input string, publishing relevant lifecycle events.
    pub fn execute_command(&mut self, input: &str) -> Result<CommandOutput, CoreError> {
        self.event_bus.publish(HadesEvent::command_entered(input));

        let storage_health = self
            .storage_service
            .health()
            .unwrap_or_else(|e| StorageHealth {
                status: hades_storage::StorageStatus::Unhealthy(e.to_string()),
                root_dir: self.storage_service.root_dir().to_path_buf(),
                writable: false,
            });

        let active_model_str = self.active_model_display();
        let active_model_opt = if active_model_str == "Not configured" {
            None
        } else {
            Some(active_model_str.as_str())
        };

        let help_entries = self.command_registry.help_entries();
        let mut context = CommandContext::new(
            self.state,
            &self.config,
            &storage_health,
            active_model_opt,
            self.version,
            help_entries,
        );

        let result = self.command_registry.execute(input, &mut context);

        match result {
            Ok(output) => {
                self.event_bus
                    .publish(HadesEvent::command_executed(input, true));

                if context.open_model_setup_requested
                    || matches!(output, CommandOutput::OpenModelSetup)
                {
                    self.transition_to(AppState::ProviderSelect)?;
                } else if context.shutdown_requested || matches!(output, CommandOutput::Exit) {
                    self.request_shutdown(Some("Command exit requested".to_string()))?;
                }

                Ok(output)
            }
            Err(e) => {
                self.event_bus
                    .publish(HadesEvent::command_executed(input, false));
                self.event_bus
                    .publish(HadesEvent::error_occurred(e.to_string()));
                Err(CoreError::Command(e))
            }
        }
    }

    /// Initiates graceful application shutdown and cleanup.
    pub fn request_shutdown(&mut self, reason: Option<String>) -> Result<(), CoreError> {
        if self.state == AppState::ShuttingDown || self.state == AppState::Exited {
            return Ok(());
        }

        info!(reason = ?reason, "Shutting down Hades core runtime");
        self.transition_to(AppState::ShuttingDown)?;

        self.event_bus.publish(HadesEvent::app_shutdown(reason));

        self.transition_to(AppState::Exited)?;
        info!("Hades core runtime shutdown complete");
        Ok(())
    }
}
