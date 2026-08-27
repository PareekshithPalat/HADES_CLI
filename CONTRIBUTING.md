# Contributing to HADES

Thank you for your interest in contributing to **HADES**! 🜲

Hades is an open-source, universal AI agent CLI runtime engineered in Rust. Our goal is to provide developers with a single, fast, secure, transparent, and completely user-controlled AI agent interface across any provider, machine, and workflow.

Whether you are fixing a bug, adding a new provider adapter, writing a diagnostic tool, optimizing the terminal UI, or improving documentation, we welcome your contributions!

---

## 📜 Table of Contents

- [Code of Conduct](#-code-of-conduct)
- [Development Prerequisites](#-development-prerequisites)
- [Repository Structure & Crates](#-repository-structure--crates)
- [Development Workflow](#-development-workflow)
- [Extending Hades](#-extending-hades)
  - [Adding a New Tool](#1-adding-a-new-tool)
  - [Adding a New Provider Adapter](#2-adding-a-new-provider-adapter)
  - [Adding a New Slash Command](#3-adding-a-new-slash-command)
- [Code Quality & Testing Standards](#-code-quality--testing-standards)
- [Submitting a Pull Request](#-submitting-a-pull-request)
- [Community & Support](#-community--support)

---

## 🤝 Code of Conduct

We are committed to providing a friendly, safe, welcoming, and harassment-free environment for all contributors, regardless of experience level, background, or identity.

- **Be respectful and constructive** in code reviews, discussions, and issue tracking.
- **Focus on what is best for the project and community**.
- **Show empathy** towards other maintainers and contributors.

---

## 🛠️ Development Prerequisites

To build and contribute to Hades, you will need:

1. **Rust Toolchain**:
   - Latest stable Rust (1.80+ recommended).
   - Install via [rustup](https://rustup.rs/):
     ```bash
     rustup update stable
     rustup component add rustfmt clippy
     ```

2. **C / C++ Compiler & Build Tools**:
   - **Windows**: MinGW-w64 (e.g. WinLibs POSIX) or MSVC C++ Build Tools.
   - **Linux**: `build-essential`, `pkg-config`, `libssl-dev`
     ```bash
     sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev
     ```
   - **macOS**: Xcode Command Line Tools
     ```bash
     xcode-select --install
     ```

---

## 🏗️ Repository Structure & Crates

Hades is organized as a modular Cargo workspace consisting of 8 focused crates:

```text
Hades-Cli/
├── assets/                  # Graphics, banners, and documentation assets
├── crates/
│   ├── hades-cli/           # Main executable, CLI argument parsing (clap), logging
│   ├── hades-config/        # Configuration management (TOML), schema validation
│   ├── hades-core/          # Core coordinator, state machine, context & command registry
│   ├── hades-events/        # Internal async Pub/Sub event bus
│   ├── hades-provider/      # Multi-provider adapters, LLM capability engine, streaming
│   ├── hades-storage/       # Session repository, conversation persistence, time formatting
│   ├── hades-tools/         # 29 built-in sandboxed tools, permission & security engine
│   └── hades-tui/           # Ratatui full-screen TUI, 5-region layout, clipboard, themes
├── Cargo.toml               # Workspace root manifest
├── CONTRIBUTING.md          # Contribution guidelines
├── LICENSE                  # MIT License
└── README.md                # Project documentation
```

### Architectural Principles

1. **The Model is the Brain, Hades is the Control Plane**: The LLM suggests actions, but Hades governs execution, sandboxing, and security.
2. **User is the Ultimate Authority**: Dangerous or mutating actions (`system.process.terminate`, destructive shell commands) require interactive user approval.
3. **Workspace Sandboxing**: Filesystem operations are strictly validated against workspace boundaries to prevent directory traversal attacks.
4. **Secret Protection**: API tokens, private keys, and environment credentials must always be redacted in display outputs, logs, and state objects.

---

## 🚀 Development Workflow

### 1. Fork and Clone

```bash
git clone https://github.com/your-username/Hades-Cli.git
cd Hades-Cli
git checkout -b feat/your-feature-name
```

### 2. Building the Project

```bash
# Build all crates in debug mode
cargo build --workspace

# Build optimized release binary
cargo build --release
```

### 3. Running Hades Locally

```bash
# Run with default settings
cargo run -p hades-cli

# Run pointing to a custom workspace or config
cargo run -p hades-cli -- --config ~/.hades/config.toml
```

---

## 🧩 Extending Hades

### 1. Adding a New Tool

All agent tools reside in `crates/hades-tools/src/` and implement the `Tool` trait.

1. **Create the Tool Struct and Implement `Tool`**:

   ```rust
   use async_trait::async_trait;
   use serde_json::json;
   use crate::context::ToolContext;
   use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};

   pub struct MyCustomTool;

   #[async_trait]
   impl Tool for MyCustomTool {
       fn definition(&self) -> ToolDefinition {
           ToolDefinition::new(
               "custom.tool_name",
               "Clear, detailed description of what the tool accomplishes.",
               json!({
                   "type": "object",
                   "properties": {
                       "query": {
                           "type": "string",
                           "description": "Description of parameter"
                       }
                   },
                   "required": ["query"],
                   "additionalProperties": false
               }),
               RiskLevel::Safe, // or RiskLevel::High if it mutates host state
               false,           // is_mutating: true requires user approval modal
           )
       }

       async fn execute(
           &self,
           call_id: &str,
           input: serde_json::Value,
           context: &ToolContext,
       ) -> ToolResult {
           let query = match input.get("query").and_then(|v| v.as_str()) {
               Some(q) => q,
               None => return ToolResult::invalid_input(call_id, "custom.tool_name", "Missing 'query'"),
           };

           // Execute safe logic
           let result_text = format!("Executed query: {query}");
           ToolResult::success(call_id, "custom.tool_name", result_text)
       }
   }
   ```

2. **Register the Tool** in `crates/hades-tools/src/registry.rs` within `ToolRegistry::default_registry()`.
3. **Add Comprehensive Unit Tests** in a `#[cfg(test)]` module within the tool's file.

---

### 2. Adding a New Provider Adapter

Provider adapters reside in `crates/hades-provider/src/adapters/` and implement the `Provider` trait.

1. Implement `Provider::name`, `Provider::list_models`, `Provider::verify_credentials`, `Provider::complete`, and `Provider::complete_stream`.
2. Ensure token streaming yields `StreamEvent::Token` chunks asynchronously.
3. Wrap secrets in `CredentialSecret` to guarantee they are never leaked in logs.
4. Register the adapter in `crates/hades-provider/src/manager.rs`.

---

### 3. Adding a New Slash Command

1. Define a struct implementing `Command` in `crates/hades-core/src/command.rs`.
2. Implement `name()`, `description()`, and `execute(&self, context: &mut CommandContext)`.
3. Register the command in `CommandRegistry::default_registry()`.

---

## 🧪 Code Quality & Testing Standards

Before submitting your pull request, verify that all four quality gates pass with **zero errors and zero warnings**:

```bash
# 1. Code Formatting
cargo fmt --all -- --check

# 2. Workspace Compilation
cargo check --workspace --all-targets

# 3. Comprehensive Test Suite
cargo test --workspace

# 4. Strict Clippy Linter
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Writing Tests

- **Unit Tests**: Place in the same file inside `#[cfg(test)] mod tests { ... }`.
- **Integration Tests**: Place in `crates/hades-cli/tests/integration_tests.rs`.
- Ensure async tests utilize `#[tokio::test]`.
- Mock external network requests; never require live API keys for test suites.

---

## 📥 Submitting a Pull Request

1. **Commit Guidelines**:
   - Write clear, concise commit messages following the Conventional Commits format:
     - `feat: add system.gpu diagnostic tool`
     - `fix: resolve prompt text wrapping on narrow terminals`
     - `docs: update provider setup instructions`
     - `refactor: optimize layout chunk allocation in hades-tui`
2. **Push to Your Fork**:
   ```bash
   git push origin feat/your-feature-name
   ```
3. **Open a Pull Request**:
   - Open a PR against the `master` / `main` branch.
   - Describe the problem solved and changes introduced.
   - Attach screenshots or terminal recordings for any UI changes.
   - Confirm all quality checks pass.

---

## 💬 Community & Support

- **Issues**: Report bugs or suggest features via GitHub Issues.
- **Discussions**: Share agent workflows, custom tools, and ideas on GitHub Discussions.
- **License**: By contributing to Hades, you agree that your contributions will be licensed under the [MIT License](LICENSE).
