use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame,
};

use crate::state::TuiState;
use hades_core::{AppState, HadesApp};

/// Main draw entry point dispatching view rendering with strict mathematical geometry.
pub fn render(frame: &mut Frame, app: &HadesApp, state: &mut TuiState) {
    let size = frame.area();
    if size.width < 10 || size.height < 5 {
        // Guard against degenerate terminal sizes during extreme resize
        return;
    }

    // Dynamic Geometry Calculation
    let header_height: u16 = if size.height >= 28 && size.width >= 60 {
        8
    } else if size.height >= 18 {
        3
    } else if size.height >= 10 {
        1
    } else {
        0
    };

    let reserved_bottom_height: u16 = 3; // 1 divider + 1 prompt + 1 status
    let conversation_height = size
        .height
        .saturating_sub(header_height + reserved_bottom_height);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Length(conversation_height),
            Constraint::Length(2), // 1 divider + 1 prompt
            Constraint::Length(1), // 1 status
        ])
        .split(size);

    if header_height > 0 {
        render_header(frame, chunks[0]);
    }
    render_conversation(frame, app, state, chunks[1]);
    render_prompt_area(frame, app, state, chunks[2]);
    render_status_bar(frame, app, state, chunks[3]);

    // Modal Overlays (Minimal, clean floating dialogs)
    match app.state() {
        AppState::CommandPalette => render_command_palette(frame, app, state, size),
        AppState::ProviderSelect => render_provider_select(frame, state, size),
        AppState::ModelSelect => render_model_select(frame, state, size),
        AppState::ModelInfo => render_model_info(frame, state, size),
        AppState::CredentialInput => render_credential_input(frame, state, size),
        AppState::Verifying => render_verifying(frame, state, size),
        AppState::VerificationFailed => render_verification_failed(frame, state, size),
        _ => {}
    }
}

/// Renders the responsive Hades branding header scaled dynamically to terminal dimensions.
fn render_header(frame: &mut Frame, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    if area.height >= 8 && area.width >= 60 {
        let lines = vec![
            Line::from(vec![
                Span::styled("                    ", Style::default()),
                Span::styled(
                    "🔱",
                    Style::default()
                        .fg(Color::Rgb(255, 215, 0))
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                "    █   █   ███   ████   █████   ████ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "    █   █  █   █  █   █  █       █    ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "    █████  █████  █   █  ████     ███ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "    █   █  █   █  █   █  █           █",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "    █   █  █   █  ████   █████   ████ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "           Universal AI Agent CLI",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "─".repeat(area.width as usize),
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    } else if area.height >= 3 {
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    " 🔱 ",
                    Style::default()
                        .fg(Color::Rgb(255, 215, 0))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "H A D E S",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                "    Universal AI Agent CLI",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "─".repeat(area.width as usize),
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    } else {
        let line = Line::from(vec![
            Span::styled(
                " 🔱 HADES ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "· Universal AI Agent CLI",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(Paragraph::new(vec![line]), area);
    }
}

/// Helper formatting and word-wrapping turn texts with tree-branch indentations.
pub fn wrap_turn_text(
    text: &str,
    width: usize,
    first_prefix: &str,
    cont_prefix: &str,
    style: Style,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let prefix_width = 5; // "  └─ " or "     "
    let content_width = width.saturating_sub(prefix_width).max(10);

    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        let words = raw_line.split(' ');
        let mut current_line = String::new();
        let mut is_first = lines.is_empty();

        for word in words {
            if word.is_empty() {
                continue;
            }
            if current_line.is_empty() {
                current_line.push_str(word);
            } else if current_line.len() + 1 + word.len() <= content_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                let prefix = if is_first {
                    first_prefix.to_string()
                } else {
                    cont_prefix.to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                    Span::styled(current_line, style),
                ]));
                is_first = false;
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            let prefix = if is_first {
                first_prefix.to_string()
            } else {
                cont_prefix.to_string()
            };
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                Span::styled(current_line, style),
            ]));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                first_prefix.to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(text.to_string(), style),
        ]));
    }

    lines
}

