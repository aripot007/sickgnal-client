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
            .map(|(i, entry)| {
                let is_selected = i == app.selected_conversation;

                let unread_str = if entry.unread_messages_count > 0 {
                    format!(" ({})", entry.unread_messages_count)
                } else {
                    String::new()
                };

                let time = entry
                    .last_message
                    .as_ref()
                    .map(|m| m.issued_at.format("%H:%M").to_string())
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
                    Span::styled(entry.conversation.title.clone(), style),
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
    let added_display = if app.group_conversation_mode {
        let added = app
            .group_conversation_usernames
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if added.is_empty() {
            String::new()
        } else {
            format!("[{}] ", added)
        }
    } else {
        String::new()
    };

    let help_text = if app.group_conversation_mode {
        Line::from(vec![
            Span::styled(
                "Group: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(added_display, Style::default().fg(Color::Yellow)),
            Span::styled(
                &app.group_conversation_input,
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
            Span::styled(
                "  | Enter: add user | F5: create | Esc: cancel",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else if app.new_conversation_mode {
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
        Line::from(vec![Span::styled(
            " n: new | g: group | Enter: open | d: delete | q: quit",
            Style::default().fg(Color::DarkGray),
        )])
    };

    let status_bar = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(status_bar, chunks[2]);
}
