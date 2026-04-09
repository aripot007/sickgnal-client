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
        Constraint::Length(5), // Title area
        Constraint::Min(1),    // Peer list
        Constraint::Length(5), // Fingerprint area
    ])
    .split(area);

    // Find the current conversation entry
    let entry = app
        .current_conversation
        .and_then(|cid| app.conversations.iter().find(|e| e.conversation.id == cid));

    // ── Title area ──
    let (title, member_count) = entry
        .map(|e| (e.conversation.title.clone(), e.conversation.peers.len()))
        .unwrap_or_else(|| ("Unknown".into(), 0));

    let title_widget = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "  Conversation Info",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Name: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&title, Style::default().fg(Color::White)),
            Span::styled(
                format!(
                    "  ({} member{})",
                    member_count,
                    if member_count != 1 { "s" } else { "" }
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(title_widget, chunks[0]);

    // ── Peer list ──
    let peers = entry
        .map(|e| e.conversation.peers.as_slice())
        .unwrap_or(&[]);

    let items: Vec<ListItem> = peers
        .iter()
        .enumerate()
        .map(|(i, peer)| {
            let is_selected = i == app.info_selected_peer;
            let marker = if is_selected { " >> " } else { "    " };
            let name = peer.name();
            let id_str = format!("  ({})", &peer.id.to_string()[..8]);

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(name, style),
                Span::styled(id_str, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let peer_list = List::new(items).block(
        Block::default()
            .title(" Members ")
            .title_style(Style::default().fg(Color::Cyan))
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(peer_list, chunks[1]);

    // ── Fingerprint area ──
    let fingerprint_content = if app.info_show_fingerprint {
        if let Some(peer) = peers.get(app.info_selected_peer) {
            let fp = app
                .sdk
                .as_ref()
                .map(|sdk| sdk.get_peer_fingerprint(peer.id))
                .unwrap_or_default();

            // Format fingerprint in groups of 4 hex chars
            let grouped: Vec<&str> = fp
                .as_bytes()
                .chunks(4)
                .map(|c| std::str::from_utf8(c).unwrap_or(""))
                .collect();

            vec![
                Line::from(vec![Span::styled(
                    format!("  Fingerprint for {}: ", peer.name()),
                    Style::default().fg(Color::DarkGray),
                )]),
                Line::from(vec![Span::styled(
                    format!("  {}", grouped.join(" ")),
                    Style::default().fg(Color::Yellow),
                )]),
            ]
        } else {
            vec![Line::from("")]
        }
    } else {
        vec![Line::from(vec![Span::styled(
            "  Press Enter to show fingerprint for selected peer",
            Style::default().fg(Color::DarkGray),
        )])]
    };

    let fp_widget = Paragraph::new(fingerprint_content).block(
        Block::default()
            .title(" Esc: back | ↑↓: navigate | Enter: fingerprint ")
            .title_alignment(Alignment::Right)
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(fp_widget, chunks[2]);
}
