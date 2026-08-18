use ratatui::style::Color;

/// Centralized color palette and visual styling for Hades CLI.
pub struct HadesTheme;

impl HadesTheme {
    // ANSI Escape Color Codes
    pub const GOLD: &'static str = "\x1b[38;5;220m";
    pub const GOLD_BOLD: &'static str = "\x1b[1;38;5;220m";
    pub const CYAN_BOLD: &'static str = "\x1b[1;36m";
    pub const GREEN_BOLD: &'static str = "\x1b[1;32m";
    pub const WHITE_BOLD: &'static str = "\x1b[1;37m";
    pub const YELLOW_BOLD: &'static str = "\x1b[1;33m";
    pub const RED_BOLD: &'static str = "\x1b[1;31m";
    pub const DARK_GRAY: &'static str = "\x1b[90m";
    pub const RESET: &'static str = "\x1b[0m";

    // Ratatui Colors
    pub const RATATUI_GOLD: Color = Color::Rgb(255, 215, 0);
    pub const RATATUI_CYAN: Color = Color::Cyan;
    pub const RATATUI_GREEN: Color = Color::Green;
    pub const RATATUI_DARK_GRAY: Color = Color::DarkGray;

    // Unicode & ASCII Branding
    pub const TRIDENT: &'static str = "🔱";
    pub const TRIDENT_FALLBACK: &'static str = "[Ψ]";

    /// Returns the large styled Hades ASCII banner with golden trident.
    pub fn banner() -> &'static str {
        r#"
                   🔱
   █   █   ███   ████   █████   ████ 
   █   █  █   █  █   █  █       █    
   █████  █████  █   █  ████     ███ 
   █   █  █   █  █   █  █           █
   █   █  █   █  ████   █████   ████ 

          Universal AI Agent CLI
"#
    }

    /// Returns a compact banner for narrower terminals (< 60 columns).
    pub fn compact_banner() -> &'static str {
        r#"
                   🔱
             H A D E S
       Universal AI Agent CLI
"#
    }
}
