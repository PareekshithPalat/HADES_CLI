use crate::theme::HadesTheme;
use std::io::{stdout, Write};

/// Manages rendering, updating, and clearing of the 2-line inline prompt and status bar.
pub struct PromptManager;

impl PromptManager {
    /// Renders the 2-line prompt and status bar at the bottom of the conversation.
    pub fn render_prompt(
        prompt_input: &str,
        cursor_pos: usize,
        active_model: &str,
        default_mode: &str,
    ) {
        let mut out = stdout();

        // 1. Format status line
        let model_display = if active_model == "Not configured" {
            format!("{}No Model{}", HadesTheme::YELLOW_BOLD, HadesTheme::RESET)
        } else {
            format!(
                "{}{}{}",
                HadesTheme::ORANGE_BOLD,
                active_model,
                HadesTheme::RESET
            )
        };
        let status_line = format!(
            "  {} · {}{} · / for commands · Ctrl+C to exit",
            model_display,
            HadesTheme::DARK_GRAY,
            default_mode
        );

        // 2. Print Prompt line (Line 1)
        print!(
            "\r\x1b[2K  {}›{} {}▌",
            HadesTheme::ORANGE_BOLD,
            HadesTheme::RESET,
            prompt_input
        );

        // 3. Print Status line (Line 2)
        print!("\n\r\x1b[2K\x1b[90m{}\x1b[0m", status_line);

        // 4. Move cursor back to prompt line at exact cursor offset
        let col = 5 + cursor_pos;
        print!("\x1b[1A\r\x1b[{}C", col);

        let _ = out.flush();
    }

    /// Clears both prompt and status lines from the terminal before emitting conversation output.
    pub fn clear_prompt() {
        let mut out = stdout();
        // Move to Line 2 and clear, then move to Line 1 and clear
        print!("\n\r\x1b[2K\x1b[1A\r\x1b[2K");
        let _ = out.flush();
    }
}
