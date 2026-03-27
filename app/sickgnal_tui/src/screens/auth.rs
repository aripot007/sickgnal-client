use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, AuthField};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    // Center the auth form
    let vert = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(14),
        Constraint::Min(1),
    ])
    .split(area);

    let horiz = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Length(50),
        Constraint::Min(1),
    ])
    .split(vert[1]);

    let form_area = horiz[1];

    let block = Block::default()
        .title(" Create Account ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(form_area);
    f.render_widget(block, form_area);

    let constraints = vec![
        Constraint::Length(1), // Username label
        Constraint::Length(1), // Username input
        Constraint::Length(1), // spacing
        Constraint::Length(1), // Password label
        Constraint::Length(1), // Password input
        Constraint::Length(1), // spacing
        Constraint::Length(1), // Confirm label
        Constraint::Length(1), // Confirm input
        Constraint::Length(1), // spacing
        Constraint::Length(1), // error or status
        Constraint::Min(0),   // filler
    ];

    let chunks = Layout::vertical(constraints).split(inner);

    let mut idx = 0;

    // Username
    let label_style = if app.auth_field == AuthField::Username {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("Username", label_style))),
        chunks[idx],
    );
    idx += 1;

    let (input_style, cursor) = if app.auth_field == AuthField::Username {
        (Style::default().fg(Color::Cyan), "_")
    } else {
        (Style::default().fg(Color::Gray), "")
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.username, input_style),
            Span::styled(cursor, Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)),
        ])),
        chunks[idx],
    );
    idx += 2;

    // Password
    let label_style = if app.auth_field == AuthField::Password {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("Password", label_style))),
        chunks[idx],
    );
    idx += 1;

    let (input_style, cursor) = if app.auth_field == AuthField::Password {
        (Style::default().fg(Color::Cyan), "_")
    } else {
        (Style::default().fg(Color::Gray), "")
    };
    let masked: String = "*".repeat(app.password.len());
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::DarkGray)),
            Span::styled(&masked, input_style),
            Span::styled(cursor, Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)),
        ])),
        chunks[idx],
    );
    idx += 2;

    // Confirm Password
    let label_style = if app.auth_field == AuthField::ConfirmPassword {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("Confirm Password", label_style))),
        chunks[idx],
    );
    idx += 1;

    let (input_style, cursor) = if app.auth_field == AuthField::ConfirmPassword {
        (Style::default().fg(Color::Cyan), "_")
    } else {
        (Style::default().fg(Color::Gray), "")
    };
    let masked: String = "*".repeat(app.confirm_password.len());
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::DarkGray)),
            Span::styled(&masked, input_style),
            Span::styled(cursor, Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)),
        ])),
        chunks[idx],
    );
    idx += 2;

    // Error or loading or help
    if app.auth_loading {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Connecting...",
                Style::default().fg(Color::Yellow),
            )))
            .alignment(Alignment::Center),
            chunks[idx],
        );
    } else if let Some(ref err) = app.auth_error {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                err.as_str(),
                Style::default().fg(Color::Red),
            )))
            .alignment(Alignment::Center),
            chunks[idx],
        );
    } else {
        let hint = if app.profiles.is_empty() {
            "Enter to create account"
        } else {
            "Enter to create account | Esc to go back"
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().fg(Color::DarkGray),
            )))
            .alignment(Alignment::Center),
            chunks[idx],
        );
    }
}
