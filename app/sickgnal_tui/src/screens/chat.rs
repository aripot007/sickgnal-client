use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::App;
use sickgnal_core::chat::storage::MessageStatus;
use uuid::Uuid;

/// Compute a horizontally centred sub-rect that is 60 % of the terminal
/// width but never narrower than `min` columns.
fn centered_rect(area: Rect, min: u16) -> Rect {
    let target = (area.width as u32 * 60 / 100) as u16;
    let w = target.max(min).min(area.width);
    let pad = (area.width.saturating_sub(w)) / 2;
    Rect::new(area.x + pad, area.y, w, area.height)
}

/// Word-wrap `text` into lines of at most `max_width` characters.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            if word.len() > max_width {
                // Force-break long words
                let mut remaining = word;
                while remaining.len() > max_width {
                    lines.push(remaining[..max_width].to_string());
                    remaining = &remaining[max_width..];
                }
                current = remaining.to_string();
            } else {
                current = word.to_string();
            }
        } else if current.len() + 1 + word.len() <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            if word.len() > max_width {
                let mut remaining = word;
                while remaining.len() > max_width {
                    lines.push(remaining[..max_width].to_string());
                    remaining = &remaining[max_width..];
                }
                current = remaining.to_string();
            } else {
                current = word.to_string();
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Look up sender display name from conversation peers.
fn sender_name(peers: &[sickgnal_core::e2e::peer::Peer], sender_id: Uuid) -> String {
    peers
        .iter()
        .find(|p| p.id == sender_id)
        .map(|p| p.name())
        .unwrap_or_else(|| sender_id.to_string()[..8].to_string())
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Check for active typing indicator
    let typing_text = app
        .current_conversation
        .and_then(|cid| app.typing_indicators.get(&cid))
        .map(|(name, _)| format!("{} is typing...", name));

    let has_reply_bar = app.reply_to_message.is_some()
        && app.editing_message_id.is_none()
        && app.confirm_delete.is_none()
        && app.selected_message.is_none();

    // Determine input area height based on wrapping
    let input_inner_width = area.width.saturating_sub(4) as usize; // borders + padding
    let prefix_len = if app.confirm_delete.is_some() {
        21
    } else if app.editing_message_id.is_some() {
        12
    } else if app.selected_message.is_some() {
        2
    } else if app.reply_to_message.is_some() {
        10
    } else {
        2
    };
    let avail_input_width = input_inner_width.saturating_sub(prefix_len + 1); // +1 for cursor
    let input_lines_needed = if avail_input_width > 0 {
        ((app.message_input.len() + avail_input_width) / avail_input_width).min(4)
    } else {
        1
    };
    let input_height = (input_lines_needed as u16 + 2).max(3); // +2 for border

    let chunks = Layout::vertical([
        Constraint::Length(3), // Header with conversation name
        Constraint::Min(1),    // Messages area
        Constraint::Length(if typing_text.is_some() { 1 } else { 0 }), // Typing indicator
        Constraint::Length(if has_reply_bar { 1 } else { 0 }), // Reply bar
        Constraint::Length(input_height), // Input area
    ])
    .split(area);

    // Determine if this is a group conversation
    let entry = app
        .current_conversation
        .and_then(|cid| app.conversations.iter().find(|e| e.conversation.id == cid));

    let is_group = entry
        .map(|e| e.conversation.peers.len() > 1)
        .unwrap_or(false);

    let peers = entry
        .map(|e| e.conversation.peers.as_slice())
        .unwrap_or(&[]);

    // Find current conversation and peer fingerprint
    let (conv_name, fingerprint) = entry
        .map(|e| {
            let fp = e
                .conversation
                .peers
                .first()
                .map(|p| p.format_fingerprint())
                .unwrap_or_default();
            (e.conversation.title.clone(), fp)
        })
        .unwrap_or_else(|| ("Chat".into(), String::new()));

    // Format fingerprint for display
    let fp_display = if fingerprint.is_empty() || fingerprint == "no fingerprint" || is_group {
        String::new()
    } else {
        format!("  [{}]", fingerprint)
    };

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled("  < ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            &conv_name,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  ({})", app.username),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(&fp_display, Style::default().fg(Color::Yellow)),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(header, chunks[0]);

    // ── Messages ──────────────────────────────────────────────────────
    let messages_area = centered_rect(chunks[1], 40);
    let chat_width = messages_area.width as usize;
    let visible_height = messages_area.height;

    // Update messages_area_height for key handler to use
    app.messages_area_height = visible_height;

    if app.messages.is_empty() {
        app.total_visual_lines = 0;
        app.message_line_offsets.clear();

        let empty = Paragraph::new(Line::from(vec![Span::styled(
            "No messages yet. Type something below.",
            Style::default().fg(Color::DarkGray),
        )]))
        .alignment(Alignment::Center);
        f.render_widget(empty, messages_area);
    } else {
        let my_id = app.my_user_id;

        // Build all visual lines and track per-message line offsets
        let mut all_lines: Vec<Line> = Vec::new();
        let mut message_line_offsets: Vec<u16> = Vec::new();

        for (idx, msg) in app.messages.iter().enumerate() {
            // Record the starting visual line for this message
            message_line_offsets.push(all_lines.len() as u16);

            let is_mine = my_id.is_some_and(|id| id == msg.sender_id);
            let is_selected = app.selected_message == Some(idx);
            let time = msg.issued_at.format("%H:%M").to_string();

            let status_str = if is_mine {
                match msg.status {
                    MessageStatus::Sending => " ...",
                    MessageStatus::Sent => " v",
                    MessageStatus::Delivered => " vv",
                    MessageStatus::Read => " vv",
                    MessageStatus::Failed => " !",
                }
            } else {
                ""
            };

            let status_color = match msg.status {
                MessageStatus::Read => Color::Cyan,
                MessageStatus::Failed => Color::Red,
                _ => Color::DarkGray,
            };

            // Selection marker
            let marker = if is_selected { ">" } else { " " };
            let marker_style = Style::default().fg(Color::Yellow);

            let bg = if is_selected {
                Some(Color::DarkGray)
            } else {
                None
            };

            let apply_bg = |mut style: Style| -> Style {
                if let Some(bg) = bg {
                    style = style.bg(bg);
                }
                style
            };

            // ── Sender name for group conversations ──
            if is_group && !is_mine {
                let name = sender_name(peers, msg.sender_id);
                all_lines.push(Line::from(vec![
                    Span::styled(marker, marker_style),
                    Span::styled(
                        name,
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            // ── Reply quote (if this message replies to another) ──
            if let Some(reply_id) = msg.reply_to_id {
                let reply_preview = app
                    .messages
                    .iter()
                    .find(|m| m.id == reply_id)
                    .map(|m| {
                        if m.content.len() > 40 {
                            format!("{}...", &m.content[..40])
                        } else {
                            m.content.clone()
                        }
                    })
                    .unwrap_or_else(|| "...".into());

                if is_mine {
                    let quote_text = format!("│ {reply_preview}");
                    let quote_len = marker.len() + quote_text.len();
                    let padding = if chat_width > quote_len {
                        " ".repeat(chat_width - quote_len)
                    } else {
                        String::new()
                    };
                    all_lines.push(Line::from(vec![
                        Span::styled(marker, marker_style),
                        Span::styled(padding, Style::default()),
                        Span::styled(
                            quote_text,
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]));
                } else {
                    all_lines.push(Line::from(vec![
                        Span::styled(marker, marker_style),
                        Span::styled(
                            format!("│ {reply_preview}"),
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]));
                }
            }

            // ── Message content with wrapping ──
            let time_suffix = format!(" {}", time);
            let status_suffix = status_str;
            let suffix_len = time_suffix.len() + status_suffix.len();

            // Available width for message text (minus marker, suffix)
            let text_max_width = chat_width
                .saturating_sub(marker.len())
                .saturating_sub(suffix_len);

            let wrapped = wrap_text(&msg.content, text_max_width.max(10));

            for (line_idx, line_text) in wrapped.iter().enumerate() {
                let is_last_line = line_idx == wrapped.len() - 1;

                if is_mine {
                    if is_last_line {
                        let content = format!("{}{} ", line_text, status_suffix);
                        let visible_len = marker.len() + content.len() + time_suffix.len();
                        let padding = if chat_width > visible_len {
                            " ".repeat(chat_width - visible_len)
                        } else {
                            String::new()
                        };
                        all_lines.push(Line::from(vec![
                            Span::styled(marker, marker_style),
                            Span::styled(padding, apply_bg(Style::default())),
                            Span::styled(
                                line_text.clone(),
                                apply_bg(Style::default().fg(Color::Green)),
                            ),
                            Span::styled(
                                status_suffix.to_string(),
                                apply_bg(Style::default().fg(status_color)),
                            ),
                            Span::styled(
                                time_suffix.clone(),
                                apply_bg(Style::default().fg(Color::DarkGray)),
                            ),
                        ]));
                    } else {
                        let visible_len = marker.len() + line_text.len();
                        let padding = if chat_width > visible_len {
                            " ".repeat(chat_width - visible_len)
                        } else {
                            String::new()
                        };
                        all_lines.push(Line::from(vec![
                            Span::styled(" ", marker_style),
                            Span::styled(padding, apply_bg(Style::default())),
                            Span::styled(
                                line_text.clone(),
                                apply_bg(Style::default().fg(Color::Green)),
                            ),
                        ]));
                    }
                } else if is_last_line {
                    all_lines.push(Line::from(vec![
                        Span::styled(if line_idx == 0 { marker } else { " " }, marker_style),
                        Span::styled(
                            line_text.clone(),
                            apply_bg(Style::default().fg(Color::White)),
                        ),
                        Span::styled(
                            format!("  {}", time),
                            apply_bg(Style::default().fg(Color::DarkGray)),
                        ),
                    ]));
                } else {
                    all_lines.push(Line::from(vec![
                        Span::styled(if line_idx == 0 { marker } else { " " }, marker_style),
                        Span::styled(
                            line_text.clone(),
                            apply_bg(Style::default().fg(Color::White)),
                        ),
                    ]));
                }
            }
        }

        let total_lines = all_lines.len() as u16;

        // Update app state for key handler and auto-scroll
        app.total_visual_lines = total_lines;
        app.message_line_offsets = message_line_offsets;

        let max_offset = total_lines.saturating_sub(visible_height);

        // Handle scroll positioning
        if let Some(sel) = app.selected_message {
            // In selection mode: center viewport on selected message
            let sel_start_line = app.message_line_offsets.get(sel).copied().unwrap_or(0);
            let half = visible_height / 2;
            let target = sel_start_line.saturating_sub(half);
            app.scroll_offset = target.min(max_offset);
        } else if app.scroll_pinned_to_bottom {
            app.scroll_offset = max_offset;
        }

        // Clamp scroll_offset to valid range
        app.scroll_offset = app.scroll_offset.min(max_offset);

        // Render messages using Paragraph with scroll
        let text = Text::from(all_lines);
        let paragraph = Paragraph::new(text).scroll((app.scroll_offset, 0));
        f.render_widget(paragraph, messages_area);
    }

    // Typing indicator
    if let Some(ref text) = typing_text {
        let typing = Paragraph::new(Line::from(vec![Span::styled(
            format!("  {text}"),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]));
        f.render_widget(typing, chunks[2]);
    }

    // ── Reply bar (shown above input when replying) ──
    if has_reply_bar {
        if let Some((_, ref preview)) = app.reply_to_message {
            let reply_bar = Paragraph::new(Line::from(vec![
                Span::styled(
                    "  Replying to: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    preview.as_str(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::ITALIC),
                ),
                Span::styled("  (Esc to cancel)", Style::default().fg(Color::DarkGray)),
            ]));
            f.render_widget(reply_bar, chunks[3]);
        }
    }

    // ── Input area — adapts to current mode ──
    let (input_prefix, input_text, cursor_pos, hint_text, prefix_color) =
        if app.confirm_delete.is_some() {
            (
                "Delete this message? ",
                "(y/n)".to_string(),
                5usize,
                " y: confirm | n: cancel ",
                Color::Red,
            )
        } else if app.editing_message_id.is_some() {
            (
                "[EDITING] > ",
                app.message_input.clone(),
                app.input_cursor,
                " Enter: save | Esc: cancel | ←→: move ",
                Color::Yellow,
            )
        } else if app.selected_message.is_some() {
            (
                "> ",
                String::new(),
                0,
                " r: reply | e: edit | d: delete | Esc: cancel | ↑↓: nav | PgUp/PgDn ",
                Color::Cyan,
            )
        } else if app.reply_to_message.is_some() {
            (
                "[REPLY] > ",
                app.message_input.clone(),
                app.input_cursor,
                " Enter: send reply | Esc: cancel | ←→: move ",
                Color::Cyan,
            )
        } else {
            (
                "> ",
                app.message_input.clone(),
                app.input_cursor,
                " Esc: back | Enter: send | ↑: select | PgUp/PgDn: scroll | F3: info ",
                Color::Cyan,
            )
        };

    // Split input text at cursor for rendering
    let before_cursor = &input_text[..cursor_pos.min(input_text.len())];
    let after_cursor = &input_text[cursor_pos.min(input_text.len())..];

    let input = Paragraph::new(Line::from(vec![
        Span::styled(input_prefix, Style::default().fg(prefix_color)),
        Span::styled(before_cursor, Style::default().fg(Color::White)),
        Span::styled(
            if after_cursor.is_empty() {
                " "
            } else {
                &after_cursor[..after_cursor
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| i)
                    .unwrap_or(after_cursor.len())]
            },
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::styled(
            if after_cursor.is_empty() {
                ""
            } else {
                &after_cursor[after_cursor
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| i)
                    .unwrap_or(after_cursor.len())..]
            },
            Style::default().fg(Color::White),
        ),
    ]))
    .wrap(Wrap { trim: false })
    .block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(hint_text)
            .title_alignment(Alignment::Right)
            .title_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(input, chunks[4]);
}
