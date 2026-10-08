//! Non-interactive prompt execution used by `hades --prompt`.
//!
//! Streams the model response to a writer (stdout in the CLI) and runs the same
//! multi-turn tool loop as the TUI. Tools that need interactive approval cannot be
//! approved without a terminal, so they are denied and the model is told why.

use std::io::Write;

use futures::StreamExt;
use hades_provider::{ProviderToolCall, StreamEvent, StreamResult, Usage};
use hades_tools::{ApprovalDecision, ToolCall, ToolResult, ToolStatus};

use crate::app::HadesApp;
use crate::error::CoreError;
use crate::state::AppState;

/// Maximum number of model turns spent on tool calls before giving up.
const MAX_TOOL_ITERATIONS: usize = 15;

const APPROVAL_UNAVAILABLE: &str = "This tool requires interactive user approval, which is unavailable in non-interactive --prompt mode. It was not executed.";

/// Summary of a completed headless run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeadlessOutcome {
    /// Tool calls that were executed.
    pub tools_executed: usize,
    /// Tool calls that were denied (by policy or because they needed approval).
    pub tools_denied: usize,
    /// Token usage reported for the final model turn, if any.
    pub usage: Option<Usage>,
}

impl HadesApp {
    /// Runs a single prompt without the TUI.
    ///
    /// Response text is streamed to `out`; tool activity and warnings go to `diag`
    /// so `out` stays clean for piping. Sound notifications are disabled for the run.
    pub async fn run_headless_prompt<O: Write, D: Write>(
        &mut self,
        prompt: &str,
        out: &mut O,
        diag: &mut D,
    ) -> Result<HeadlessOutcome, CoreError> {
        if self.model_manager().active_model_id().is_none() {
            return Err(CoreError::Runtime(
                "No active AI model configured. Run `hades` and select one with /model before using --prompt.".to_string(),
            ));
        }
        if prompt.trim().is_empty() {
            return Err(CoreError::Runtime("Prompt must not be empty.".to_string()));
        }

        self.set_notifications_enabled(false);

        let mut outcome = HeadlessOutcome::default();
        let mut ends_with_newline = true;
        let (mut stream, _report, mut message_id) = self.send_prompt_stream(prompt).await?;

        for iteration in 1.. {
            let (tool_calls, usage) = self
                .consume_headless_stream(stream, &message_id, out, &mut ends_with_newline)
                .await?;
            outcome.usage = usage.or(outcome.usage);

            let tool_calls = match tool_calls {
                Some(calls) if !calls.is_empty() => calls,
                _ => break,
            };

            if iteration >= MAX_TOOL_ITERATIONS {
                return Err(CoreError::Runtime(format!(
                    "Stopped after {MAX_TOOL_ITERATIONS} tool iterations without a final answer."
                )));
            }

            for tc in tool_calls {
                if self.run_headless_tool(&tc, diag).await? {
                    outcome.tools_executed += 1;
                } else {
                    outcome.tools_denied += 1;
                }
            }

            let (next, _report, next_id) = self.send_continuation_stream().await?;
            stream = next;
            message_id = next_id;
        }

        if !ends_with_newline {
            writeln!(out).map_err(io_error)?;
        }
        out.flush().map_err(io_error)?;
        Ok(outcome)
    }

