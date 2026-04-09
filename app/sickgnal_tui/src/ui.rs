use ratatui::Frame;

use crate::app::{App, Screen};
use crate::screens;

pub fn draw(f: &mut Frame, app: &mut App) {
    match app.screen {
        Screen::ProfileSelect => screens::profile::draw(f, app),
        Screen::Auth => screens::auth::draw(f, app),
        Screen::Conversations => screens::conversations::draw(f, app),
        Screen::Chat => screens::chat::draw(f, app),
        Screen::ConversationInfo => screens::conversation_info::draw(f, app),
    }
}
