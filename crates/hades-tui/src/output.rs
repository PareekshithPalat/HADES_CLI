use crate::theme::HadesTheme;
use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
};
use hades_core::CommandOutput;
use std::io::{stdout, Write};

/// Handles terminal output formatting, startup presentation, and turn streaming
/// directly onto the primary screen so that all conversation enters native scrollback.
pub struct ConversationPrinter;

impl ConversationPrinter {
    /// Clears the active screen viewport and renders the large Hades startup identity.
    pub fn print_startup(active_model: &str) {
        let mut out = stdout();
        // Clear the visible viewport to eliminate previous cargo/build command artifacts
        let _ = execute!(out, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0));

        let width = terminal::size().map(|(w, _)| w).unwrap_or(80) as usize;
        let sep_len = width.clamp(40, 72);
        let separator = "─".repeat(sep_len);

        println!("\x1b[90m{}\x1b[0m", separator);

        if width >= 60 {
            println!(
                "                    {}{}{}",
                HadesTheme::GOLD_BOLD,
                HadesTheme::TRIDENT,
                HadesTheme::RESET
            );
            println!(
                "    {}█   █   ███   ████   █████   ████ {}",
                HadesTheme::FIRE_ORANGE_BOLD,
                HadesTheme::RESET
            );
            println!(
                "    {}█   █  █   █  █   █  █       █    {}",
                HadesTheme::FIRE_ORANGE_BOLD,
                HadesTheme::RESET
            );
            println!(
                "    {}█████  █████  █   █  ████     ███ {}",
                HadesTheme::FIRE_ORANGE_BOLD,
                HadesTheme::RESET
            );
            println!(
                "    {}█   █  █   █  █   █  █           █{}",
                HadesTheme::FIRE_ORANGE_BOLD,
                HadesTheme::RESET
            );
            println!(
                "    {}█   █  █   █  ████   █████   ████ {}",
                HadesTheme::FIRE_ORANGE_BOLD,
                HadesTheme::RESET
            );
            println!();
            println!(
                "           {}Universal AI Agent CLI{}",
                HadesTheme::DARK_GRAY,
                HadesTheme::RESET
            );
        } else {
            println!(
                "                    {}{}{}",
                HadesTheme::GOLD_BOLD,
                HadesTheme::TRIDENT,
                HadesTheme::RESET
            );
            println!(
                "              {}H A D E S{}",
                HadesTheme::FIRE_ORANGE_BOLD,
                HadesTheme::RESET
            );
            println!(
                "        {}Universal AI Agent CLI{}",
                HadesTheme::DARK_GRAY,
                HadesTheme::RESET
            );
        }

        println!("\x1b[90m{}\x1b[0m", separator);
        println!();
        println!("  \x1b[1;37mWelcome to Hades.\x1b[0m");
        if active_model == "Not configured" {
            println!("  \x1b[90mActive model:\x1b[0m \x1b[33mNot configured\x1b[0m (\x1b[1;38;5;208m/model\x1b[0m to configure)");
        } else {
            println!(
                "  \x1b[90mActive model:\x1b[0m \x1b[1;38;5;208m{}\x1b[0m",
                active_model
            );
        }
        println!();
        println!("  \x1b[90mType a prompt and press \x1b[1;37mEnter\x1b[0m\x1b[90m to chat, or \x1b[1;38;5;208m/\x1b[0m\x1b[90m for commands (/help, /model, /exit).\x1b[0m");
        println!();
        let _ = out.flush();
    }

    /// Prints a user message turn to stdout.
    pub fn print_user_prompt(prompt: &str) {
        println!("  \x1b[1;36mYou\x1b[0m");
        for (i, line) in prompt.lines().enumerate() {
            let prefix = if i == 0 { "  └─ " } else { "     " };
            println!("\x1b[90m{}\x1b[0m{}", prefix, line);
        }
        println!();
        let _ = stdout().flush();
    }

    /// Prints the opening of the Hades turn with an initial activity indicator.
    pub fn start_hades_turn(activity: &str, spinner: &str) {
        println!("  \x1b[1;32mHades\x1b[0m");
        print!("  \x1b[90m└─ \x1b[33m{} {}\x1b[0m", spinner, activity);
        let _ = stdout().flush();
    }

    /// Updates the active transient activity line in place.
    pub fn update_activity(activity: &str, spinner: &str) {
        print!(
            "\r\x1b[2K  \x1b[90m└─ \x1b[33m{} {}\x1b[0m",
            spinner, activity
        );
        let _ = stdout().flush();
    }

    /// Begins streaming the assistant response, clearing the activity line.
    pub fn start_streaming_chunk(first_chunk: &str) {
        print!("\r\x1b[2K  \x1b[90m└─ \x1b[0m{}", first_chunk);
        let _ = stdout().flush();
    }

    /// Appends an incremental delta text chunk during response streaming.
    pub fn append_streaming_chunk(chunk: &str) {
        print!("{}", chunk);
        let _ = stdout().flush();
    }

    /// Finalizes the assistant response turn with trailing newlines.
    pub fn finalize_hades_turn() {
        println!();
        println!();
        let _ = stdout().flush();
    }

    /// Prints a full non-streamed assistant response turn.
    pub fn print_hades_full_response(response: &str) {
        print!("\r\x1b[2K");
        for (i, line) in response.lines().enumerate() {
            let prefix = if i == 0 { "  └─ " } else { "     " };
            println!("\x1b[90m{}\x1b[0m{}", prefix, line);
        }
        println!();
        let _ = stdout().flush();
    }

    /// Prints an error message underneath the active turn.
    pub fn print_turn_error(error: &str) {
        print!("\r\x1b[2K");
        println!(
            "  \x1b[90m└─ \x1b[1;31mError: \x1b[0m\x1b[31m{}\x1b[0m",
            error
        );
        println!();
        let _ = stdout().flush();
    }

    /// Prints command execution output to stdout.
    pub fn print_command_output(output: &CommandOutput) {
        let text = output.to_string();
        for line in text.lines() {
            println!("  {}", line);
        }
        println!();
        let _ = stdout().flush();
    }

    /// Prints a global error message banner.
    pub fn print_error(error: &str) {
        println!("  \x1b[1;31mError: \x1b[0m\x1b[31m{}\x1b[0m", error);
        println!();
        let _ = stdout().flush();
    }
}
