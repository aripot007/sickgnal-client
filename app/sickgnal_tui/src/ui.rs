use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Span,
    widgets::Paragraph,
};

use crate::app::{App, Screen};
use crate::screens;

pub fn draw(f: &mut Frame, app: &mut App) {
    // Clean up expired toasts (5-second timeout)
    if let Some(time) = app.toast_time {
        if time.elapsed().as_secs() >= 5 {
            app.toast_message = None;
            app.toast_time = None;
        }
    }

    // Render the current screen
    match app.screen {
        Screen::ProfileSelect => screens::profile::draw(f, app),
        Screen::Auth => screens::auth::draw(f, app),
        Screen::Conversations => screens::conversations::draw(f, app),
        Screen::Chat => screens::chat::draw(f, app),
        Screen::ConversationInfo => screens::conversation_info::draw(f, app),
    }

    // Overlay toast at the bottom if active
    if let Some(ref msg) = app.toast_message {
        let area = f.area();
        let toast_area = Rect {
            x: 0,
            y: area.height.saturating_sub(1),
            width: area.width,
            height: 1,
        };
        let (fg, bg) = if app.toast_is_error {
            (Color::White, Color::Red)
        } else {
            (Color::White, Color::DarkGray)
        };
        let toast = Paragraph::new(Span::styled(
            format!(" {msg} "),
            Style::default().fg(fg).bg(bg),
        ));
        f.render_widget(toast, toast_area);
    }
}