/// Renders the strictly bounded conversation stream with application-owned scrollbar and auto-follow.
fn render_conversation(frame: &mut Frame, app: &HadesApp, state: &mut TuiState, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // Split horizontally into conversation text area and 1-column scrollbar track
    let conv_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let content_area = conv_chunks[0];
    let scrollbar_area = conv_chunks[1];

    let mut lines: Vec<Line<'static>> = Vec::new();
    let width = content_area.width as usize;

    // 1. Welcome / Initial Guide if conversation is empty
    if state.turns.is_empty() && state.active_output.is_none() && state.error_message.is_none() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "  Welcome to Hades.",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![
            Span::styled("  Active model: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.active_model_display(),
                if app.active_model_display() == "Not configured" {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                },
            ),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                "  Type a prompt and press ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to chat, or ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "/",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " for commands (/help, /status, /model, /exit).",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(""));
    }

    // 2. Global Error Banner, if any
    if let Some(ref err) = state.error_message {
        lines.push(Line::from(vec![
            Span::styled(
                "  Error: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(err.clone(), Style::default().fg(Color::Red)),
        ]));
        lines.push(Line::from(""));
    }

    // 3. Active Command Output, if any
    if let Some(ref output) = state.active_output {
        let out_str = output.to_string();
        for l in out_str.lines() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(l.to_string(), Style::default().fg(Color::White)),
            ]));
        }
        lines.push(Line::from(""));
    }

    // 4. Chronological Conversation Turns with Dynamic Width-Aware Wrapping
    for turn in &state.turns {
        // User turn header
        lines.push(Line::from(vec![Span::styled(
            "  You",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]));

        // User turn content (indented with tree branch)
        let user_lines = wrap_turn_text(
            &turn.user_prompt,
            width,
            "  └─ ",
            "     ",
            Style::default().fg(Color::White),
        );
        lines.extend(user_lines);
        lines.push(Line::from(""));

        // Hades turn response / activity / error
        lines.push(Line::from(vec![Span::styled(
            "  Hades",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]));

        if let Some(ref activity) = turn.activity_text {
            lines.push(Line::from(vec![
                Span::styled("  └─ ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} ", state.spinner_char()),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(activity.clone(), Style::default().fg(Color::Yellow)),
            ]));
        } else if let Some(ref response) = turn.assistant_response {
            let resp_lines = wrap_turn_text(
                response,
                width,
                "  └─ ",
                "     ",
                Style::default().fg(Color::White),
            );
            lines.extend(resp_lines);
        } else if let Some(ref err) = turn.error_text {
            lines.push(Line::from(vec![
                Span::styled(
                    "  └─ Error: ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(err.clone(), Style::default().fg(Color::Red)),
            ]));
        }
        lines.push(Line::from(""));
    }

    // Synchronize geometry and scroll offset safely
    let total_lines = lines.len();
    let viewport_height = content_area.height as usize;
    state.update_geometry(total_lines, viewport_height);

    let max_scroll = state.max_scroll_offset();
    let scroll_y = state.scroll_offset;

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y as u16, 0));

    frame.render_widget(paragraph, content_area);

    // 5. Render Application-Owned Scrollbar Widget
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .thumb_symbol("█")
        .track_symbol(Some("░"))
        .begin_symbol(Some("▲"))
        .end_symbol(Some("▼"))
        .thumb_style(Style::default().fg(Color::Cyan))
        .track_style(Style::default().fg(Color::DarkGray));

    let mut scrollbar_state = ScrollbarState::new(max_scroll).position(scroll_y);
    frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);

    // 6. Subtle New-Content Indicator when scrolled away from the bottom
    if state.has_new_content_below {
        let indicator_line = Line::from(vec![
            Span::styled(
                "  ↓ New content below (press ",
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                "End",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to follow)  ", Style::default().fg(Color::Yellow)),
        ]);
        let indicator_width = 38u16.min(content_area.width);
        let indicator_area = Rect {
            x: content_area.x + content_area.width.saturating_sub(indicator_width + 1),
            y: content_area.y + content_area.height.saturating_sub(1),
            width: indicator_width,
            height: 1,
        };
        frame.render_widget(Clear, indicator_area);
        frame.render_widget(
            Paragraph::new(indicator_line).style(Style::default().bg(Color::Rgb(40, 40, 40))),
            indicator_area,
        );
    }
}

