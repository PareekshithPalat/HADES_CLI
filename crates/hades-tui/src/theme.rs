use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

/// Centralized color palette and visual styling for Hades CLI.
pub struct HadesTheme;

impl HadesTheme {
    // ANSI Escape Color Codes (Fiery Orange / Fire Aesthetic) — kept for
    // plain terminal writes outside of ratatui (logs, early boot text, etc).
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

    // Gradient endpoints for the wordmark: deep fire-red -> orange -> gold
    const GRAD_START: (u8, u8, u8) = (255, 40, 0); // fire red
    const GRAD_MID: (u8, u8, u8) = (255, 110, 0); // orange
    const GRAD_END: (u8, u8, u8) = (255, 200, 0); // gold

    // Unicode & ASCII Branding
    pub const TRIDENT: &'static str = "🔱";
    pub const TRIDENT_FALLBACK: &'static str = "[Ψ]";

    /// Raw block-letter "HADES" wordmark (no trident, no color).
    /// Each line is the same visual width so the gradient lines up cleanly.
    const WORDMARK_LINES: [&'static str; 6] = [
        r"██╗  ██╗ █████╗ ██████╗ ███████╗███████╗",
        r"██║  ██║██╔══██╗██╔══██╗██╔════╝██╔════╝",
        r"███████║███████║██║  ██║█████╗  ███████╗",
        r"██╔══██║██╔══██║██║  ██║██╔══╝  ╚════██║",
        r"██║  ██║██║  ██║██████╔╝███████╗███████║",
        r"╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝ ╚══════╝╚══════╝",
    ];

    /// Three-stop linear interpolation: red -> orange -> gold, t in [0,1].
    fn gradient_color(t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let (a, b, frac) = if t < 0.5 {
            (Self::GRAD_START, Self::GRAD_MID, t / 0.5)
        } else {
            (Self::GRAD_MID, Self::GRAD_END, (t - 0.5) / 0.5)
        };
        let lerp = |x: u8, y: u8| -> u8 { (x as f32 + (y as f32 - x as f32) * frac).round() as u8 };
        Color::Rgb(lerp(a.0, b.0), lerp(a.1, b.1), lerp(a.2, b.2))
    }

    /// Builds the full-size banner as styled ratatui `Text`, ready to hand
    /// straight to a `Paragraph`. Trident sits to the left, vertically
    /// centered against the 6-line wordmark; wordmark is gradient-colored
    /// left-to-right (fire red -> orange -> gold), like Gemini/Codex CLI banners.
    pub fn banner() -> Text<'static> {
        let width = Self::WORDMARK_LINES[0].chars().count().max(1);
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(6);

        for (row, raw) in Self::WORDMARK_LINES.iter().enumerate() {
            let mut spans: Vec<Span<'static>> = Vec::new();

            // Left gutter: trident on the middle row(s), blank elsewhere,
            // so it reads as a logo mark beside the wordmark, not above it.
            let gutter = if row == 2 {
                format!(" {} ", Self::TRIDENT)
            } else {
                "    ".to_string()
            };
            spans.push(Span::styled(
                gutter,
                Style::default().fg(Self::RATATUI_GOLD),
            ));

            for (col, ch) in raw.chars().enumerate() {
                let t = col as f32 / width.saturating_sub(1).max(1) as f32;
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default()
                        .fg(Self::gradient_color(t))
                        .add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(spans));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "     Universal AI Agent CLI",
            Style::default()
                .fg(Self::RATATUI_DARK_GRAY)
                .add_modifier(Modifier::ITALIC),
        )));

        Text::from(lines)
    }

    /// Compact banner for narrow terminals (< 60 columns): trident + name
    /// on one line, gradient applied to "HADES" only.
    pub fn compact_banner() -> Text<'static> {
        let name = "HADES";
        let mut spans = vec![Span::styled(
            format!("{} ", Self::TRIDENT),
            Style::default().fg(Self::RATATUI_GOLD),
        )];

        let len = name.chars().count().max(1);
        for (i, ch) in name.chars().enumerate() {
            let t = i as f32 / (len - 1).max(1) as f32;
            spans.push(Span::styled(
                ch.to_string(),
                Style::default()
                    .fg(Self::gradient_color(t))
                    .add_modifier(Modifier::BOLD),
            ));
        }

        Text::from(vec![
            Line::from(spans),
            Line::from(Span::styled(
                "Universal AI Agent CLI",
                Style::default().fg(Self::RATATUI_DARK_GRAY),
            )),
        ])
    }
}
