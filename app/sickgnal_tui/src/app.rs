use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent};
use sickgnal_core::chat::client::ChatEvent as SdkEvent;
use sickgnal_core::chat::storage::Message;
use sickgnal_sdk::TlsConfig;
use sickgnal_sdk::client::{self, SyncBridge};
use sickgnal_sdk::dto::ConversationEntry;
use tokio::sync::mpsc;
use tracing::{error, warn};
use uuid::Uuid;

use sickgnal_sdk::account::{Profile, ProfileManager};

// ─── Spinner ───────────────────────────────────────────────────────────────

/// Braille spinner frames for loading animation.
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// ─── User-friendly error mapping ───────────────────────────────────────────

/// Convert an SDK error into a clean, user-facing message.
///
/// The raw error is always logged via `tracing::error!` before this is called.
pub fn friendly_error(context: &str, err: &client::Error) -> String {
    use sickgnal_core::chat::client::Error as ChatErr;

    match err {
        // ── Connection / IO errors ──
        client::Error::Io(_) => "Could not reach the server".into(),

        // ── E2E-level errors ──
        client::Error::E2E(e2e) => friendly_e2e_error(context, e2e),

        // ── Chat-client-level errors ──
        client::Error::Client(chat) => match chat {
            ChatErr::NotConnected => "Not connected to server".into(),
            ChatErr::ConversationNotFound(_) => "Conversation not found".into(),
            ChatErr::MessageNotFound(_, _) => "Message not found".into(),
            ChatErr::UnknownPeer(_) => "Unknown peer".into(),
            ChatErr::E2E(e2e) => friendly_e2e_error(context, e2e),
            _ => format!("{context} failed"),
        },

        // ── Local auth errors ──
        client::Error::InvalidPassword => "Incorrect password".into(),
        client::Error::NoAccount => "No account found".into(),

        // ── Storage / other ──
        client::Error::Storage(_) => "A storage error occurred".into(),
        _ => format!("{context} failed"),
    }
}

/// Helper to map E2E-layer errors to user-friendly messages.
fn friendly_e2e_error(context: &str, e2e: &sickgnal_core::e2e::client::Error) -> String {
    use sickgnal_core::e2e::client::Error as E2EErr;

    match e2e {
        E2EErr::UserNotFound => "User not found".into(),
        E2EErr::NoPrekeyAvailable => "User has not finished setting up their account".into(),
        E2EErr::MessageStreamError(_) => "Lost connection to the server".into(),
        E2EErr::WorkerSendError | E2EErr::ReceiveWorkerStopped => {
            "Connection to the server was lost".into()
        }
        _ => format!("{context} failed"),
    }
}

// ─── Screens ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    ProfileSelect,
    Auth,
    Conversations,
    Chat,
    ConversationInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    SignUp,
    SignIn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthField {
    Username,
    Password,
    ConfirmPassword,
}

// ─── App state ─────────────────────────────────────────────────────────────

pub struct App {
    pub should_quit: bool,
    pub screen: Screen,

    // Profile selection state
    pub profile_manager: ProfileManager,
    pub profiles: Vec<Profile>,
    pub selected_profile: usize,
    pub profile_password: String,
    pub profile_password_mode: bool,
    pub profile_error: Option<String>,

    // Auth state
    pub auth_mode: AuthMode,
    pub auth_field: AuthField,
    pub username: String,
    pub password: String,
    pub confirm_password: String,
    pub auth_error: Option<String>,
    pub auth_loading: bool,

    // Conversations state
    pub conversations: Vec<ConversationEntry>,
    pub selected_conversation: usize,
    pub new_conversation_mode: bool,
    pub new_conversation_username: String,

    // Group conversation creation
    pub group_conversation_mode: bool,
    pub group_conversation_input: String,
    pub group_conversation_usernames: Vec<String>,
    pub group_conversation_peer_ids: Vec<Uuid>,

    // Chat state
    pub current_conversation: Option<Uuid>,
    pub messages: Vec<Message>,
    pub message_input: String,
    pub input_cursor: usize, // byte offset into message_input
    pub scroll_offset: u16,  // visual line offset from top (used by Paragraph::scroll)
    pub scroll_pinned_to_bottom: bool, // if true, draw() will snap scroll to bottom
    pub messages_area_height: u16,     // visible height in lines, updated by draw()
    pub total_visual_lines: u16,       // total wrapped lines, updated by draw()
    pub message_line_offsets: Vec<u16>, // start visual line of each message, updated by draw()
    pub my_user_id: Option<Uuid>,

    // Message selection / editing / deletion
    pub selected_message: Option<usize>,
    pub editing_message_id: Option<Uuid>,
    pub original_message_text: String,
    pub confirm_delete: Option<Uuid>,

    // Reply state
    pub reply_to_message: Option<(Uuid, String)>, // (message_id, preview text)

    // Conversation info state
    pub info_selected_peer: usize,
    pub info_show_fingerprint: bool,
    pub info_add_member_mode: bool,
    pub info_add_member_input: String,

    // Typing indicators
    pub last_typing_sent: Option<Instant>,
    pub typing_indicators: HashMap<Uuid, (String, Instant)>,

    // Toast notification
    pub toast_message: Option<String>,
    pub toast_is_error: bool,
    pub toast_time: Option<Instant>,

