use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::capability::ModelCapabilities;

/// Pricing metadata for model consumption where published.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PricingMetadata {
    pub input_cost_per_million: Option<f64>,
    pub output_cost_per_million: Option<f64>,
    pub currency: String,
}

impl Default for PricingMetadata {
    fn default() -> Self {
        Self {
            input_cost_per_million: None,
            output_cost_per_million: None,
            currency: "USD".to_string(),
        }
    }
}

/// Normalized Hades representation of an AI model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    /// Canonical model identifier (e.g., "gpt-4o", "llama-3.3-70b-versatile").
    pub id: String,

    /// Identifier of the provider hosting or serving this model.
    pub provider_id: String,

    /// Human-friendly display title.
    pub display_name: String,

    /// Descriptive summary of model architecture, capabilities, or typical workloads.
    pub description: String,

    /// Maximum context token window size, if known.
    pub context_window: Option<u32>,

    /// Detailed capability support flags.
    pub capabilities: ModelCapabilities,

    /// Published token pricing metadata, if available.
    pub pricing: Option<PricingMetadata>,

    /// Provider-specific or auxiliary metadata properties.
    pub metadata: HashMap<String, String>,
}

impl Model {
    /// Creates a new `Model` with standard defaults.
    pub fn new(
        id: impl Into<String>,
        provider_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        let id_str = id.into();
        let display_str = display_name.into();
        Self {
            id: id_str.clone(),
            provider_id: provider_id.into(),
            display_name: if display_str.is_empty() {
                id_str
            } else {
                display_str
            },
            description: "General-purpose language model.".to_string(),
            context_window: None,
            capabilities: ModelCapabilities::standard_text(),
            pricing: None,
            metadata: HashMap::new(),
        }
    }

    /// Formats the context window into a human-readable string (e.g. "128K", "8K", "Unknown").
    pub fn context_window_display(&self) -> String {
        match self.context_window {
            Some(tokens) => {
                if tokens >= 1_000_000 {
                    format!("{}M", tokens / 1_000_000)
                } else if tokens >= 1_000 {
                    format!("{}K", tokens / 1_000)
                } else {
                    format!("{} tokens", tokens)
                }
            }
            None => "Unknown".to_string(),
        }
    }
}
