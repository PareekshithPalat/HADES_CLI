use ratatui::style::Color;

/// Centralized color palette and visual styling for Hades CLI.
pub struct HadesTheme;

impl HadesTheme {
    // ANSI Escape Color Codes (Fiery Orange / Fire Aesthetic)
    pub const GOLD: &'static str = "\x1b[38;5;220m";
    pub const GOLD_BOLD: &'static str = "\x1b[1;38;5;220m";
    pub const ORANGE_BOLD: &'static str = "\x1b[1;38;5;208m";
    pub const FIRE_ORANGE_BOLD: &'static str = "\x1b[1;38;5;202m";
    pub const GREEN_BOLD: &'static str = "\x1b[1;32m";
    pub const WHITE_BOLD: &'static str = "\x1b[1;37m";
    pub const YELLOW_BOLD: &'static str = "\x1b[1;33m";
    pub const RED_BOLD: &'static str = "\x1b[1;31m";
    pub const DARK_GRAY: &'static str = "\x1b[90m";
    pub const RESET: &'static str = "\x1b[0m";

    // Ratatui Colors
    pub const RATATUI_ORANGE: Color = Color::Rgb(255, 125, 0);
    pub const RATATUI_FIRE: Color = Color::Rgb(255, 85, 0);
    pub const RATATUI_GOLD: Color = Color::Rgb(255, 195, 0);
    pub const RATATUI_GREEN: Color = Color::Green;
    pub const RATATUI_DARK_GRAY: Color = Color::DarkGray;
    pub const RATATUI_CYAN: Color = Color::Rgb(0, 200, 255);

    // Unicode & ASCII Branding
    pub const TRIDENT: &'static str = "🜲";
    pub const TRIDENT_ALT: &'static str = "🔱";
    pub const TRIDENT_FALLBACK: &'static str = "[Ψ]";

    /// Returns the large styled Hades ASCII banner with fiery trident.
    pub fn banner() -> &'static str {
        r#"
                   🜲
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
                   🜲
             H A D E S
       Universal AI Agent CLI
"#
    }
}