    // SDK bridge
    pub sdk: Option<SyncBridge>,
    pub event_rx: Option<mpsc::Receiver<SdkEvent>>,

    // Storage dir
    pub data_dir: PathBuf,

    // Connection settings
    pub server_addr: String,
    pub tls_config: TlsConfig,

    // Async auth: background thread handle + spinner state
    pub auth_handle:
        Option<thread::JoinHandle<Result<(SyncBridge, mpsc::Receiver<SdkEvent>), client::Error>>>,
    pub auth_spinner_tick: usize,
    pub auth_was_signup: bool,
}

impl App {
    pub fn new(data_dir: PathBuf, server_addr: String, tls_config: TlsConfig) -> Self {
        let profile_manager =
            ProfileManager::new(data_dir.clone()).expect("create profile manager");
        let profiles = profile_manager.list_profiles().unwrap_or_default();

        // If no profiles exist, go straight to auth (sign-up).
        // data_dir will be set from the username during sign-up.
        let screen = if profiles.is_empty() {
            Screen::Auth
        } else {
            Screen::ProfileSelect
        };

        Self {
            should_quit: false,
            screen,

            profile_manager,
            profiles,
            selected_profile: 0,
            profile_password: String::new(),
            profile_password_mode: false,
            profile_error: None,

            auth_mode: AuthMode::SignUp,
            auth_field: AuthField::Username,
            username: String::new(),
            password: String::new(),
            confirm_password: String::new(),
            auth_error: None,
            auth_loading: false,

            conversations: Vec::new(),
            selected_conversation: 0,
            new_conversation_mode: false,
            new_conversation_username: String::new(),

            group_conversation_mode: false,
            group_conversation_input: String::new(),
            group_conversation_usernames: Vec::new(),
            group_conversation_peer_ids: Vec::new(),

            current_conversation: None,
            messages: Vec::new(),
            message_input: String::new(),
            input_cursor: 0,
            scroll_offset: 0,
            scroll_pinned_to_bottom: true,
            messages_area_height: 0,
            total_visual_lines: 0,
            message_line_offsets: Vec::new(),
            my_user_id: None,

            selected_message: None,
            editing_message_id: None,
            original_message_text: String::new(),
            confirm_delete: None,

            reply_to_message: None,

            info_selected_peer: 0,
            info_show_fingerprint: false,
            info_add_member_mode: false,
            info_add_member_input: String::new(),

            last_typing_sent: None,
            typing_indicators: HashMap::new(),

            toast_message: None,
            toast_is_error: false,
            toast_time: None,

            sdk: None,
            event_rx: None,

            data_dir: PathBuf::new(),

            server_addr,
            tls_config,

            auth_handle: None,
            auth_spinner_tick: 0,
            auth_was_signup: false,
        }
    }