/// Helper estimating wrapped line count given line texts and terminal width.
pub fn estimate_wrapped_line_count(lines: &[Line], width: u16) -> usize {
    if width == 0 {
        return lines.len();
    }
    let w = width as usize;
    let mut total = 0;
    for line in lines {
        let line_len: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        if line_len == 0 {
            total += 1;
        } else {
            total += line_len.div_ceil(w);
        }
    }
    total
}

/// Renders the fixed prompt input area directly above the status line.
fn render_prompt_area(frame: &mut Frame, app: &HadesApp, state: &TuiState, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // 1. Subtle horizontal divider
    let separator = "─".repeat(area.width as usize);
    let sep_para = Paragraph::new(Line::from(vec![Span::styled(
        separator,
        Style::default().fg(Color::DarkGray),
    )]));
    frame.render_widget(sep_para, chunks[0]);

    // 2. Terminal-native prompt line
    let prompt_line = if app.state() == AppState::AiThinking || app.state() == AppState::AiStreaming
    {
        Line::from(vec![
            Span::styled(
                " › ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&state.prompt_input, Style::default().fg(Color::White)),
            Span::styled("▌", Style::default().fg(Color::DarkGray)),
            Span::styled(
                " (generating response...)",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                " › ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&state.prompt_input, Style::default().fg(Color::White)),
            Span::styled("▌", Style::default().fg(Color::Cyan)),
        ])
    };

    let prompt_para = Paragraph::new(prompt_line);
    frame.render_widget(prompt_para, chunks[1]);
}

/// Renders the compact status line pinned to the bottom row of the terminal.
fn render_status_bar(frame: &mut Frame, app: &HadesApp, state: &TuiState, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let model_display = app.active_model_display();
    let mode_display = &app.config().general.default_mode;

    let status_line = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            if model_display == "Not configured" {
                "No Model"
            } else {
                &model_display
            },
            if model_display == "Not configured" {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            },
        ),
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            mode_display,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        if let Some(ref usage) = state.current_usage {
            Span::styled(
                format!("{} tokens", usage.total_tokens.unwrap_or_default()),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::styled("/ for commands", Style::default().fg(Color::DarkGray))
        },
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        Span::styled("Ctrl+C to exit", Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(status_line);
    frame.render_widget(paragraph, area);
}

/// Helper computing a centered popup rectangle given percentage dimensions.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Renders the floating Command Palette overlay.
fn render_command_palette(frame: &mut Frame, app: &HadesApp, state: &TuiState, area: Rect) {
    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup_area);

    let commands = app.commands().list();
    let items: Vec<ListItem> = commands
        .iter()
        .enumerate()
        .map(|(idx, cmd)| {
            let is_selected = idx == state.selected_palette_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_selected { " ▸ " } else { "   " };
            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(
                    format!("{:<12}", cmd.name),
                    style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {}", cmd.description), style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let block = Block::default()
        .title(" Commands ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, popup_area);
}

/// Renders the floating Provider Selection modal.
fn render_provider_select(frame: &mut Frame, state: &TuiState, area: Rect) {
    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = state
        .providers
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            let is_selected = idx == state.selected_provider_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_selected { " ▸ " } else { "   " };
            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(
                    format!("{:<20}", p.name),
                    style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {}", p.description), style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let block = Block::default()
        .title(" Select AI Provider ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, popup_area);
}

/// Renders the floating Model Selection modal.
fn render_model_select(frame: &mut Frame, state: &TuiState, area: Rect) {
    let popup_area = centered_rect(70, 60, area);
    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = state
        .models
        .iter()
        .enumerate()
        .map(|(idx, m)| {
            let is_selected = idx == state.selected_model_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_selected { " ▸ " } else { "   " };
            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(
                    format!("{:<28}", m.display_name),
                    style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" Context: {:<6}", m.context_window_display()),
                    style,
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let block = Block::default()
        .title(" Select Model ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, popup_area);
}

/// Renders the floating Model Information Details card.
fn render_model_info(frame: &mut Frame, state: &TuiState, area: Rect) {
    let popup_area = centered_rect(65, 55, area);
    frame.render_widget(Clear, popup_area);

    let model = match state.selected_model {
        Some(ref m) => m,
        None => return,
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Model: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &model.display_name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({})", model.id),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Provider: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&model.provider_id, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Capabilities:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
    ];

    for (cap, cap_state) in model.capabilities.iter() {
        let (symbol, color) = match cap_state {
            hades_provider::CapabilityState::Supported => ("✓", Color::Green),
            hades_provider::CapabilityState::Unsupported => ("✗", Color::DarkGray),
            hades_provider::CapabilityState::Unknown => ("?", Color::DarkGray),
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("    {} ", symbol),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(cap.to_string(), Style::default().fg(Color::White)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Context Window: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            model.context_window_display(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  [ Enter = Proceed ]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("    [ Esc = Back ]", Style::default().fg(Color::DarkGray)),
    ]));

    let block = Block::default()
        .title(" Model Details ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup_area);
}

/// Renders the floating Credential Input modal.
fn render_credential_input(frame: &mut Frame, state: &TuiState, area: Rect) {
    let popup_area = centered_rect(65, 45, area);
    frame.render_widget(Clear, popup_area);

    let masked_key: String = "*".repeat(state.credential_input.len());

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Enter authentication API key (credentials are stored securely locally):",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  API Key: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if masked_key.is_empty() {
                    "(paste or type key)▌".to_string()
                } else {
                    format!("{masked_key}▌")
                },
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Endpoint override: ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                if state.custom_endpoint_input.is_empty() {
                    "(default)"
                } else {
                    &state.custom_endpoint_input
                },
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Enter = Connect   Tab = Toggle field   Esc = Back",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title(" Credential Setup ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup_area);
}

/// Renders the floating Connection Verification card.
fn render_verifying(frame: &mut Frame, state: &TuiState, area: Rect) {
    let popup_area = centered_rect(50, 30, area);
    frame.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("  {} ", state.spinner_char()),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Verifying provider access & model...",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Please wait...",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title(" Connecting ")
        .title_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, popup_area);
}

/// Renders the floating Verification Failed diagnostics dialog.
fn render_verification_failed(frame: &mut Frame, state: &TuiState, area: Rect) {
    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup_area);

    let error_msg = state
        .verification_error
        .as_deref()
        .unwrap_or("Authentication failed.");

    let actions = ["Retry", "Change Credential", "Back to Models"];
    let action_spans: Vec<Span> = actions
        .iter()
        .enumerate()
        .flat_map(|(idx, act)| {
            let is_sel = idx == state.verification_action_index;
            let style = if is_sel {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            vec![
                Span::styled(
                    if is_sel {
                        format!(" ▸ [{act}] ")
                    } else {
                        format!("   [{act}] ")
                    },
                    style,
                ),
                Span::raw("  "),
            ]
        })
        .collect();

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Error: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(error_msg, Style::default().fg(Color::Red)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Please check API key validity and network connection.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(action_spans),
        Line::from(""),
        Line::from(Span::styled(
            "  ↑ ↓ Navigate   Enter Select   Esc Back",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title(" Verification Failed ")
        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup_area);
}
