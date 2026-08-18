# Hades Architecture Documentation — Phase 1: Model & Provider Engine

## 1. Architectural Overview

Hades is designed around a modular, decoupled architecture where presentation, domain logic, provider adapters, and persistence subsystems interact through well-defined traits and asynchronous streams.

```text
               ┌───────────────────────┐
               │       hades-cli       │  (Executable entry point, CLI parsing, logging)
               └───────────┬───────────┘
                           │
               ┌───────────▼───────────┐
               │       hades-tui       │  (Terminal UI, rendering, key handling, modals)
               └───────────┬───────────┘
                           │
               ┌───────────▼───────────┐
               │      hades-core       │  (Application coordinator, state machine, commands)
               └─────┬───────────┬─────┘
                     │           │
        ┌────────────▼───┐   ┌───▼──────────────┐
        │ hades-provider │   │   hades-config   │
        │(AI Adapters,   │   │  (TOML config)   │
        │ Models, Keys)  │   └──────────────────┘
        └────────────┬───┘
                     │
          ┌──────────┴──────────┐
          │                     │
 ┌────────▼────────┐   ┌────────▼────────┐
 │  hades-events   │   │  hades-storage  │
 │ (Pub/Sub Bus)   │   │  (Data storage) │
 └─────────────────┘   └─────────────────┘
```

---

## 2. Provider Subsystem (`hades-provider`)

The `hades-provider` crate encapsulates all interactions with Large Language Models and external AI inference endpoints.

### Core Trait: `Provider`

All AI providers implement the asynchronous `Provider` trait:

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn metadata(&self) -> &ProviderMetadata;
    async fn authenticate(&self, cred: &Credential) -> Result<(), ProviderError>;
    async fn list_models(&self, cred: &Credential) -> Result<Vec<Model>, ProviderError>;
    async fn get_model(&self, model_id: &str, cred: &Credential) -> Result<Model, ProviderError>;
    fn capabilities(&self, model_id: &str) -> ModelCapabilities;
    async fn complete(&self, request: CompletionRequest, cred: &Credential) -> Result<CompletionResponse, ProviderError>;
    async fn complete_stream(&self, request: CompletionRequest, cred: &Credential) -> Result<StreamResult, ProviderError>;
}
```

### Provider Registry: `ModelManager`

`ModelManager` maintains registered providers, routes model requests, discovers models dynamically via remote APIs, and tracks the active provider and model.

### Adapter Pattern: `OpenAiProvider`

The `OpenAiProvider` adapter standardizes interactions across OpenAI, Groq, local Ollama instances, and custom OpenAI-compatible endpoints:
- Automatic base URL and authorization header configuration.
- Model discovery via `GET /v1/models`.
- Authentication verification via header inspection and test pings.
- Non-streaming completions via `POST /v1/chat/completions`.
- Real-time Server-Sent Events (SSE) streaming via `futures::stream::unfold` state machine.
- Token usage parsing from chunks and response metadata.
- Error normalization into typed `ProviderError` variants.

---

## 3. Capability Framework

Capabilities are tracked explicitly at the model level via `ModelCapabilities`:

- `Capability`: Enum representing specific features (`TextGeneration`, `Streaming`, `ToolCalling`, `StructuredOutput`, `Vision`, `AudioInput`, `Reasoning`, `LongContext`, `ImageGeneration`, `Embeddings`).
- `CapabilityState`: Tri-state value:
  - `Supported` (`✓`)
  - `Unsupported` (`✗`)
  - `Unknown` (`?`)

---

## 4. Credential Security & Redaction

- `CredentialSecret`: Wrapper around sensitive strings that implements `Zeroize` and redacts values in both `Debug` and `Display` implementations (`"[REDACTED]"`).
- `CredentialBackend`: Async trait for storing, retrieving, and deleting provider credentials.
- `FileCredentialBackend`: Local JSON storage at `~/.hades/credentials.json`.
- Credentials are never logged, formatted into error messages, or checked into version control.

---

## 5. Extended Application State Machine

The application lifecycle state machine in `hades-core` enforces strict transitions for configuration and setup workflows:

```text
               [Startup]
                   │
         +---------+---------+
         │                   │ (Unconfigured)
         ▼ (Configured)      ▼
     [Running] ◄───── [ProviderSelect]
       │   ▲                 │
       │   │                 ▼
       │   │           [ModelSelect]
       │   │                 │
       │   │                 ▼
       │   │            [ModelInfo]
       │   │                 │
       │   │                 ▼
       │   │          [CredentialInput]
       │   │                 │
       │   │                 ▼
       │   │            [Verifying]
       │   │             │       │
       │   │ (Success)   ▼       ▼ (Failure)
       │   +─────── [Running]   [VerificationFailed]
       │                             │
       │                             ▼
       │                    [CredentialInput] / [ModelSelect]
       │
       +──────► [CommandPalette]
       │
       +──────► [AiThinking] ──► [AiStreaming] ──► [Running]
       │
       ▼
  [ShuttingDown]
       │
       ▼
   [Exited]
```

---

## 6. How to Add a New Provider Adapter

To add a new provider (e.g., Anthropic, Google Gemini, Mistral AI):

1. **Implement `Provider` Trait**:
   Create `crates/hades-provider/src/adapters/<provider_name>.rs` implementing `Provider`.
2. **Define Metadata & Models**:
   Specify `ProviderMetadata` (id, display name, default endpoint, dynamic discovery flag).
3. **Handle Authentication & Errors**:
   Normalize provider HTTP status codes into typed `ProviderError` variants.
4. **Register in `ModelManager`**:
   In `crates/hades-core/src/app.rs`, instantiate the provider and call `model_manager.register_provider(Arc::new(...))`.