    /// Returns a TLS warning message if the connection is not using production TLS.
    pub fn tls_warning(&self) -> Option<&'static str> {
        match &self.tls_config {
            TlsConfig::None => Some(
                "WARNING: TLS is disabled \u{2014} your connection to the server is not encrypted",
            ),
            TlsConfig::Insecure { .. } => Some(
                "WARNING: Custom TLS implementation in use \u{2014} the connection may be less secure",
            ),
            TlsConfig::Rustls { .. } => None,
        }
    }

    // ─── Cursor helpers ─────────────────────────────────────────────────

    /// Insert a character at the current cursor position in `message_input`.
    fn input_insert(&mut self, c: char) {
        self.message_input.insert(self.input_cursor, c);
        self.input_cursor += c.len_utf8();
    }

    /// Delete the character before the cursor in `message_input`.
    fn input_backspace(&mut self) {
        if self.input_cursor > 0 {
            // Find the previous char boundary
            let prev = self.message_input[..self.input_cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.message_input.remove(prev);
            self.input_cursor = prev;
        }
    }

    /// Move cursor left by one character.
    fn input_cursor_left(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor = self.message_input[..self.input_cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right by one character.
    fn input_cursor_right(&mut self) {
        if self.input_cursor < self.message_input.len() {
            self.input_cursor = self.message_input[self.input_cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.input_cursor + i)
                .unwrap_or(self.message_input.len());
        }
    }

    /// Clear message_input and reset cursor.
    fn input_clear(&mut self) {
        self.message_input.clear();
        self.input_cursor = 0;
    }

    /// Set message_input to a string and place cursor at end.
    fn input_set(&mut self, s: String) {
        self.input_cursor = s.len();
        self.message_input = s;
    }

    /// Show a toast notification.
    pub fn show_toast(&mut self, msg: impl Into<String>, is_error: bool) {
        self.toast_message = Some(msg.into());
        self.toast_is_error = is_error;
        self.toast_time = Some(Instant::now());
    }

    /// Show a red error toast.
    pub fn show_error_toast(&mut self, msg: impl Into<String>) {
        self.show_toast(msg, true);
    }

    /// Show a green info toast.
    pub fn show_info_toast(&mut self, msg: impl Into<String>) {
        self.show_toast(msg, false);
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.screen {
            Screen::ProfileSelect => self.handle_profile_key(key),
            Screen::Auth => self.handle_auth_key(key),
            Screen::Conversations => self.handle_conversations_key(key),
            Screen::Chat => self.handle_chat_key(key),
            Screen::ConversationInfo => self.handle_conversation_info_key(key),
        }
    }

    // ─── Profile selection key handling ─────────────────────────────

    fn handle_profile_key(&mut self, key: KeyEvent) {
        // Block input while auth is loading
        if self.auth_loading {
            return;
        }

        // Password entry mode: user selected a profile, now typing password
        if self.profile_password_mode {
            match key.code {
                KeyCode::Esc => {
                    self.profile_password_mode = false;
                    self.profile_password.clear();
                    self.profile_error = None;
                }
                KeyCode::Char(c) => {
                    self.profile_password.push(c);
                    self.profile_error = None;
                }
                KeyCode::Backspace => {
                    self.profile_password.pop();
                    self.profile_error = None;
                }
                KeyCode::Enter => {
                    if self.profile_password.is_empty() {
                        self.profile_error = Some("Password required".into());
                        return;
                    }
                    // Try to sign in directly
                    let profile = &self.profiles[self.selected_profile];
                    let dir = match self.profile_manager.profile_dir(&profile.name) {
                        Ok(d) => d,
                        Err(e) => {
                            self.profile_error = Some(format!("{e}"));
                            return;
                        }
                    };

                    // Verify password
                    let account_file = match sickgnal_sdk::account::AccountFile::new(dir.clone()) {
                        Ok(af) => af,
                        Err(e) => {
                            self.profile_error = Some(format!("{e}"));
                            return;
                        }
                    };

                    match account_file.verify(&profile.username, &self.profile_password) {
                        Ok(true) => {
                            // Password correct — connect
                            self.data_dir = dir;
                            self.username = profile.username.clone();
                            self.password = self.profile_password.clone();
                            self.profile_password.clear();
                            self.profile_password_mode = false;
                            self.auth_mode = AuthMode::SignIn;
                            self.attempt_auth();
                        }
                        Ok(false) => {
                            self.profile_error = Some("Wrong password".into());
                        }
                        Err(e) => {
                            self.profile_error = Some(format!("{e}"));
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        // Profile selection mode
        // The last slot is always "+ New Account"
        let total = self.profiles.len() + 1; // profiles + "new" card

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Left | KeyCode::Char('h') => {
                if self.selected_profile > 0 {
                    self.selected_profile -= 1;
                    self.profile_error = None;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.selected_profile < total - 1 {
                    self.selected_profile += 1;
                    self.profile_error = None;
                }
            }
            KeyCode::Enter => {
                if self.selected_profile == self.profiles.len() {
                    // "+ New Account" selected — go to sign-up
                    // Use a new profile dir named after whatever they sign up as
                    self.data_dir = PathBuf::new(); // will be set during auth
                    self.auth_mode = AuthMode::SignUp;
                    self.auth_field = AuthField::Username;
                    self.username.clear();
                    self.password.clear();
                    self.confirm_password.clear();
                    self.auth_error = None;
                    self.screen = Screen::Auth;
                } else {
                    // Existing profile — show password input
                    self.profile_password_mode = true;
                    self.profile_password.clear();
                    self.profile_error = None;
                }
            }
            KeyCode::Char('d') => {
                if self.selected_profile < self.profiles.len() {
                    let name = self.profiles[self.selected_profile].name.clone();
                    if let Err(e) = self.profile_manager.delete_profile(&name) {
                        warn!("Failed to delete profile '{}': {e}", name);
                    }
                    self.profiles = self.profile_manager.list_profiles().unwrap_or_default();
                    if self.selected_profile >= self.profiles.len() + 1 {
                        self.selected_profile = self.profiles.len(); // clamp to "+" card
                    }
                    // If no profiles left, go to auth
                    if self.profiles.is_empty() {
                        self.data_dir = self
                            .profile_manager
                            .profile_dir("default")
                            .unwrap_or_else(|_| PathBuf::from("./tui_storage/default"));
                        self.screen = Screen::Auth;
                    }
                }
            }
            _ => {}
        }
    }

    // ─── Auth key handling (sign-up only) ─────────────────────────────────

    fn handle_auth_key(&mut self, key: KeyEvent) {
        if self.auth_loading {
            return;
        }

        match key.code {
            KeyCode::Esc => {
                // Go back to profile selection if profiles exist
                if !self.profiles.is_empty() {
                    self.screen = Screen::ProfileSelect;
                    self.auth_error = None;
                }
            }
            KeyCode::Up | KeyCode::BackTab => {
                self.auth_field = match self.auth_field {
                    AuthField::Username => AuthField::Username,
                    AuthField::Password => AuthField::Username,
                    AuthField::ConfirmPassword => AuthField::Password,
                };
            }
            KeyCode::Down | KeyCode::Tab => {
                self.auth_field = match self.auth_field {
                    AuthField::Username => AuthField::Password,
                    AuthField::Password => AuthField::ConfirmPassword,
                    AuthField::ConfirmPassword => AuthField::ConfirmPassword,
                };
            }
            KeyCode::Char(c) => {
                match self.auth_field {
                    AuthField::Username => self.username.push(c),
                    AuthField::Password => self.password.push(c),
                    AuthField::ConfirmPassword => self.confirm_password.push(c),
                }
                self.auth_error = None;
            }
            KeyCode::Backspace => {
                match self.auth_field {
                    AuthField::Username => {
                        self.username.pop();
                    }
                    AuthField::Password => {
                        self.password.pop();
                    }
                    AuthField::ConfirmPassword => {
                        self.confirm_password.pop();
                    }
                }
                self.auth_error = None;
            }
            KeyCode::Enter => {
                self.attempt_auth();
            }
            _ => {}
        }
    }

    fn attempt_auth(&mut self) {
        // Validate
        if self.username.is_empty() {
            self.auth_error = Some("Username cannot be empty".into());
            return;
        }
        if self.password.is_empty() {
            self.auth_error = Some("Password cannot be empty".into());
            return;
        }
        if self.auth_mode == AuthMode::SignUp && self.password != self.confirm_password {
            self.auth_error = Some("Passwords do not match".into());
            return;
        }

        self.auth_loading = true;
        self.auth_error = None;
        self.auth_spinner_tick = 0;

        // If data_dir is not set (new profile from "+" card), create one from username
        if self.data_dir.as_os_str().is_empty() {
            match self.profile_manager.profile_dir(&self.username) {
                Ok(d) => self.data_dir = d,
                Err(e) => {
                    self.auth_error = Some(format!("Storage error: {e}"));
                    self.auth_loading = false;
                    return;
                }
            }
        }

        // Handle local account file
        let account_file = match sickgnal_sdk::account::AccountFile::new(self.data_dir.clone()) {
            Ok(af) => af,
            Err(e) => {
                self.auth_error = Some(format!("Storage error: {e}"));
                self.auth_loading = false;
                return;
            }
        };

        if self.auth_mode == AuthMode::SignIn {
            // Check if an account exists before trying to verify
            if !account_file.exists() {
                self.auth_error = Some("No account found. Please sign up first.".into());
                self.auth_loading = false;
                return;
            }
            // Verify credentials
            match account_file.verify(&self.username, &self.password) {
                Ok(true) => {}
                Ok(false) => {
                    self.auth_error = Some("Incorrect password".into());
                    self.auth_loading = false;
                    return;
                }
                Err(e) => {
                    self.auth_error = Some(format!("Verification error: {e}"));
                    self.auth_loading = false;
                    return;
                }
            }
        } else {
            // Create account file
            if let Err(e) = account_file.create(&self.username, &self.password) {
                self.auth_error = Some(format!("Account creation error: {e}"));
                self.auth_loading = false;
                return;
            }
        }

        // Spawn the connection in a background thread so the UI stays responsive
        let username = self.username.clone();
        let password = self.password.clone();
        let dir = self.data_dir.clone();
        let existing = self.auth_mode == AuthMode::SignIn;
        self.auth_was_signup = self.auth_mode == AuthMode::SignUp;

        let server_addr = self.server_addr.clone();
        let tls_config = self.tls_config.clone();

        let handle = thread::spawn(move || {
            SyncBridge::connect(
                username,
                &password,
                dir,
                existing,
                &server_addr,
                &tls_config,
            )
        });

        self.auth_handle = Some(handle);
    }

    /// Called from the main loop to check if the background auth thread finished.
    pub fn poll_auth_completion(&mut self) {
        // Advance spinner tick for animation
        if self.auth_loading {
            self.auth_spinner_tick = self.auth_spinner_tick.wrapping_add(1);
        }

        let is_finished = self.auth_handle.as_ref().is_some_and(|h| h.is_finished());

        if !is_finished {
            return;
        }

        let handle = self.auth_handle.take().unwrap();
        match handle.join() {
            Ok(Ok((bridge, mut event_rx))) => {
                // Drain events emitted during sync — the DB already reflects them.
                // Only live events (after this point) should be processed.
                while event_rx.try_recv().is_ok() {}

                self.my_user_id = Some(bridge.user_id());
                self.event_rx = Some(event_rx);

                // Load conversations from storage
                match bridge.list_conversations() {
                    Ok(convos) => self.conversations = convos,
                    Err(e) => {
                        error!("Failed to list conversations: {e}");
                    }
                }

                self.sdk = Some(bridge);
                self.auth_loading = false;
                self.screen = Screen::Conversations;
                self.show_info_toast("Connected");

                // Refresh profiles list so profile selection is up to date
                self.profiles = self.profile_manager.list_profiles().unwrap_or_default();
            }
            Ok(Err(e)) => {
                error!("Auth connection failed: {e}");
                let msg = friendly_error("Connection", &e);

                // If sign-up failed, clean up the local account file so user can retry
                if self.auth_was_signup {
                    if let Ok(af) = sickgnal_sdk::account::AccountFile::new(self.data_dir.clone()) {
                        let _ = af.delete();
                    }
                    self.auth_error = Some(msg.clone());
                } else {
                    // Came from profile selection (sign-in) — go back to profile screen
                    if !self.profiles.is_empty() {
                        self.profile_error = Some(msg.clone());
                        self.screen = Screen::ProfileSelect;
                    } else {
                        self.auth_error = Some(msg.clone());
                    }
                }

                self.show_error_toast(msg);
                self.auth_loading = false;
            }
            Err(_) => {
                let msg = "Connection attempt crashed unexpectedly".to_string();

                if !self.auth_was_signup && !self.profiles.is_empty() {
                    self.profile_error = Some(msg.clone());
                    self.screen = Screen::ProfileSelect;
                } else {
                    self.auth_error = Some(msg.clone());
                }

                self.show_error_toast(msg);
                self.auth_loading = false;
            }
        }
    }

    // ─── Conversations key handling ─────────────────────────────────────

    fn handle_conversations_key(&mut self, key: KeyEvent) {
        if self.new_conversation_mode {
            self.handle_new_conversation_key(key);
            return;
        }
        if self.group_conversation_mode {
            self.handle_group_conversation_key(key);
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('n') => {
                self.new_conversation_mode = true;
                self.new_conversation_username.clear();
            }
            KeyCode::Char('g') => {
                self.group_conversation_mode = true;
                self.group_conversation_input.clear();
                self.group_conversation_usernames.clear();
                self.group_conversation_peer_ids.clear();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_conversation > 0 {
                    self.selected_conversation -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.conversations.is_empty()
                    && self.selected_conversation < self.conversations.len() - 1
                {
                    self.selected_conversation += 1;
                }
            }
            KeyCode::Enter => {
                if !self.conversations.is_empty() {
                    let entry = &self.conversations[self.selected_conversation];
                    let conv_id = entry.conversation.id;
                    self.current_conversation = Some(conv_id);

                    // Mark the messages as read
                    if let Some(ref mut sdk) = self.sdk {
                        if let Err(err) = sdk.mark_conversation_as_read(conv_id) {
                            error!("Failed to mark conversation as read: {}", err);
                            self.show_error_toast(friendly_error(
                                "Marking conversation as read",
                                &err,
                            ));
                        }
                    }

                    // Clear unread count in the entry
                    self.conversations[self.selected_conversation].unread_messages_count = 0;

                    // Load messages (SQL returns DESC; reverse to chronological order)
                    if let Some(ref sdk) = self.sdk {
                        match sdk.get_messages(conv_id) {
                            Ok(mut msgs) => {
                                msgs.reverse();
                                self.messages = msgs;
                            }
                            Err(e) => {
                                error!("Failed to load messages: {e}");
                                self.show_error_toast(friendly_error("Loading messages", &e));
                            }
                        }
                    }

                    self.input_clear();
                    self.scroll_pinned_to_bottom = true;
                    self.selected_message = None;
                    self.screen = Screen::Chat;
                }
            }
            KeyCode::Char('d') => {
                if !self.conversations.is_empty() {
                    let conv_id = self.conversations[self.selected_conversation]
                        .conversation
                        .id;
                    if let Some(ref mut sdk) = self.sdk {
                        if let Err(e) = sdk.delete_conversation(conv_id) {
                            error!("Failed to delete conversation: {e}");
                            self.show_error_toast(friendly_error("Deleting conversation", &e));
                        }
                    }
                    self.conversations.remove(self.selected_conversation);
                    if self.selected_conversation > 0
                        && self.selected_conversation >= self.conversations.len()
                    {
                        self.selected_conversation = self.conversations.len().saturating_sub(1);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_new_conversation_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.new_conversation_mode = false;
                self.new_conversation_username.clear();
            }
            KeyCode::Char(c) => {
                self.new_conversation_username.push(c);
            }
            KeyCode::Backspace => {
                self.new_conversation_username.pop();
            }
            KeyCode::Enter => {
                if self.new_conversation_username.is_empty() {
                    self.show_error_toast("Username cannot be empty");
                    return;
                }

                if let Some(ref mut sdk) = self.sdk {
                    // Resolve username to UUID first
                    let profile =
                        match sdk.get_profile_by_username(self.new_conversation_username.clone()) {
                            Ok(p) => p,
                            Err(e) => {
                                error!("User lookup failed: {e}");
                                self.show_error_toast(friendly_error("Finding user", &e));
                                return;
                            }
                        };

                    match sdk.start_conversation(profile.id, None) {
                        Ok(conv) => {
                            let conv_id = conv.id;
                            let entry = ConversationEntry {
                                conversation: conv,
                                unread_messages_count: 0,
                                last_message: None,
                            };
                            self.conversations.push(entry);
                            self.selected_conversation = self.conversations.len() - 1;
                            self.new_conversation_mode = false;
                            self.new_conversation_username.clear();

                            // Open the conversation directly
                            self.current_conversation = Some(conv_id);
                            self.messages.clear();
                            self.input_clear();
                            self.scroll_pinned_to_bottom = true;
                            self.screen = Screen::Chat;
                        }
                        Err(e) => {
                            error!("Failed to start conversation: {e}");
                            self.show_error_toast(friendly_error("Starting conversation", &e));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_group_conversation_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.group_conversation_mode = false;
                self.group_conversation_input.clear();
                self.group_conversation_usernames.clear();
                self.group_conversation_peer_ids.clear();
            }
            KeyCode::Char(c) => {
                self.group_conversation_input.push(c);
            }
            KeyCode::Backspace => {
                self.group_conversation_input.pop();
            }
            KeyCode::Enter => {
                if !self.group_conversation_input.is_empty() {
                    // Resolve username and add to the group
                    if let Some(ref mut sdk) = self.sdk {
                        let username = self.group_conversation_input.clone();
                        match sdk.get_profile_by_username(username.clone()) {
                            Ok(profile) => {
                                if self.group_conversation_peer_ids.contains(&profile.id) {
                                    self.show_error_toast("User already added");
                                } else {
                                    self.group_conversation_peer_ids.push(profile.id);
                                    self.group_conversation_usernames.push(username);
                                    self.group_conversation_input.clear();
                                }
                            }
                            Err(e) => {
                                self.show_error_toast(friendly_error("Finding user", &e));
                            }
                        }
                    }
                }
            }
            KeyCode::F(5) => {
                // Finalize group creation
                if self.group_conversation_peer_ids.len() < 2 {
                    self.show_error_toast("Add at least 2 members for a group");
                    return;
                }

                if let Some(ref mut sdk) = self.sdk {
                    let peers = self.group_conversation_peer_ids.clone();
                    match sdk.start_group_conversation(peers, None) {
                        Ok(conv) => {
                            let conv_id = conv.id;
                            let entry = ConversationEntry {
                                conversation: conv,
                                unread_messages_count: 0,
                                last_message: None,
                            };
                            self.conversations.push(entry);
                            self.selected_conversation = self.conversations.len() - 1;

                            self.group_conversation_mode = false;
                            self.group_conversation_input.clear();
                            self.group_conversation_usernames.clear();
                            self.group_conversation_peer_ids.clear();

                            // Open the conversation directly
                            self.current_conversation = Some(conv_id);
                            self.messages.clear();
                            self.input_clear();
                            self.scroll_pinned_to_bottom = true;
                            self.screen = Screen::Chat;
                        }
                        Err(e) => {
                            error!("Failed to create group: {e}");
                            self.show_error_toast(friendly_error("Creating group", &e));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // ─── Chat key handling ──────────────────────────────────────────────

    fn handle_chat_key(&mut self, key: KeyEvent) {
        // ── Delete confirmation mode ──
        if let Some(msg_id) = self.confirm_delete {
            match key.code {
                KeyCode::Char('y') => {
                    if let (Some(conv_id), Some(sdk)) = (self.current_conversation, &mut self.sdk) {
                        if let Err(e) = sdk.delete_message(conv_id, msg_id) {
                            error!("Failed to delete message: {e}");
                            self.show_error_toast(friendly_error("Deleting message", &e));
                        } else if let Some(msg) = self.messages.iter_mut().find(|m| m.id == msg_id)
                        {
                            msg.content = "[deleted]".to_string();
                        }
                    }
                    self.confirm_delete = None;
                    self.selected_message = None;
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.confirm_delete = None;
                }
                _ => {}
            }
            return;
        }

        // ── Editing mode ──
        if self.editing_message_id.is_some() {
            match key.code {
                KeyCode::Enter => {
                    if let Some(msg_id) = self.editing_message_id.take() {
                        let new_text = self.message_input.clone();
                        if let (Some(conv_id), Some(sdk)) =
                            (self.current_conversation, &mut self.sdk)
                        {
                            if let Err(e) = sdk.edit_message(conv_id, msg_id, new_text.clone()) {
                                error!("Failed to edit message: {e}");
                                self.show_error_toast(friendly_error("Editing message", &e));
                            } else if let Some(msg) =
                                self.messages.iter_mut().find(|m| m.id == msg_id)
                            {
                                msg.content = new_text;
                            }
                        }
                        self.input_clear();
                        self.original_message_text.clear();
                    }
                }
                KeyCode::Esc => {
                    self.editing_message_id = None;
                    self.input_clear();
                    self.original_message_text.clear();
                }
                KeyCode::Char(c) => {
                    self.input_insert(c);
                }
                KeyCode::Backspace => {
                    self.input_backspace();
                }
                KeyCode::Left => self.input_cursor_left(),
                KeyCode::Right => self.input_cursor_right(),
                _ => {}
            }
            return;
        }

        // ── Message selection mode ──
        if let Some(sel) = self.selected_message {
            match key.code {
                KeyCode::Up => {
                    if sel > 0 {
                        self.selected_message = Some(sel - 1);
                    }
                }
                KeyCode::Down => {
                    if sel + 1 < self.messages.len() {
                        self.selected_message = Some(sel + 1);
                    } else {
                        // Past the last message → exit selection mode, snap to bottom
                        self.selected_message = None;
                        self.scroll_pinned_to_bottom = true;
                    }
                }
                KeyCode::Esc => {
                    self.selected_message = None;
                }
                KeyCode::Char('e') => {
                    if let Some(msg) = self.messages.get(sel) {
                        if Some(msg.sender_id) == self.my_user_id {
                            self.editing_message_id = Some(msg.id);
                            self.original_message_text = msg.content.clone();
                            self.input_set(msg.content.clone());
                            self.selected_message = None;
                        } else {
                            self.show_error_toast("Can only edit your own messages");
                        }
                    }
                }
                KeyCode::Char('d') => {
                    if let Some(msg) = self.messages.get(sel) {
                        if Some(msg.sender_id) == self.my_user_id {
                            self.confirm_delete = Some(msg.id);
                        } else {
                            self.show_error_toast("Can only delete your own messages");
                        }
                    }
                }
                KeyCode::Char('r') => {
                    // Reply to the selected message (own or peer)
                    if let Some(msg) = self.messages.get(sel) {
                        let preview = if msg.content.len() > 50 {
                            format!("{}...", &msg.content[..50])
                        } else {
                            msg.content.clone()
                        };
                        self.reply_to_message = Some((msg.id, preview));
                        self.selected_message = None;
                    }
                }
                KeyCode::Char(c) => {
                    // Any other char exits selection and starts typing
                    self.selected_message = None;
                    self.input_insert(c);
                }
                KeyCode::PageUp => {
                    let jump = 5.min(sel);
                    self.selected_message = Some(sel - jump);
                }
                KeyCode::PageDown => {
                    let jump = 5;
                    let new_sel = sel + jump;
                    if new_sel < self.messages.len() {
                        self.selected_message = Some(new_sel);
                    } else {
                        self.selected_message = Some(self.messages.len().saturating_sub(1));
                    }
                }
                _ => {}
            }
            return;
        }

        // ── Normal input mode ──
        match key.code {
            KeyCode::Esc => {
                // If replying, cancel reply first
                if self.reply_to_message.is_some() {
                    self.reply_to_message = None;
                    return;
                }

                // Refresh conversations list before going back
                if let Some(ref sdk) = self.sdk {
                    match sdk.list_conversations() {
                        Ok(convos) => self.conversations = convos,
                        Err(e) => {
                            warn!("Failed to refresh conversations: {e}");
                        }
                    }
                }
                self.screen = Screen::Conversations;
                self.current_conversation = None;
                self.messages.clear();
                self.selected_message = None;
            }
            KeyCode::Enter => {
                if !self.message_input.is_empty() {
                    let conv_id = self.current_conversation;
                    let text = self.message_input.clone();
                    if let (Some(conv_id), Some(sdk)) = (conv_id, &mut self.sdk) {
                        let result = if let Some((reply_to_id, _)) = self.reply_to_message.take() {
                            sdk.send_reply(conv_id, text, reply_to_id)
                        } else {
                            sdk.send_message(conv_id, text)
                        };
                        match result {
                            Ok(msg) => {
                                self.messages.push(msg);
                                self.input_clear();
                                self.scroll_pinned_to_bottom = true;
                            }
                            Err(e) => {
                                error!("Failed to send message: {e}");
                                self.show_error_toast(friendly_error("Sending message", &e));
                            }
                        }
                    }
                }
            }
            KeyCode::F(3) => {
                self.info_selected_peer = 0;
                self.info_show_fingerprint = false;
                self.info_add_member_mode = false;
                self.info_add_member_input.clear();
                self.screen = Screen::ConversationInfo;
            }
            KeyCode::Char(c) => {
                self.input_insert(c);
                // Send typing indicator with 3-second cooldown
                self.maybe_send_typing_indicator();
            }
            KeyCode::Backspace => {
                self.input_backspace();
            }
            KeyCode::Left => self.input_cursor_left(),
            KeyCode::Right => self.input_cursor_right(),
            KeyCode::Up => {
                // Enter message selection mode at the last message
                if !self.messages.is_empty() {
                    self.selected_message = Some(self.messages.len() - 1);
                    self.scroll_pinned_to_bottom = false;
                }
            }
            KeyCode::Down => {
                // No-op in input mode (already at bottom)
            }
            KeyCode::PageUp => {
                let page = self.messages_area_height.max(1);
                self.scroll_offset = self.scroll_offset.saturating_sub(page);
                self.scroll_pinned_to_bottom = false;
            }
            KeyCode::PageDown => {
                let page = self.messages_area_height.max(1);
                let max_offset = self.total_visual_lines.saturating_sub(self.messages_area_height);
                self.scroll_offset = (self.scroll_offset + page).min(max_offset);
                if self.scroll_offset >= max_offset {
                    self.scroll_pinned_to_bottom = true;
                }
            }
            _ => {}
        }
    }

    /// Send a typing indicator if the 3-second cooldown has expired.
    fn maybe_send_typing_indicator(&mut self) {
        let now = Instant::now();
        let should_send = self
            .last_typing_sent
            .map(|t| now.duration_since(t).as_secs() >= 3)
            .unwrap_or(true);

        if should_send {
            if let (Some(conv_id), Some(sdk)) = (self.current_conversation, &mut self.sdk) {
                if let Err(e) = sdk.send_typing_indicator(conv_id) {
                    warn!("Failed to send typing indicator: {e}");
                }
            }
            self.last_typing_sent = Some(now);
        }
    }

    fn handle_conversation_info_key(&mut self, key: KeyEvent) {
        // ── Add member sub-mode ──
        if self.info_add_member_mode {
            match key.code {
                KeyCode::Esc => {
                    self.info_add_member_mode = false;
                    self.info_add_member_input.clear();
                }
                KeyCode::Char(c) => {
                    self.info_add_member_input.push(c);
                }
                KeyCode::Backspace => {
                    self.info_add_member_input.pop();
                }
                KeyCode::Enter => {
                    if self.info_add_member_input.is_empty() {
                        self.show_error_toast("Username cannot be empty");
                        return;
                    }
                    let conv_id = match self.current_conversation {
                        Some(id) => id,
                        None => return,
                    };
                    let sdk = match self.sdk.as_mut() {
                        Some(s) => s,
                        None => return,
                    };
                    let username = self.info_add_member_input.clone();
                    match sdk.get_profile_by_username(username) {
                        Ok(profile) => {
                            match sdk.add_peer_to_conversation(conv_id, profile.id) {
                                Ok(()) => {
                                    // Refresh conversation data
                                    if let Ok(Some(conv)) = sdk.get_conversation(conv_id) {
                                        if let Some(entry) = self
                                            .conversations
                                            .iter_mut()
                                            .find(|e| e.conversation.id == conv_id)
                                        {
                                            entry.conversation = conv;
                                        }
                                    }
                                    self.show_info_toast("Member added");
                                    self.info_add_member_mode = false;
                                    self.info_add_member_input.clear();
                                }
                                Err(e) => {
                                    self.show_error_toast(friendly_error("Adding member", &e));
                                }
                            }
                        }
                        Err(e) => {
                            self.show_error_toast(friendly_error("Finding user", &e));
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        // ── Normal info navigation ──
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::Chat;
            }
            KeyCode::Up => {
                self.info_selected_peer = self.info_selected_peer.saturating_sub(1);
                self.info_show_fingerprint = false;
            }
            KeyCode::Down => {
                if let Some(entry) = self
                    .current_conversation
                    .and_then(|cid| self.conversations.iter().find(|e| e.conversation.id == cid))
                {
                    let max = entry.conversation.peers.len().saturating_sub(1);
                    if self.info_selected_peer < max {
                        self.info_selected_peer += 1;
                        self.info_show_fingerprint = false;
                    }
                }
            }
            KeyCode::Enter => {
                // Toggle fingerprint display for the selected peer
                self.info_show_fingerprint = !self.info_show_fingerprint;
            }
            KeyCode::Char('a') => {
                self.info_add_member_mode = true;
                self.info_add_member_input.clear();
            }
            _ => {}
        }
    }

    // ─── SDK event polling ──────────────────────────────────────────────

    pub fn poll_sdk_events(&mut self) {
        // Collect events first to avoid double mutable borrow
        let mut events = Vec::new();

        if let Some(rx) = self.event_rx.as_mut() {
            // Non-blocking: drain all available events
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }

        for event in events {
            self.handle_sdk_event(event);
        }

        // Clean up expired typing indicators (older than 5 seconds)
        let now = Instant::now();
        self.typing_indicators
            .retain(|_, (_, timestamp)| now.duration_since(*timestamp).as_secs() < 5);
    }

    fn handle_sdk_event(&mut self, event: SdkEvent) {
        match event {
            SdkEvent::MessageReceived {
                conversation_id,
                msg,
            } => {
                let msg_id = msg.id;

                if self.current_conversation == Some(conversation_id) {
                    self.messages.push(msg);

                    // Mark as read immediately since the conversation is open
                    if let Some(sdk) = &mut self.sdk {
                        if let Err(e) = sdk.mark_as_read(conversation_id, msg_id) {
                            warn!("Failed to mark message as read: {e}");
                        }
                    }
                }

                if let Some(entry) = self
                    .conversations
                    .iter_mut()
                    .find(|e| e.conversation.id == conversation_id)
                {
                    if self.current_conversation != Some(conversation_id) {
                        entry.unread_messages_count += 1;
                    }
                }
            }
            SdkEvent::MessageStatusUpdated {
                conversation_id: _,
                message_id,
                status,
            } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
                    msg.status = status;
                }
            }
            SdkEvent::ConversationCreatedByPeer(conv) => {
                if !self
                    .conversations
                    .iter()
                    .any(|e| e.conversation.id == conv.id)
                {
                    self.conversations.push(ConversationEntry {
                        conversation: conv,
                        unread_messages_count: 0,
                        last_message: None,
                    });
                }
            }
            SdkEvent::ConversationDeleted(conv_id) => {
                self.conversations.retain(|e| e.conversation.id != conv_id);
                if self.current_conversation == Some(conv_id) {
                    self.screen = Screen::Conversations;
                    self.current_conversation = None;
                    self.messages.clear();
                }
            }
            SdkEvent::MessageEdited {
                conversation_id,
                message_id,
                new_content,
            } => {
                if self.current_conversation == Some(conversation_id) {
                    if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
                        msg.content = new_content.to_string();
                    }
                }
            }
            SdkEvent::MessageDeleted {
                conversation_id,
                message_id,
            } => {
                if self.current_conversation == Some(conversation_id) {
                    if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
                        msg.content = "[deleted]".to_string();
                    }
                }
            }
            SdkEvent::TypingIndicator {
                conversation_id,
                peer_id,
            } => {
                // Look up peer name
                let peer_name = self
                    .conversations
                    .iter()
                    .find(|e| e.conversation.id == conversation_id)
                    .and_then(|e| e.conversation.peers.iter().find(|p| p.id == peer_id))
                    .map(|p| p.name())
                    .unwrap_or_else(|| "Someone".into());

                self.typing_indicators
                    .insert(conversation_id, (peer_name, Instant::now()));
            }
            SdkEvent::ConnectionStateChanged(state) => {
                self.show_info_toast(format!("Connection: {:?}", state));
            }
            SdkEvent::PeerAddedToConversation {
                conversation_id,
                peer_id,
            } => {
                // Refresh the conversation entry to get updated peers list
                if let Some(ref sdk) = self.sdk {
                    if let Ok(Some(conv)) = sdk.get_conversation(conversation_id) {
                        if let Some(entry) = self
                            .conversations
                            .iter_mut()
                            .find(|e| e.conversation.id == conversation_id)
                        {
                            entry.conversation = conv;
                        }
                    }
                }
                let name = peer_id.to_string()[..8].to_string();
                self.show_info_toast(format!("New member added: {}", name));
            }
        }
    }
}
