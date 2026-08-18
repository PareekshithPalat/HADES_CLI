use std::time::Duration;

use crossterm::event::{self, Event};
use futures::StreamExt;
use tracing::{error, info};

use crate::error::TuiError;
use crate::input::{InputHandler, KeyActionResult};
use crate::state::TuiState;
use crate::terminal::{init_terminal, restore_terminal};
use crate::ui;
use hades_core::{AppState, HadesApp};
use hades_provider::{Credential, Model, StreamEvent};

/// Runs the full-screen Ratatui UI event loop with application-owned scrollable conversation viewport.
pub struct TuiRunner;

impl TuiRunner {
    /// Starts the main UI rendering and event processing loop.
    pub async fn run(app: &mut HadesApp) -> Result<(), TuiError> {
        let mut terminal = init_terminal()?;
        let mut tui_state = TuiState::new();
        tui_state.providers = app.model_manager().list_providers();

        info!("Starting Hades Full-Screen Ratatui TUI event loop");

        let loop_result: Result<(), TuiError> = async {
            while app.state() != AppState::Exited {
                // Advance transient activity spinner
                tui_state.tick_spinner();

                // 1. Draw current UI frame with strictly partitioned geometry and scrollbar
                terminal.draw(|frame| {
                    ui::render(frame, app, &mut tui_state);
                })?;

                // 2. Poll for terminal input events (keyboard and mouse)
                if event::poll(Duration::from_millis(50))? {
                    match event::read()? {
                        Event::Key(key_event) if key_event.kind == event::KeyEventKind::Press => {
                            let action =
                                InputHandler::handle_key_event(key_event, app, &mut tui_state)?;
                            match action {
                                KeyActionResult::Quit => break,
                                KeyActionResult::Handled => {}
                                KeyActionResult::SelectProvider(provider_id) => {
                                    let cred = app
                                        .credential_backend()
                                        .get_credential(&provider_id)
                                        .await
                                        .unwrap_or_default()
                                        .unwrap_or_else(|| {
                                            Credential::with_api_key(&provider_id, "")
                                        });

                                    match app
                                        .model_manager_mut()
                                        .discover_models(&provider_id, &cred)
                                        .await
                                    {
                                        Ok(models) if !models.is_empty() => {
                                            tui_state.models = models;
                                            tui_state.selected_model_index = 0;
                                            app.transition_to(AppState::ModelSelect)?;
                                        }
                                        _ => {
                                            let fallback_models = match provider_id.as_str() {
                                                "groq" => vec![
                                                    Model::new(
                                                        "llama-3.3-70b-versatile",
                                                        "groq",
                                                        "Llama 3.3 70B Versatile",
                                                    ),
                                                    Model::new(
                                                        "llama-3.1-8b-instant",
                                                        "groq",
                                                        "Llama 3.1 8B Instant",
                                                    ),
                                                    Model::new(
                                                        "mixtral-8x7b-32768",
                                                        "groq",
                                                        "Mixtral 8x7B Instruct",
                                                    ),
                                                ],
                                                "ollama" => vec![
                                                    Model::new(
                                                        "llama3.2",
                                                        "ollama",
                                                        "Llama 3.2 (Local)",
                                                    ),
                                                    Model::new(
                                                        "qwen2.5-coder",
                                                        "ollama",
                                                        "Qwen 2.5 Coder (Local)",
                                                    ),
                                                    Model::new(
                                                        "mistral",
                                                        "ollama",
                                                        "Mistral 7B (Local)",
                                                    ),
                                                ],
                                                _ => vec![
                                                    Model::new(
                                                        "gpt-4o",
                                                        "openai",
                                                        "GPT-4o Frontier Multimodal",
                                                    ),
                                                    Model::new(
                                                        "gpt-4o-mini",
                                                        "openai",
                                                        "GPT-4o Mini Fast",
                                                    ),
                                                    Model::new("o1", "openai", "o1 Reasoning"),
                                                ],
                                            };
                                            tui_state.models = fallback_models;
                                            tui_state.selected_model_index = 0;
                                            app.transition_to(AppState::ModelSelect)?;
                                        }
                                    }
                                }
                                KeyActionResult::VerifyModel => {
                                    terminal.draw(|frame| {
                                        ui::render(frame, app, &mut tui_state);
                                    })?;

                                    let provider_id = tui_state
                                        .selected_model
                                        .as_ref()
                                        .map(|m| m.provider_id.clone())
                                        .unwrap_or_default();
                                    let model_id = tui_state
                                        .selected_model
                                        .as_ref()
                                        .map(|m| m.id.clone())
                                        .unwrap_or_default();
                                    let endpoint =
                                        if tui_state.custom_endpoint_input.trim().is_empty() {
                                            None
                                        } else {
                                            Some(tui_state.custom_endpoint_input.trim().to_string())
                                        };

                                    let mut cred = Credential::with_api_key(
                                        &provider_id,
                                        tui_state.credential_input.trim(),
                                    );
                                    cred.endpoint = endpoint;

                                    match app
                                        .verify_and_persist_active_model(
                                            &provider_id,
                                            &model_id,
                                            &cred,
                                        )
                                        .await
                                    {
                                        Ok(_) => {
                                            tui_state.clear_error();
                                            app.transition_to(AppState::Running)?;
                                        }
                                        Err(e) => {
                                            tui_state.verification_error = Some(e.to_string());
                                            tui_state.verification_action_index = 0;
                                            app.transition_to(AppState::VerificationFailed)?;
                                        }
                                    }
                                }
                                KeyActionResult::SubmitPrompt(prompt) => {
                                    // 1. Enter thinking state & render immediately showing user turn + activity state
                                    app.transition_to(AppState::AiThinking)?;
                                    tui_state.scroll_to_bottom();
                                    terminal.draw(|frame| {
                                        ui::render(frame, app, &mut tui_state);
                                    })?;

                                    // 2. Execute streaming request
                                    match app.send_prompt_stream(&prompt).await {
                                        Ok(mut stream) => {
                                            app.transition_to(AppState::AiStreaming)?;
                                            while let Some(item) = stream.next().await {
                                                // Non-blocking poll for scrolling events during active streaming
                                                if event::poll(Duration::from_millis(5))? {
                                                    match event::read()? {
                                                        Event::Key(k)
                                                            if k.kind
                                                                == event::KeyEventKind::Press =>
                                                        {
                                                            let _ = InputHandler::handle_key_event(
                                                                k,
                                                                app,
                                                                &mut tui_state,
                                                            );
                                                        }
                                                        Event::Mouse(m) => {
                                                            let _ =
                                                                InputHandler::handle_mouse_event(
                                                                    m,
                                                                    app,
                                                                    &mut tui_state,
                                                                );
                                                        }
                                                        _ => {}
                                                    }
                                                }

                                                match item {
                                                    Ok(StreamEvent::Delta(text)) => {
                                                        tui_state.tick_spinner();
                                                        if let Some(turn) =
                                                            tui_state.turns.last_mut()
                                                        {
                                                            turn.append_response_chunk(&text);
                                                        }
                                                        terminal.draw(|frame| {
                                                            ui::render(frame, app, &mut tui_state);
                                                        })?;
                                                    }
                                                    Ok(StreamEvent::Usage(usage)) => {
                                                        tui_state.current_usage = Some(usage);
                                                    }
                                                    Ok(StreamEvent::Finished(_)) => break,
                                                    Ok(StreamEvent::Started) => {}
                                                    Ok(StreamEvent::Error(err)) => {
                                                        if let Some(turn) =
                                                            tui_state.turns.last_mut()
                                                        {
                                                            turn.set_error(err);
                                                        }
                                                        break;
                                                    }
                                                    Err(err) => {
                                                        if let Some(turn) =
                                                            tui_state.turns.last_mut()
                                                        {
                                                            turn.set_error(err.to_string());
                                                        }
                                                        break;
                                                    }
                                                }
                                            }
                                            app.transition_to(AppState::Running)?;
                                        }
                                        Err(_) => {
                                            // Fallback to non-streaming execution
                                            match app.send_prompt(&prompt).await {
                                                Ok(resp) => {
                                                    if let Some(turn) = tui_state.turns.last_mut() {
                                                        turn.set_response(resp.content);
                                                    }
                                                }
                                                Err(e) => {
                                                    if let Some(turn) = tui_state.turns.last_mut() {
                                                        turn.set_error(e.to_string());
                                                    }
                                                }
                                            }
                                            app.transition_to(AppState::Running)?;
                                        }
                                    }
                                }
                            }
                        }
                        Event::Mouse(mouse_event) => {
                            let _ =
                                InputHandler::handle_mouse_event(mouse_event, app, &mut tui_state)?;
                        }
                        _ => {}
                    }
                }
            }
            Ok(())
        }
        .await;

        // 3. Always restore terminal safely upon exit
        if let Err(e) = restore_terminal() {
            error!(error = %e, "Failed to restore terminal state");
        }

        loop_result
    }
}