    /// Streams one model turn to `out`, persisting the result to the session.
    async fn consume_headless_stream<O: Write>(
        &mut self,
        mut stream: StreamResult,
        message_id: &str,
        out: &mut O,
        ends_with_newline: &mut bool,
    ) -> Result<(Option<Vec<ProviderToolCall>>, Option<Usage>), CoreError> {
        let mut content = String::new();
        let mut usage = None;
        let mut tool_calls = None;
        let mut failure = None;

        while let Some(item) = stream.next().await {
            match item {
                Ok(StreamEvent::Delta(text)) => {
                    out.write_all(text.as_bytes()).map_err(io_error)?;
                    out.flush().map_err(io_error)?;
                    *ends_with_newline = text.ends_with('\n');
                    content.push_str(&text);
                }
                Ok(StreamEvent::ToolCallsReady(calls)) => tool_calls = Some(calls),
                Ok(StreamEvent::Usage(u)) => usage = Some(u),
                Ok(StreamEvent::Finished(_)) => break,
                Ok(StreamEvent::Started) | Ok(StreamEvent::ToolCallChunk { .. }) => {}
                Ok(StreamEvent::Error(message)) => {
                    failure = Some(CoreError::Runtime(message));
                    break;
                }
                Err(e) => {
                    failure = Some(CoreError::Provider(e));
                    break;
                }
            }
        }

        if failure.is_none() {
            if let Some(ref calls) = tool_calls {
                self.record_assistant_tool_calls(message_id, &content, calls)
                    .await?;
                return Ok((tool_calls, usage));
            }
        }

        self.finalize_streaming_response(message_id, &content, usage, failure.is_some())
            .await?;
        match failure {
            Some(e) => Err(e),
            None => Ok((None, usage)),
        }
    }

    /// Executes one tool call. Returns `true` if it ran, `false` if it was denied.
    async fn run_headless_tool<D: Write>(
        &mut self,
        tc: &ProviderToolCall,
        diag: &mut D,
    ) -> Result<bool, CoreError> {
        let name = &tc.function.name;
        let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
            .unwrap_or_else(|_| serde_json::Value::Object(Default::default()));
        let call = ToolCall::new(&tc.id, name, args);

        let result = self.execute_tool_call(call).await?;

        if self.state() == AppState::ToolApproval {
            self.resolve_pending_approval(ApprovalDecision::Deny)
                .await?;
            let denied = ToolResult::permission_denied(&tc.id, name, APPROVAL_UNAVAILABLE);
            self.persist_tool_result(&denied).await;
            let _ = writeln!(
                diag,
                "hades: skipped tool '{name}': requires interactive approval"
            );
            return Ok(false);
        }

        if result.status == ToolStatus::PermissionDenied {
            // Policy denials are not persisted by `execute_tool_call`; the model still
            // needs a tool result for this call id to continue the conversation.
            self.persist_tool_result(&result).await;
            let reason = result.error.as_deref().unwrap_or("permission denied");
            let _ = writeln!(diag, "hades: denied tool '{name}': {reason}");
            return Ok(false);
        }

        let _ = writeln!(diag, "hades: ran tool '{name}'");
        Ok(true)
    }
}

