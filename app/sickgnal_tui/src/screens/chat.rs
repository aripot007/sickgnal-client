use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;
use sickgnal_core::chat::storage::MessageStatus;

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::vertical([
        Constraint::Length(3), // Header with conversation name
        Constraint::Min(1),    // Messages area
        Constraint::Length(3), // Input area
    ])
    .split(area);

    // Find current conversation and peer fingerprint
    let (conv_name, fingerprint) = app
        .current_conversation
        .and_then(|cid| app.conversations.iter().find(|c| c.id == cid))
        .map(|c| {
            let fp = app
                .sdk
                .as_ref()
                .map(|sdk| sdk.get_peer_fingerprint(c.peer_user_id))
                .unwrap_or_default();
            (c.peer_name.clone(), fp)
        })
        .unwrap_or_else(|| ("Chat".into(), String::new()));

    // Format fingerprint for display (groups of 4 hex chars)
    let fp_display = if fingerprint.is_empty() {
        String::new()
    } else {
        let grouped: Vec<&str> = fingerprint
            .as_bytes()
            .chunks(4)
            .map(|c| std::str::from_utf8(c).unwrap_or(""))
            .collect();
        format!("  [{}]", grouped.join(" "))
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

    // Messages
    let messages_area = chunks[1];

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

        for msg in &app.messages {
            let is_mine = my_id.is_some_and(|id| id == msg.sender_id);
            let time = msg.timestamp.format("%H:%M").to_string();

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

            if is_mine {
                // Right-aligned: pad with spaces so content sits on the right
                let content = format!("{}{} ", msg.content, status_str);
                let time_part = format!(" {}", time);
                let visible_len = content.len() + time_part.len();
                let padding = if width > visible_len {
                    " ".repeat(width - visible_len)
                } else {
                    String::new()
                };

                let line = Line::from(vec![
                    Span::styled(padding, Style::default()),
                    Span::styled(&msg.content, Style::default().fg(Color::Green)),
                    Span::styled(status_str, Style::default().fg(status_color)),
                    Span::styled(format!(" {}", time), Style::default().fg(Color::DarkGray)),
                ]);
                items.push(ListItem::new(line));
            } else {
                // Left-aligned messages (from peer)
                let line = Line::from(vec![
                    Span::styled(&msg.content, Style::default().fg(Color::White)),
                    Span::styled(format!("  {}", time), Style::default().fg(Color::DarkGray)),
                ]);
                items.push(ListItem::new(line));
            }
        }

        // Apply scroll: show from the bottom, scrolling up
        let visible_height = messages_area.height as usize;
        let total = items.len();
        let offset = app.scroll_offset as usize;

        let end = total.saturating_sub(offset);
        let start = end.saturating_sub(visible_height);

        let visible_items: Vec<ListItem> =
            items.into_iter().skip(start).take(end - start).collect();

        let list = List::new(visible_items);
        f.render_widget(list, messages_area);
    }

    // Input area
    let input = Paragraph::new(Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Cyan)),
        Span::styled(&app.message_input, Style::default().fg(Color::White)),
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
            .title(" Esc: back | Enter: send ")
            .title_alignment(Alignment::Right)
            .title_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(input, chunks[2]);
}
