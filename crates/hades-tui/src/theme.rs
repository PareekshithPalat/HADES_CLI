use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

/// The ratatui colors used by one named Hades terminal palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub primary: Color,
    pub accent: Color,
    pub alert: Color,
    pub info: Color,
    pub muted: Color,
    gradient_start: (u8, u8, u8),
    gradient_mid: (u8, u8, u8),
    gradient_end: (u8, u8, u8),
}

impl ThemePalette {
    const fn new(
        primary: Color,
        accent: Color,
        alert: Color,
        info: Color,
        muted: Color,
        gradient_start: (u8, u8, u8),
        gradient_mid: (u8, u8, u8),
        gradient_end: (u8, u8, u8),
    ) -> Self {
        Self { primary, accent, alert, info, muted, gradient_start, gradient_mid, gradient_end }
    }
}

thread_local! {
    static ACTIVE_PALETTE: std::cell::Cell<ThemePalette> = const {
        std::cell::Cell::new(HadesTheme::FIRE)
    };
}

/// Centralized color palette and visual styling for Hades CLI.
pub struct HadesTheme;

impl HadesTheme {
    pub const FIRE: ThemePalette = ThemePalette::new(
        Color::Rgb(255, 125, 0), Color::Rgb(255, 195, 0), Color::Rgb(255, 85, 0),
        Color::Rgb(0, 200, 255), Color::DarkGray,
        (255, 40, 0), (255, 110, 0), (255, 200, 0),
    );
    pub const MATRIX: ThemePalette = ThemePalette::new(
        Color::Rgb(0, 220, 100), Color::Rgb(170, 255, 0), Color::Rgb(0, 150, 70),
        Color::Rgb(80, 255, 180), Color::DarkGray,
        (0, 110, 45), (0, 190, 75), (170, 255, 0),
    );
    pub const CYAN: ThemePalette = ThemePalette::new(
        Color::Rgb(0, 220, 255), Color::Rgb(60, 140, 255), Color::Rgb(0, 150, 220),
        Color::Rgb(100, 240, 255), Color::DarkGray,
        (0, 120, 190), (0, 210, 255), (60, 140, 255),
    );
    pub const MONOCHROME: ThemePalette = ThemePalette::new(
        Color::White, Color::Gray, Color::LightRed, Color::LightCyan, Color::DarkGray,
        (150, 150, 150), (210, 210, 210), (255, 255, 255),
    );

    /// Resolves a configured palette name. Unknown names deliberately use fire.
    pub fn palette(name: &str) -> ThemePalette {
        match name.trim().to_ascii_lowercase().as_str() {
            "matrix" => Self::MATRIX,
            "cyan" => Self::CYAN,
            "monochrome" | "mono" => Self::MONOCHROME,
            "fire" => Self::FIRE,
            _ => Self::FIRE,
        }
    }

    /// Selects the palette used by the current TUI render thread.
    pub fn set_active(name: &str) {
        ACTIVE_PALETTE.with(|palette| palette.set(Self::palette(name)));
    }

    fn active() -> ThemePalette {
        ACTIVE_PALETTE.with(|palette| palette.get())
    }

    pub fn primary() -> Color { Self::active().primary }
    pub fn accent() -> Color { Self::active().accent }
    pub fn alert() -> Color { Self::active().alert }
    pub fn info() -> Color { Self::active().info }
    pub fn muted() -> Color { Self::active().muted }

    /// Recolors legacy fire-styled widgets after they have been rendered.
    /// This keeps all existing widget styling on the selected palette while the
    /// TUI is progressively migrated away from the original named constants.
    pub fn apply_to_buffer(buffer: &mut ratatui::buffer::Buffer) {
        let palette = Self::active();
        for cell in buffer.content_mut() {
            if cell.fg() == Self::RATATUI_ORANGE {
                cell.set_fg(palette.primary);
            } else if cell.fg() == Self::RATATUI_GOLD {
                cell.set_fg(palette.accent);
            } else if cell.fg() == Self::RATATUI_FIRE {
                cell.set_fg(palette.alert);
            } else if cell.fg() == Self::RATATUI_CYAN {
                cell.set_fg(palette.info);
            } else if cell.fg() == Self::RATATUI_DARK_GRAY {
                cell.set_fg(palette.muted);
            }

            if cell.bg() == Self::RATATUI_ORANGE {
                cell.set_bg(palette.primary);
            } else if cell.bg() == Self::RATATUI_GOLD {
                cell.set_bg(palette.accent);
            } else if cell.bg() == Self::RATATUI_FIRE {
                cell.set_bg(palette.alert);
            } else if cell.bg() == Self::RATATUI_CYAN {
                cell.set_bg(palette.info);
            } else if cell.bg() == Self::RATATUI_DARK_GRAY {
                cell.set_bg(palette.muted);
            }
        }
    }
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
            (Self::active().gradient_start, Self::active().gradient_mid, t / 0.5)
        } else {
            (Self::active().gradient_mid, Self::active().gradient_end, (t - 0.5) / 0.5)
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
                Style::default().fg(Self::accent()),
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
                .fg(Self::muted())
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
            Style::default().fg(Self::accent()),
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
                Style::default().fg(Self::muted()),
            )),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_palettes_have_distinct_primary_accents() {
        assert_eq!(HadesTheme::palette("fire"), HadesTheme::FIRE);
        assert_eq!(HadesTheme::palette("matrix").primary, Color::Rgb(0, 220, 100));
        assert_eq!(HadesTheme::palette("cyan").primary, Color::Rgb(0, 220, 255));
        assert_eq!(HadesTheme::palette("monochrome").primary, Color::White);
    }

    #[test]
    fn unknown_palette_falls_back_to_fire() {
        assert_eq!(HadesTheme::palette("not-a-theme"), HadesTheme::FIRE);
        assert_eq!(HadesTheme::palette(" MATRIX "), HadesTheme::MATRIX);
    }
}
