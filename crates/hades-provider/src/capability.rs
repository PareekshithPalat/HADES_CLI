use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// High-level AI model capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    TextGeneration,
    Streaming,
    ToolCalling,
    StructuredOutput,
    Vision,
    AudioInput,
    Reasoning,
    LongContext,
    ImageGeneration,
    Embeddings,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TextGeneration => write!(f, "Text generation"),
            Self::Streaming => write!(f, "Streaming"),
            Self::ToolCalling => write!(f, "Tool calling"),
            Self::StructuredOutput => write!(f, "Structured output"),
            Self::Vision => write!(f, "Vision"),
            Self::AudioInput => write!(f, "Audio input"),
            Self::Reasoning => write!(f, "Reasoning"),
            Self::LongContext => write!(f, "Long context"),
            Self::ImageGeneration => write!(f, "Image generation"),
            Self::Embeddings => write!(f, "Embeddings"),
        }
    }
}

/// Explicit support state for an individual model capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityState {
    Supported,
    Unsupported,
    Unknown,
}

impl fmt::Display for CapabilityState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supported => write!(f, "✓"),
            Self::Unsupported => write!(f, "✗"),
            Self::Unknown => write!(f, "?"),
        }
    }
}

/// Structured collection of capability states for a specific model.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ModelCapabilities {
    states: HashMap<Capability, CapabilityState>,
}

impl ModelCapabilities {
    /// Creates a new empty `ModelCapabilities` collection.
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Sets the support state for a capability.
    pub fn set(&mut self, capability: Capability, state: CapabilityState) -> &mut Self {
        self.states.insert(capability, state);
        self
    }

    /// Gets the state of a capability (defaults to `Unknown` if not explicitly defined).
    pub fn get(&self, capability: Capability) -> CapabilityState {
        self.states
            .get(&capability)
            .copied()
            .unwrap_or(CapabilityState::Unknown)
    }

    /// Returns whether the capability is explicitly `Supported`.
    pub fn supports(&self, capability: Capability) -> bool {
        self.get(capability) == CapabilityState::Supported
    }

    /// Returns standard text model capabilities default (text + streaming supported, others unknown/unsupported).
    pub fn standard_text() -> Self {
        let mut caps = Self::new();
        caps.set(Capability::TextGeneration, CapabilityState::Supported);
        caps.set(Capability::Streaming, CapabilityState::Supported);
        caps.set(Capability::StructuredOutput, CapabilityState::Supported);
        caps.set(Capability::ToolCalling, CapabilityState::Unknown);
        caps.set(Capability::Vision, CapabilityState::Unsupported);
        caps.set(Capability::AudioInput, CapabilityState::Unsupported);
        caps.set(Capability::Reasoning, CapabilityState::Unknown);
        caps.set(Capability::LongContext, CapabilityState::Unknown);
        caps.set(Capability::ImageGeneration, CapabilityState::Unsupported);
        caps.set(Capability::Embeddings, CapabilityState::Unknown);
        caps
    }

    /// Returns an iterator over all defined capabilities and their states.
    pub fn iter(&self) -> impl Iterator<Item = (Capability, CapabilityState)> + '_ {
        const ALL: [Capability; 10] = [
            Capability::TextGeneration,
            Capability::Streaming,
            Capability::ToolCalling,
            Capability::StructuredOutput,
            Capability::Vision,
            Capability::AudioInput,
            Capability::Reasoning,
            Capability::LongContext,
            Capability::ImageGeneration,
            Capability::Embeddings,
        ];

        ALL.into_iter().map(move |cap| (cap, self.get(cap)))
    }
}
