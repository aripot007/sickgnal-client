use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;
use sickgnal_core::chat::storage::MessageStatus;

/// Compute a horizontally centred sub-rect that is 60 % of the terminal
/// width but never narrower than `min` columns.
fn centered_rect(area: Rect, min: u16) -> Rect {
    let target = (area.width as u32 * 60 / 100) as u16;
    let w = target.max(min).min(area.width);
    let pad = (area.width.saturating_sub(w)) / 2;
    Rect::new(area.x + pad, area.y, w, area.height)
}

pub fn draw(f: &mut Frame, app: &App) {
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

    let chunks = Layout::vertical([
        Constraint::Length(3), // Header with conversation name
        Constraint::Min(1),    // Messages area
        Constraint::Length(if typing_text.is_some() { 1 } else { 0 }), // Typing indicator
        Constraint::Length(if has_reply_bar { 1 } else { 0 }), // Reply bar
        Constraint::Length(3), // Input area
    ])
    .split(area);

    // Find current conversation and peer fingerprint
    let (conv_name, fingerprint) = app
        .current_conversation
        .and_then(|cid| app.conversations.iter().find(|e| e.conversation.id == cid))
        .map(|entry| {
            let fp = entry
                .conversation
                .peers
                .first()
                .map(|p| p.format_fingerprint())
                .unwrap_or_default();
            (entry.conversation.title.clone(), fp)
        })
        .unwrap_or_else(|| ("Chat".into(), String::new()));

    // Format fingerprint for display
    let fp_display = if fingerprint.is_empty() || fingerprint == "no fingerprint" {
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

    if app.messages.is_empty() {
        let empty = Paragraph::new(Line::from(vec![Span::styled(
            "No messages yet. Type something below.",
            Style::default().fg(Color::DarkGray),
        )]))
        .alignment(Alignment::Center);
        f.render_widget(empty, messages_area);
    } else {
        let my_id = app.my_user_id;
        let width = messages_area.width as usize;

        // Build message lines
        let mut items: Vec<ListItem> = Vec::new();

        for (idx, msg) in app.messages.iter().enumerate() {
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

            // Highlight background for selected message
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

            let mut lines: Vec<Line> = Vec::new();

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
                    // Right-aligned quote
                    let quote_text = format!("  {} ", reply_preview);
                    let quote_len = marker.len() + quote_text.len();
                    let padding = if width > quote_len {
                        " ".repeat(width - quote_len)
                    } else {
                        String::new()
                    };
                    lines.push(Line::from(vec![
                        Span::styled(marker, marker_style),
                        Span::styled(padding, Style::default()),
                        Span::styled(
                            format!("│ {reply_preview}"),
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]));
                } else {
                    // Left-aligned quote
                    lines.push(Line::from(vec![
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

            // ── Message content ──
            if is_mine {
                // Right-aligned: pad with spaces so content sits on the right
                let content = format!("{}{} ", msg.content, status_str);
                let time_part = format!(" {}", time);
                let visible_len = marker.len() + content.len() + time_part.len();
                let padding = if width > visible_len {
                    " ".repeat(width - visible_len)
                } else {
                    String::new()
                };

                lines.push(Line::from(vec![
                    Span::styled(marker, marker_style),
                    Span::styled(padding, apply_bg(Style::default())),
                    Span::styled(&msg.content, apply_bg(Style::default().fg(Color::Green))),
                    Span::styled(status_str, apply_bg(Style::default().fg(status_color))),
                    Span::styled(
                        format!(" {}", time),
                        apply_bg(Style::default().fg(Color::DarkGray)),
                    ),
                ]));
            } else {
                // Left-aligned messages (from peer)
                lines.push(Line::from(vec![
                    Span::styled(marker, marker_style),
                    Span::styled(&msg.content, apply_bg(Style::default().fg(Color::White))),
                    Span::styled(
                        format!("  {}", time),
                        apply_bg(Style::default().fg(Color::DarkGray)),
                    ),
                ]));
            }

            items.push(ListItem::new(lines));
        }

        // Apply scroll: in selection mode, center on selected message;
        // otherwise show from the bottom with scroll offset.
        let visible_height = messages_area.height as usize;
        let total = items.len();

        let (start, end) = if let Some(sel) = app.selected_message {
            // Center the selected message in the view
            let half = visible_height / 2;
            let center_start = sel.saturating_sub(half);
            let center_end = (center_start + visible_height).min(total);
            let center_start = center_end.saturating_sub(visible_height);
            (center_start, center_end)
        } else {
            let offset = app.scroll_offset as usize;
            let end = total.saturating_sub(offset);
            let start = end.saturating_sub(visible_height);
            (start, end)
        };

        let visible_items: Vec<ListItem> =
            items.into_iter().skip(start).take(end - start).collect();

        let list = List::new(visible_items);
        f.render_widget(list, messages_area);
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
    let (input_prefix, input_text, hint_text, prefix_color) = if app.confirm_delete.is_some() {
        (
            "Delete this message? ",
            "(y/n)".to_string(),
            " y: confirm | n: cancel ",
            Color::Red,
        )
    } else if app.editing_message_id.is_some() {
        (
            "[EDITING] > ",
            app.message_input.clone(),
            " Enter: save | Esc: cancel ",
            Color::Yellow,
        )
    } else if app.selected_message.is_some() {
        (
            "> ",
            String::new(),
            " r: reply | e: edit | d: delete | Esc: cancel | ↑↓: nav ",
            Color::Cyan,
        )
    } else if app.reply_to_message.is_some() {
        (
            "[REPLY] > ",
            app.message_input.clone(),
            " Enter: send reply | Esc: cancel reply ",
            Color::Cyan,
        )
    } else {
        (
            "> ",
            app.message_input.clone(),
            " Esc: back | Enter: send | ↑: select message ",
            Color::Cyan,
        )
    };

    let input = Paragraph::new(Line::from(vec![
        Span::styled(input_prefix, Style::default().fg(prefix_color)),
        Span::styled(&input_text, Style::default().fg(Color::White)),
        Span::styled(
            "_",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]))
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
