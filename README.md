# 🔱 HADES — Universal AI Agent CLI

> **"Any model. Any provider. Any project. Any machine. Any task. One user-controlled AI agent."**

---

## 📌 Project Overview

**Hades** is an open-source, cross-platform, modular, and universal AI agent CLI runtime written in Rust. It provides a rich terminal interface, extensible command architecture, pluggable AI model & provider subsystem, resilient configuration, and event-driven runtime.

---

## 🚦 Current Status: Phase 1 (Model & Provider Engine)

Hades has completed **Phase 1: Model & Provider Engine**. This subsystem establishes a universal provider adapter architecture, capability detection, dynamic model discovery, secure credential storage, and asynchronous token streaming.

### Feature Status Matrix

| Subsystem / Feature | Status | Notes |
| :--- | :--- | :--- |
| **Cargo Workspace & Crates** | ✅ Implemented | Modular 7-crate architecture (`hades-provider` added) |
| **Core State Machine** | ✅ Implemented | Explicit lifecycle states & setup workflow transitions |
| **Command Architecture** | ✅ Implemented | Extensible registry (`/help`, `/status`, `/model`, `/exit`) |
| **Interactive Terminal UI** | ✅ Implemented | Screens for chat, setup, model details, masked credentials |
| **Command Palette** | ✅ Implemented | Activated via `/`, keyboard navigation (`↑`/`↓`/`Enter`/`Esc`) |
| **Provider Adapter Trait** | ✅ Implemented | Pluggable `Provider` trait supporting dynamic discovery & streaming |
| **OpenAI-Compatible Adapter** | ✅ Implemented | Real adapter supporting OpenAI, Groq, local Ollama, Custom |
| **Capability Engine** | ✅ Implemented | Explicit 10-point capability matrix (`✓`, `✗`, `?`) |
| **Credential Abstraction** | ✅ Implemented | `CredentialSecret` with redacting `Debug`/`Display` & file backend |
| **Configuration Persistence** | ✅ Implemented | Validated TOML storage (`ActiveModelConfig`) at `~/.hades/config.toml` |
| **Startup Model Restoration** | ✅ Implemented | Restores active model on launch or opens interactive setup |
| **Real-time SSE Streaming** | ✅ Implemented | Non-blocking token streaming and usage accounting |
| **Internal Event Bus** | ✅ Implemented | Asynchronous pub/sub event system with Phase 1 lifecycle events |
| **Structured Logging** | ✅ Implemented | Non-intrusive file tracing at `~/.hades/logs/hades.log` |
| **Terminal Restoration** | ✅ Implemented | Safe panic hooks and exit cleanup |
| **Conversation Persistence** | ⏳ Planned (Phase 2+) | Full chat history, sessions & checkpointing |
| **Tools & Agent Capabilities** | ⏳ Future (Phase 3+) | File, terminal, git, MCP integrations |

---

## 🏗️ Architecture

Hades follows a clean layered architecture:

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

### Crates Summary

- **`hades-cli`**: Binary application entry point, argument parsing with `clap`, and background log worker setup.
- **`hades-tui`**: Terminal user interface built with `ratatui` and `crossterm`. Manages interactive views, dialogs, masked credential input, streaming renders, and panic hooks.
- **`hades-core`**: Core runtime containing the state machine (`AppState`), command registry (`CommandRegistry`), and application controller (`HadesApp`).
- **`hades-provider`**: Pluggable AI provider abstraction (`Provider`), registry (`ModelManager`), capability framework, credential storage (`CredentialBackend`), and provider adapters (`OpenAiProvider`).
- **`hades-config`**: Configuration system supporting loading, saving, defaults, and schema validation.
- **`hades-events`**: Event infrastructure publishing system transitions, model lifecycle, and operational metrics.
- **`hades-storage`**: Key-value atomic file storage with health monitoring.

---

## 🚀 Getting Started

### Prerequisites

- **Rust toolchain** (1.80+ or latest stable)
- Supported Platforms: **Windows**, **macOS**, **Linux**

### Building

```bash
# Build all workspace crates
cargo build --workspace

# Build for release
cargo build --release
```

### Running

```bash
# Run Hades
cargo run -p hades-cli
```

### Command Line Options

```text
Usage: hades [OPTIONS]

Options:
  -c, --config <FILE>     Custom path to configuration file (defaults to ~/.hades/config.toml)
  -d, --data-dir <DIR>    Custom directory for persistent storage (defaults to ~/.hades/data)
  -l, --log-dir <DIR>     Custom directory for log files (defaults to ~/.hades/logs)
  -h, --help              Print help
  -V, --version           Print version
```

---

## ⌨️ Interactive Keyboard Controls

| Key | Context | Action |
| :--- | :--- | :--- |
| `[Text]` + `Enter` | Main Screen (Running) | Send prompt to active AI model |
| `/` | Main Screen (Empty Prompt) | Open Interactive Command Palette |
| `↑` / `↓` | Command / Provider / Model Select | Navigate items |
| `Enter` | Any Selection List | Select item / Confirm action |
| `Tab` | Credential Input | Toggle between API Key and Endpoint Override |
| `Esc` | Setup Modals | Go back to previous step / Cancel |
| `Esc` | Main Screen | Clear active messages & prompt |
| `Ctrl+C` | Any | Graceful interrupt and safe shutdown |

---

## 🔌 Supported Providers in Phase 1

1. **OpenAI**: `gpt-4o`, `gpt-4o-mini`, `o1`, `o3-mini`, plus dynamic `/v1/models` discovery.
2. **Groq**: Ultra-fast inference with `llama-3.3-70b-versatile`, `llama-3.1-8b-instant`, `mixtral-8x7b-32768`.
3. **Ollama**: Local, private offline LLM runtime (`llama3.2`, `qwen2.5-coder`, `mistral`, etc.).
4. **Custom OpenAI-Compatible**: Any endpoint compatible with the OpenAI specification (vLLM, LM Studio, DeepSeek, LocalAI, etc.).

---

## 🧪 Testing & Verification

Run the full verification suite:

```bash
# Check code formatting
cargo fmt --all -- --check

# Check compilation across all targets
cargo check --workspace

# Run all unit and integration tests
cargo test --workspace

# Run Clippy lints with warnings treated as errors
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
