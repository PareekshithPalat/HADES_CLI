use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tracing::{debug, info};

use crate::state::{ChatTurn, TuiState};
use hades_core::{AppState, CommandOutput, CoreError, HadesApp};

/// Outcome of processing an input event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyActionResult {
    /// Input event was processed internally by UI state machine.
    Handled,

    /// User submitted a prompt for AI model generation.
    SubmitPrompt(String),

    /// User selected a provider from the list to discover models.
    SelectProvider(String),

    /// User initiated provider credential and model verification.
    VerifyModel,

    /// Application should initiate graceful shutdown and terminate.
    Quit,
}

/// Decoupled handler translating Crossterm keyboard and mouse events into application state transitions.
pub struct InputHandler;

impl InputHandler {
    /// Processes a single `KeyEvent` against the current application state.
    pub fn handle_key_event(
        key_event: KeyEvent,
        app: &mut HadesApp,
        tui_state: &mut TuiState,
    ) -> Result<KeyActionResult, CoreError> {
        // Global Ctrl+C handler -> Immediate graceful shutdown
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && key_event.code == KeyCode::Char('c')
        {
            info!("Received Ctrl+C interrupt signal");
            app.request_shutdown(Some("SIGINT / Ctrl+C".to_string()))?;
            return Ok(KeyActionResult::Quit);
        }

        match app.state() {
            AppState::Running => Self::handle_running(key_event, app, tui_state),
            AppState::CommandPalette => Self::handle_command_palette(key_event, app, tui_state),
            AppState::ProviderSelect => Self::handle_provider_select(key_event, app, tui_state),
            AppState::ModelSelect => Self::handle_model_select(key_event, app, tui_state),
            AppState::ModelInfo => Self::handle_model_info(key_event, app, tui_state),
            AppState::CredentialInput => Self::handle_credential_input(key_event, app, tui_state),
            AppState::VerificationFailed => {
                Self::handle_verification_failed(key_event, app, tui_state)
            }
            _ => Ok(KeyActionResult::Handled),
        }
    }

    /// Processes a single `MouseEvent` (mouse-wheel scrolling).
    pub fn handle_mouse_event(
        mouse_event: MouseEvent,
        app: &mut HadesApp,
        tui_state: &mut TuiState,
    ) -> Result<KeyActionResult, CoreError> {
        if app.state() == AppState::Running
            || app.state() == AppState::AiThinking
            || app.state() == AppState::AiStreaming
        {
            match mouse_event.kind {
                MouseEventKind::ScrollUp => {
                    tui_state.scroll_up(3);
                    return Ok(KeyActionResult::Handled);
                }
                MouseEventKind::ScrollDown => {
                    tui_state.scroll_down(3);
                    return Ok(KeyActionResult::Handled);
                }
                _ => {}
            }
        }
        Ok(KeyActionResult::Handled)
    }

