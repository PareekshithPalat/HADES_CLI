pub mod error;
pub mod model;
pub mod service;

pub use error::ConfigError;
pub use model::{
    ActiveModelConfig, BrowserConfig, GeneralConfig, HadesConfig, McpConfig, McpServerConfig,
    McpTransportType, NotificationConfig, ProviderConfig, UiConfig, CURRENT_CONFIG_VERSION,
};
pub use service::ConfigService;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config_validation() {
        let config = HadesConfig::default();
        assert_eq!(config.version, CURRENT_CONFIG_VERSION);
        assert_eq!(config.general.app_name, "hades");
        assert_eq!(config.general.default_mode, "simple");
        assert_eq!(config.ui.theme, "dark");
        assert!(config.ui.show_status_bar);
        assert_eq!(config.model, None);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempdir().expect("create temp dir");
        let config_path = dir.path().join("config.toml");
        let service = ConfigService::with_path(&config_path);

        let config = HadesConfig {
            general: GeneralConfig {
                default_mode: "expert".to_string(),
                ..Default::default()
            },
            ui: UiConfig {
                theme: "light".to_string(),
                ..Default::default()
            },
            model: Some(ActiveModelConfig::new("openai", "gpt-4o")),
            ..Default::default()
        };

        service.save(&config).expect("save config");
        assert!(config_path.exists());

        let loaded = service.load().expect("load config");
        assert_eq!(loaded, config);
        assert_eq!(loaded.model.as_ref().unwrap().model_id, "gpt-4o");
    }

    #[test]
    fn test_sse_mcp_transport_roundtrip() {
        let dir = tempdir().expect("create temp dir");
        let config_path = dir.path().join("config.toml");
        let service = ConfigService::with_path(&config_path);
        let mut config = HadesConfig::default();
        config.mcp.servers.insert(
            "remote".to_string(),
            McpServerConfig {
                transport: McpTransportType::Sse,
                url: Some("https://example.com/sse".to_string()),
                ..Default::default()
            },
        );

        service.save(&config).expect("save config");
        assert!(std::fs::read_to_string(&config_path)
            .expect("read config")
            .contains("transport = \"sse\""));

        let loaded = service.load().expect("load config");
        assert_eq!(
            loaded.mcp.servers["remote"].transport,
            McpTransportType::Sse
        );
    }

    #[test]
    fn test_load_or_create_missing() {
        let dir = tempdir().expect("create temp dir");
        let config_path = dir.path().join("sub").join("config.toml");
        let service = ConfigService::with_path(&config_path);

        assert!(!config_path.exists());
        let loaded = service.load_or_create().expect("load or create");
        assert_eq!(loaded, HadesConfig::default());
        assert!(config_path.exists());
    }

    #[test]
    fn test_malformed_config() {
        let dir = tempdir().expect("create temp dir");
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, "invalid toml {[[[[").expect("write invalid file");

        let service = ConfigService::with_path(&config_path);
        let result = service.load();
        assert!(result.is_err());
        match result {
            Err(ConfigError::Parse { .. }) => {}
            _ => panic!("Expected Parse error on malformed config"),
        }
    }

    #[test]
    fn test_invalid_validation_empty_version() {
        let config = HadesConfig {
            version: "   ".to_string(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        match result {
            Err(ConfigError::Validation(msg)) => {
                assert!(msg.contains("version"));
            }
            _ => panic!("Expected Validation error"),
        }
    }

    #[test]
    fn test_invalid_model_config() {
        let config = HadesConfig {
            model: Some(ActiveModelConfig::new("  ", "gpt-4o")),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_provider_config_defaults_disable_local_total_timeout() {
        let provider = ProviderConfig::default();
        assert_eq!(provider.connect_timeout_secs, 10);
        assert_eq!(provider.local_request_timeout_secs, 0);
        assert!(provider.local_stream_idle_timeout_secs > 60);
        assert!(provider.cloud_stream_idle_timeout_secs > 0);
        assert!(provider.validate().is_ok());
    }

    #[test]
    fn test_provider_config_parses_partial_section() {
        let toml_str = r#"
            [provider]
            local_stream_idle_timeout_secs = 1800
        "#;
        let config: HadesConfig = toml::from_str(toml_str).expect("parse config");
        assert_eq!(config.provider.local_stream_idle_timeout_secs, 1800);
        assert_eq!(config.provider.connect_timeout_secs, 10);
    }

    #[test]
    fn test_provider_config_rejects_zero_connect_timeout() {
        let config = HadesConfig {
            provider: ProviderConfig {
                connect_timeout_secs: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