fn io_error(e: std::io::Error) -> CoreError {
    CoreError::Runtime(format!("Failed to write output: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use hades_config::ActiveModelConfig;
    use hades_provider::{
        CompletionRequest, CompletionResponse, Credential, FinishReason, MessageRole, Model,
        ModelCapabilities, Provider, ProviderError, ProviderMetadata,
    };
    use hades_storage::FileSessionRepository;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    /// Provider that replays one scripted stream per request and records the requests it saw.
    struct ScriptedProvider {
        metadata: ProviderMetadata,
        turns: Mutex<Vec<Vec<Result<StreamEvent, ProviderError>>>>,
        requests: Arc<Mutex<Vec<CompletionRequest>>>,
    }

    impl ScriptedProvider {
        fn new(
            turns: Vec<Vec<Result<StreamEvent, ProviderError>>>,
        ) -> (Self, Arc<Mutex<Vec<CompletionRequest>>>) {
            let requests = Arc::new(Mutex::new(Vec::new()));
            let provider = Self {
                metadata: ProviderMetadata {
                    id: "scripted".to_string(),
                    name: "Scripted".to_string(),
                    description: "Scripted test provider".to_string(),
                    default_endpoint: None,
                    supports_dynamic_model_discovery: false,
                    requires_api_key: false,
                    is_local: true,
                },
                turns: Mutex::new(turns.into_iter().rev().collect()),
                requests: requests.clone(),
            };
            (provider, requests)
        }
    }

    #[async_trait]
    impl Provider for ScriptedProvider {
        fn id(&self) -> &str {
            &self.metadata.id
        }
        fn metadata(&self) -> &ProviderMetadata {
            &self.metadata
        }
        async fn authenticate(&self, _: &Credential) -> Result<(), ProviderError> {
            Ok(())
        }
        async fn list_models(&self, _: &Credential) -> Result<Vec<Model>, ProviderError> {
            Ok(vec![Model::new("scripted-model", "scripted", "Scripted")])
        }
        async fn get_model(&self, id: &str, _: &Credential) -> Result<Model, ProviderError> {
            Ok(Model::new(id, "scripted", id))
        }
        fn capabilities(&self, _: &str) -> ModelCapabilities {
            ModelCapabilities::standard_text()
        }
        async fn complete(
            &self,
            _: CompletionRequest,
            _: &Credential,
        ) -> Result<CompletionResponse, ProviderError> {
            unreachable!("headless mode always streams")
        }
        async fn complete_stream(
            &self,
            request: CompletionRequest,
            _: &Credential,
        ) -> Result<StreamResult, ProviderError> {
            self.requests.lock().unwrap().push(request);
            let events = self
                .turns
                .lock()
                .unwrap()
                .pop()
                .expect("unexpected extra model turn");
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    fn text_turn(chunks: &[&str]) -> Vec<Result<StreamEvent, ProviderError>> {
        let mut events = vec![Ok(StreamEvent::Started)];
        events.extend(chunks.iter().map(|c| Ok(StreamEvent::Delta(c.to_string()))));
        events.push(Ok(StreamEvent::Finished(FinishReason::Stop)));
        events
    }

    fn tool_turn(name: &str, args: serde_json::Value) -> Vec<Result<StreamEvent, ProviderError>> {
        vec![
            Ok(StreamEvent::Started),
            Ok(StreamEvent::ToolCallsReady(vec![
                ProviderToolCall::function("call_1", name, args.to_string()),
            ])),
            Ok(StreamEvent::Finished(FinishReason::ToolCalls)),
        ]
    }

    fn app_with(
        turns: Vec<Vec<Result<StreamEvent, ProviderError>>>,
    ) -> (
        HadesApp,
        Arc<Mutex<Vec<CompletionRequest>>>,
        tempfile::TempDir,
    ) {
        let dir = tempdir().expect("temp dir");
        let config_service = hades_config::ConfigService::with_path(dir.path().join("c.toml"));
        let config = hades_config::HadesConfig {
            model: Some(ActiveModelConfig::new("scripted", "scripted-model")),
            ..Default::default()
        };
        config_service.save(&config).expect("save config");

        let mut app = HadesApp::with_backends(
            config_service,
            hades_storage::StorageService::with_root(dir.path().join("data")),
            hades_events::EventBus::new(),
            Arc::new(hades_provider::FileCredentialBackend::with_path(
                dir.path().join("credentials.json"),
            )),
            Arc::new(FileSessionRepository::with_dir(dir.path().join("sessions"))),
        );
        let (provider, requests) = ScriptedProvider::new(turns);
        app.model_manager_mut()
            .register_provider(Arc::new(provider));
        app.init().expect("init");
        app.set_workspace(dir.path());
        (app, requests, dir)
    }

    #[tokio::test]
    async fn test_streams_response_to_output() {
        let (mut app, _requests, _dir) = app_with(vec![text_turn(&["2 + 2 ", "= 4"])]);
        let mut out = Vec::new();
        let mut diag = Vec::new();

        let outcome = app
            .run_headless_prompt("What is 2+2?", &mut out, &mut diag)
            .await
            .expect("headless run");

        assert_eq!(String::from_utf8(out).unwrap(), "2 + 2 = 4\n");
        assert!(diag.is_empty());
        assert_eq!(outcome, HeadlessOutcome::default());

        let session = app.active_session().expect("session");
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[1].content, "2 + 2 = 4");
    }

    #[tokio::test]
    async fn test_safe_tool_runs_and_result_reaches_model() {
        let (mut app, requests, _dir) = app_with(vec![
            tool_turn("system.platform", serde_json::json!({})),
            text_turn(&["You are on a computer.\n"]),
        ]);
        let mut out = Vec::new();
        let mut diag = Vec::new();

        let outcome = app
            .run_headless_prompt("What OS is this?", &mut out, &mut diag)
            .await
            .expect("headless run");

        assert_eq!(outcome.tools_executed, 1);
        assert_eq!(outcome.tools_denied, 0);
        assert_eq!(String::from_utf8(out).unwrap(), "You are on a computer.\n");
        assert!(String::from_utf8(diag)
            .unwrap()
            .contains("ran tool 'system.platform'"));

        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let last = requests[1].messages.last().expect("tool message");
        assert_eq!(last.role, MessageRole::Tool);
        assert_eq!(last.tool_call_id.as_deref(), Some("call_1"));
    }

    #[tokio::test]
    async fn test_tool_needing_approval_is_denied_without_running() {
        let (mut app, requests, dir) = app_with(vec![
            tool_turn(
                "filesystem.write",
                serde_json::json!({ "path": "out.txt", "content": "data" }),
            ),
            text_turn(&["I could not write the file."]),
        ]);
        let mut out = Vec::new();
        let mut diag = Vec::new();

        let outcome = app
            .run_headless_prompt("Write out.txt", &mut out, &mut diag)
            .await
            .expect("headless run");

        assert_eq!(outcome.tools_executed, 0);
        assert_eq!(outcome.tools_denied, 1);
        assert!(!dir.path().join("out.txt").exists());
        assert_eq!(app.state(), AppState::Running);
        assert!(String::from_utf8(diag)
            .unwrap()
            .contains("requires interactive approval"));

        let requests = requests.lock().unwrap();
        let tool_msg = requests[1].messages.last().expect("tool message");
        assert_eq!(tool_msg.role, MessageRole::Tool);
        assert!(tool_msg
            .content
            .as_deref()
            .unwrap()
            .contains("non-interactive --prompt mode"));
    }

    #[tokio::test]
    async fn test_stream_error_returns_err_and_keeps_partial_output() {
        let (mut app, _requests, _dir) = app_with(vec![vec![
            Ok(StreamEvent::Delta("partial".to_string())),
            Err(ProviderError::StreamError {
                provider: "scripted".to_string(),
                message: "connection reset".to_string(),
            }),
        ]]);
        let mut out = Vec::new();
        let mut diag = Vec::new();

        let result = app.run_headless_prompt("hi", &mut out, &mut diag).await;

        assert!(matches!(result, Err(CoreError::Provider(_))));
        assert_eq!(String::from_utf8(out).unwrap(), "partial");
        let session = app.active_session().expect("session");
        assert!(session.messages[1].metadata.is_interrupted);
    }

    #[tokio::test]
    async fn test_rejects_missing_model_and_empty_prompt() {
        let (mut app, _requests, _dir) = app_with(vec![]);
        let mut out = Vec::new();
        let mut diag = Vec::new();

        let empty = app.run_headless_prompt("   ", &mut out, &mut diag).await;
        assert!(matches!(empty, Err(CoreError::Runtime(_))));

        app.model_manager_mut().clear_active();
        let missing = app.run_headless_prompt("hi", &mut out, &mut diag).await;
        match missing {
            Err(CoreError::Runtime(msg)) => assert!(msg.contains("No active AI model")),
            other => panic!("expected missing model error, got {other:?}"),
        }
    }
}
