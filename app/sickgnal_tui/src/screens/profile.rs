use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, SPINNER_FRAMES};

const CARD_WIDTH: u16 = 16;
const CARD_HEIGHT: u16 = 7;

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    // Vertical layout: title, cards, password/help
    let chunks = Layout::vertical([
        Constraint::Min(3),              // Top spacer
        Constraint::Length(3),           // Title
        Constraint::Length(1),           // Spacer
        Constraint::Length(CARD_HEIGHT), // Cards row
        Constraint::Length(2),           // Spacer
        Constraint::Length(3),           // Password / help area
        Constraint::Min(1),              // Bottom spacer
    ])
    .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![Span::styled(
        "Who's using sickgnal?",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )]))
    .alignment(Alignment::Center);
    f.render_widget(title, chunks[1]);

    // Calculate card positions (centered horizontally)
    let total_cards = app.profiles.len() + 1; // profiles + "+" card
    let total_width = total_cards as u16 * (CARD_WIDTH + 2); // cards + gaps
    let cards_area = chunks[3];

    // Center the cards row
    let horiz = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Length(total_width),
        Constraint::Min(1),
    ])
    .split(cards_area);

    let card_slots: Vec<Rect> = (0..total_cards)
        .map(|i| {
            let x = horiz[1].x + i as u16 * (CARD_WIDTH + 2);
            Rect::new(x, horiz[1].y, CARD_WIDTH, CARD_HEIGHT)
        })
        .collect();

    // Draw each profile card
    for (i, profile) in app.profiles.iter().enumerate() {
        let is_selected = i == app.selected_profile;
        let card_area = card_slots[i];

        let border_color = if is_selected {
            Color::Cyan
        } else {
            Color::DarkGray
        };

        let initial = profile
            .username
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(card_area);
        f.render_widget(block, card_area);

        // Card content: centered initial + name
        let content_lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                &initial,
                Style::default()
                    .fg(if is_selected {
                        Color::Cyan
                    } else {
                        Color::White
                    })
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                profile.username.clone(),
                Style::default().fg(if is_selected {
                    Color::Cyan
                } else {
                    Color::Gray
                }),
            )),
        ];

        let card_text = Paragraph::new(content_lines).alignment(Alignment::Center);
        f.render_widget(card_text, inner);
    }

    // Draw "+" new account card
    {
        let i = app.profiles.len();
        let is_selected = app.selected_profile == i;
        let card_area = card_slots[i];

        let border_color = if is_selected {
            Color::Green
        } else {
            Color::DarkGray
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(card_area);
        f.render_widget(block, card_area);

        let content_lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "+",
                Style::default()
                    .fg(if is_selected {
                        Color::Green
                    } else {
                        Color::DarkGray
                    })
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "New",
                Style::default().fg(if is_selected {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            )),
        ];

        let card_text = Paragraph::new(content_lines).alignment(Alignment::Center);
        f.render_widget(card_text, inner);
    }

    // Bottom area: password input, loading spinner, or help text
    let bottom_area = chunks[5];

    if app.auth_loading {
        // Show spinner while connecting after password entry
        let frame = SPINNER_FRAMES[app.auth_spinner_tick % SPINNER_FRAMES.len()];
        let spinner = Paragraph::new(Line::from(vec![
            Span::styled(format!("{frame} "), Style::default().fg(Color::Cyan)),
            Span::styled("Signing in...", Style::default().fg(Color::Yellow)),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(spinner, bottom_area);
    } else if app.profile_password_mode {
        let profile = &app.profiles[app.selected_profile];
        let masked: String = "*".repeat(app.profile_password.len());

        let input = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("Password for {}: ", profile.username),
                Style::default().fg(Color::White),
            ),
            Span::styled(&masked, Style::default().fg(Color::Cyan)),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(input, bottom_area);
    } else if let Some(ref err) = app.profile_error {
        let error = Paragraph::new(Line::from(Span::styled(
            err.as_str(),
            Style::default().fg(Color::Red),
        )))
        .alignment(Alignment::Center);
        f.render_widget(error, bottom_area);
    } else {
        let help = Paragraph::new(Line::from(Span::styled(
            "< > to select | Enter to sign in | d to delete | q to quit",
            Style::default().fg(Color::DarkGray),
        )))
        .alignment(Alignment::Center);
        f.render_widget(help, bottom_area);
    }
}