    fn handle_running(
        key_event: KeyEvent,
        app: &mut HadesApp,
        tui_state: &mut TuiState,
    ) -> Result<KeyActionResult, CoreError> {
        match key_event.code {
            // Open command palette on '/' when prompt input is empty
            KeyCode::Char('/') if tui_state.prompt_input.is_empty() => {
                tui_state.selected_palette_index = 0;
                tui_state.clear_error();
                app.transition_to(AppState::CommandPalette)?;
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Enter => {
                let input = tui_state.prompt_input.trim().to_string();
                if input.is_empty() {
                    return Ok(KeyActionResult::Handled);
                }

                tui_state.prompt_input.clear();
                tui_state.prompt_cursor_position = 0;

                if input.starts_with('/') {
                    // Execute CLI slash command
                    match app.execute_command(&input) {
                        Ok(output) => {
                            if matches!(output, CommandOutput::OpenModelSetup) {
                                tui_state.providers = app.model_manager().list_providers();
                                tui_state.selected_provider_index = 0;
                            } else {
                                tui_state.set_output(output);
                            }
                        }
                        Err(e) => {
                            tui_state.set_error(e.to_string());
                        }
                    }
                    Ok(KeyActionResult::Handled)
                } else {
                    // Add new user turn immediately to conversation stream
                    tui_state.turns.push(ChatTurn::new(&input));
                    tui_state.scroll_to_bottom();
                    tui_state.active_output = None;
                    tui_state.clear_error();

                    // Send user prompt to model runner
                    Ok(KeyActionResult::SubmitPrompt(input))
                }
            }
            KeyCode::Up => {
                if tui_state.prompt_input.is_empty() {
                    tui_state.scroll_up(1);
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Down => {
                if tui_state.prompt_input.is_empty() {
                    tui_state.scroll_down(1);
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::PageUp => {
                tui_state.page_up();
                Ok(KeyActionResult::Handled)
            }
            KeyCode::PageDown => {
                tui_state.page_down();
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Home => {
                if tui_state.prompt_input.is_empty() {
                    tui_state.scroll_to_top();
                } else {
                    tui_state.prompt_cursor_position = 0;
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::End => {
                if tui_state.prompt_input.is_empty() {
                    tui_state.scroll_to_bottom();
                } else {
                    tui_state.prompt_cursor_position = tui_state.prompt_input.len();
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Left => {
                if tui_state.prompt_cursor_position > 0 {
                    tui_state.prompt_cursor_position -= 1;
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Right => {
                if tui_state.prompt_cursor_position < tui_state.prompt_input.len() {
                    tui_state.prompt_cursor_position += 1;
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Char(c) => {
                tui_state.push_prompt_char(c);
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Backspace => {
                tui_state.pop_prompt_char();
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Esc => {
                tui_state.clear_error();
                tui_state.active_output = None;
                tui_state.prompt_input.clear();
                tui_state.prompt_cursor_position = 0;
                tui_state.scroll_to_bottom();
                Ok(KeyActionResult::Handled)
            }
            _ => Ok(KeyActionResult::Handled),
        }
    }

    fn handle_command_palette(
        key_event: KeyEvent,
        app: &mut HadesApp,
        tui_state: &mut TuiState,
    ) -> Result<KeyActionResult, CoreError> {
        let commands = app.commands().list();
        let count = commands.len();

        match key_event.code {
            KeyCode::Up => {
                if count > 0 {
                    tui_state.selected_palette_index = if tui_state.selected_palette_index == 0 {
                        count - 1
                    } else {
                        tui_state.selected_palette_index - 1
                    };
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Down => {
                if count > 0 {
                    tui_state.selected_palette_index =
                        (tui_state.selected_palette_index + 1) % count;
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Enter => {
                if let Some(cmd_info) = commands.get(tui_state.selected_palette_index) {
                    let cmd_name = cmd_info.name.clone();
                    debug!(command = %cmd_name, "Executing selected command from palette");

                    app.transition_to(AppState::Running)?;
                    match app.execute_command(&cmd_name) {
                        Ok(output) => {
                            if matches!(output, CommandOutput::OpenModelSetup) {
                                tui_state.providers = app.model_manager().list_providers();
                                tui_state.selected_provider_index = 0;
                            } else if matches!(output, CommandOutput::Exit) {
                                return Ok(KeyActionResult::Quit);
                            } else {
                                tui_state.set_output(output);
                            }
                        }
                        Err(e) => {
                            tui_state.set_error(e.to_string());
                        }
                    }
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Esc => {
                app.transition_to(AppState::Running)?;
                Ok(KeyActionResult::Handled)
            }
            _ => Ok(KeyActionResult::Handled),
        }
    }

    fn handle_provider_select(
        key_event: KeyEvent,
        app: &mut HadesApp,
        tui_state: &mut TuiState,
    ) -> Result<KeyActionResult, CoreError> {
        if tui_state.providers.is_empty() {
            tui_state.providers = app.model_manager().list_providers();
        }
        let count = tui_state.providers.len();

        match key_event.code {
            KeyCode::Up => {
                if count > 0 {
                    tui_state.selected_provider_index = if tui_state.selected_provider_index == 0 {
                        count - 1
                    } else {
                        tui_state.selected_provider_index - 1
                    };
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Down => {
                if count > 0 {
                    tui_state.selected_provider_index =
                        (tui_state.selected_provider_index + 1) % count;
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Enter => {
                if let Some(provider) = tui_state.providers.get(tui_state.selected_provider_index) {
                    let pid = provider.id.clone();
                    Ok(KeyActionResult::SelectProvider(pid))
                } else {
                    Ok(KeyActionResult::Handled)
                }
            }
            KeyCode::Esc => {
                let _ = app.transition_to(AppState::Running);
                Ok(KeyActionResult::Handled)
            }
            _ => Ok(KeyActionResult::Handled),
        }
    }

    fn handle_model_select(
        key_event: KeyEvent,
        app: &mut HadesApp,
        tui_state: &mut TuiState,
    ) -> Result<KeyActionResult, CoreError> {
        let count = tui_state.models.len();

        match key_event.code {
            KeyCode::Up => {
                if count > 0 {
                    tui_state.selected_model_index = if tui_state.selected_model_index == 0 {
                        count - 1
                    } else {
                        tui_state.selected_model_index - 1
                    };
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Down => {
                if count > 0 {
                    tui_state.selected_model_index = (tui_state.selected_model_index + 1) % count;
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Enter => {
                if let Some(model) = tui_state.models.get(tui_state.selected_model_index) {
                    tui_state.selected_model = Some(model.clone());
                    app.transition_to(AppState::ModelInfo)?;
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Esc => {
                app.transition_to(AppState::ProviderSelect)?;
                Ok(KeyActionResult::Handled)
            }
            _ => Ok(KeyActionResult::Handled),
        }
    }

    fn handle_model_info(
        key_event: KeyEvent,
        app: &mut HadesApp,
        _tui_state: &mut TuiState,
    ) -> Result<KeyActionResult, CoreError> {
        match key_event.code {
            KeyCode::Enter => {
                app.transition_to(AppState::CredentialInput)?;
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Esc => {
                app.transition_to(AppState::ModelSelect)?;
                Ok(KeyActionResult::Handled)
            }
            _ => Ok(KeyActionResult::Handled),
        }
    }

    fn handle_credential_input(
        key_event: KeyEvent,
        app: &mut HadesApp,
        tui_state: &mut TuiState,
    ) -> Result<KeyActionResult, CoreError> {
        match key_event.code {
            KeyCode::Enter => {
                app.transition_to(AppState::Verifying)?;
                Ok(KeyActionResult::VerifyModel)
            }
            KeyCode::Tab => {
                tui_state.is_editing_endpoint = !tui_state.is_editing_endpoint;
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Char(c) => {
                tui_state.push_credential_char(c);
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Backspace => {
                tui_state.pop_credential_char();
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Esc => {
                app.transition_to(AppState::ModelInfo)?;
                Ok(KeyActionResult::Handled)
            }
            _ => Ok(KeyActionResult::Handled),
        }
    }

    fn handle_verification_failed(
        key_event: KeyEvent,
        app: &mut HadesApp,
        tui_state: &mut TuiState,
    ) -> Result<KeyActionResult, CoreError> {
        match key_event.code {
            KeyCode::Left | KeyCode::Up => {
                if tui_state.verification_action_index > 0 {
                    tui_state.verification_action_index -= 1;
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Right | KeyCode::Down => {
                if tui_state.verification_action_index < 2 {
                    tui_state.verification_action_index += 1;
                }
                Ok(KeyActionResult::Handled)
            }
            KeyCode::Enter => match tui_state.verification_action_index {
                0 => {
                    app.transition_to(AppState::Verifying)?;
                    Ok(KeyActionResult::VerifyModel)
                }
                1 => {
                    app.transition_to(AppState::CredentialInput)?;
                    Ok(KeyActionResult::Handled)
                }
                _ => {
                    app.transition_to(AppState::ModelSelect)?;
                    Ok(KeyActionResult::Handled)
                }
            },
            KeyCode::Esc => {
                app.transition_to(AppState::ModelSelect)?;
                Ok(KeyActionResult::Handled)
            }
            _ => Ok(KeyActionResult::Handled),
        }
    }
}
