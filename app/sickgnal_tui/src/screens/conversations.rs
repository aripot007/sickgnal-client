use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(1),    // Conversation list
        Constraint::Length(3), // Status / help bar
    ])
    .split(area);

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "  sickgnal",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" - ", Style::default().fg(Color::White)),
        Span::styled(
            &app.username,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(header, chunks[0]);

    // Conversation list
    if app.conversations.is_empty() && !app.new_conversation_mode {
        let empty = Paragraph::new(Line::from(vec![Span::styled(
            "No conversations yet. Press 'n' to start one.",
            Style::default().fg(Color::DarkGray),
        )]))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::NONE));
        f.render_widget(empty, chunks[1]);
    } else {
        let items: Vec<ListItem> = app
            .conversations
            .iter()
            .enumerate()
            .map(|(i, conv)| {
                let is_selected = i == app.selected_conversation;

                let unread_str = if conv.unread_count > 0 {
                    format!(" ({})", conv.unread_count)
                } else {
                    String::new()
                };

                let time = conv
                    .last_message_at
                    .map(|t| t.format("%H:%M").to_string())
                    .unwrap_or_default();

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let marker = if is_selected { "> " } else { "  " };

                let line = Line::from(vec![
                    Span::styled(marker, style),
                    Span::styled(conv.peer_name.clone(), style),
                    Span::styled(unread_str, Style::default().fg(Color::Yellow)),
                    Span::styled(format!("  {}", time), Style::default().fg(Color::DarkGray)),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::NONE)
                .title_alignment(Alignment::Left),
        );
        f.render_widget(list, chunks[1]);
    }

    // Status bar / help
    let help_text = if app.new_conversation_mode {
        Line::from(vec![
            Span::styled(
                "New conversation with: ",
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                &app.new_conversation_username,
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
            Span::styled(
                "  | Enter to create | Esc to cancel",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        let status = app.status_message.as_deref().unwrap_or("");

        Line::from(vec![
            Span::styled(
                " n: new | Enter: open | d: delete | q: quit",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(format!("  {}", status), Style::default().fg(Color::Green)),
        ])
    };

    let status_bar = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(status_bar, chunks[2]);
}
