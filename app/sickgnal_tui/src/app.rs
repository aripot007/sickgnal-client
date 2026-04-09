use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use sickgnal_core::chat::client::ChatEvent as SdkEvent;
use sickgnal_core::chat::storage::Message;
use sickgnal_sdk::TlsConfig;
use sickgnal_sdk::client::SyncBridge;
use sickgnal_sdk::dto::ConversationEntry;
use tokio::sync::mpsc;
use tracing::error;
use uuid::Uuid;

use sickgnal_sdk::account::{Profile, ProfileManager};

// ─── Screens ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    ProfileSelect,
    Auth,
    Conversations,
    Chat,
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
    pub status_message: Option<String>,

    // Chat state
    pub current_conversation: Option<Uuid>,
    pub messages: Vec<Message>,
    pub message_input: String,
    pub scroll_offset: u16,
    pub my_user_id: Option<Uuid>,

    // SDK bridge
    pub sdk: Option<SyncBridge>,
    pub event_rx: Option<mpsc::Receiver<SdkEvent>>,

    // Storage dir
    pub data_dir: PathBuf,
}

impl App {
    pub fn new(data_dir: PathBuf) -> Self {
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
            status_message: None,

            current_conversation: None,
            messages: Vec::new(),
            message_input: String::new(),
            scroll_offset: 0,
            my_user_id: None,

            sdk: None,
            event_rx: None,

            data_dir: PathBuf::new(),
        }
    }

    /// Select a profile and transition to the auth screen.
    fn select_profile(&mut self, profile_name: String) {
        let dir = match self.profile_manager.profile_dir(&profile_name) {
            Ok(d) => d,
            Err(e) => {
                self.profile_error = Some(format!("Error: {e}"));
                return;
            }
        };
        self.data_dir = dir;

        // Check if the profile has an account file
        let account_file = sickgnal_sdk::account::AccountFile::new(self.data_dir.clone()).ok();
        let has_account = account_file.as_ref().is_some_and(|af| af.exists());

        if has_account {
            self.auth_mode = AuthMode::SignIn;
            self.username = account_file
                .and_then(|af| af.username().ok())
                .unwrap_or_default();
            self.auth_field = AuthField::Password;
        } else {
            self.auth_mode = AuthMode::SignUp;
            self.username = String::new();
            self.auth_field = AuthField::Username;
        }

        self.password.clear();
        self.confirm_password.clear();
        self.auth_error = None;
        self.screen = Screen::Auth;
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.screen {
            Screen::ProfileSelect => self.handle_profile_key(key),
            Screen::Auth => self.handle_auth_key(key),
            Screen::Conversations => self.handle_conversations_key(key),
            Screen::Chat => self.handle_chat_key(key),
        }
    }

    // ─── Profile selection key handling ─────────────────────────────

    fn handle_profile_key(&mut self, key: KeyEvent) {
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
                    let _ = self.profile_manager.delete_profile(&name);
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

        // Connect to server via SDK
        let existing = self.auth_mode == AuthMode::SignIn;
        match SyncBridge::connect(
            self.username.clone(),
            &self.password,
            self.data_dir.clone(),
            existing,
            "127.0.0.1:8080",
            &TlsConfig::None,
        ) {
            Ok((bridge, event_rx)) => {
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
                self.status_message = Some("Connected".into());

                // Refresh profiles list so profile selection is up to date
                self.profiles = self.profile_manager.list_profiles().unwrap_or_default();
            }
            Err(e) => {
                // If sign-up failed, clean up the local account file so user can retry
                if self.auth_mode == AuthMode::SignUp {
                    let _ = account_file.delete();
                }

                let msg = format!("Connection failed: {e}");
                self.auth_error = Some(msg);
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

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('n') => {
                self.new_conversation_mode = true;
                self.new_conversation_username.clear();
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
                    self.current_conversation = Some(entry.conversation.id);

                    // Load messages
                    if let Some(ref sdk) = self.sdk {
                        match sdk.get_messages(entry.conversation.id) {
                            Ok(msgs) => self.messages = msgs,
                            Err(e) => {
                                error!("Failed to load messages: {e}");
                                self.status_message = Some(format!("Failed to load messages: {e}"));
                            }
                        }
                    }

                    self.message_input.clear();
                    self.scroll_offset = 0;
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
                            self.status_message = Some(format!("Delete failed: {e}"));
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
                self.status_message = None;
            }
            KeyCode::Char(c) => {
                self.new_conversation_username.push(c);
                self.status_message = None;
            }
            KeyCode::Backspace => {
                self.new_conversation_username.pop();
                self.status_message = None;
            }
            KeyCode::Enter => {
                if self.new_conversation_username.is_empty() {
                    self.status_message = Some("Username cannot be empty".into());
                    return;
                }

                if let Some(ref mut sdk) = self.sdk {
                    // Resolve username to UUID first
                    let profile =
                        match sdk.get_profile_by_username(self.new_conversation_username.clone()) {
                            Ok(p) => p,
                            Err(e) => {
                                self.status_message = Some(format!("User not found: {e}"));
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
                            self.message_input.clear();
                            self.scroll_offset = 0;
                            self.screen = Screen::Chat;
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Error: {e}"));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // ─── Chat key handling ──────────────────────────────────────────────

    fn handle_chat_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                // Refresh conversations list before going back
                if let Some(ref sdk) = self.sdk {
                    match sdk.list_conversations() {
                        Ok(convos) => self.conversations = convos,
                        Err(e) => {
                            error!("Failed to refresh conversations: {e}");
                        }
                    }
                }
                self.screen = Screen::Conversations;
                self.current_conversation = None;
                self.messages.clear();
            }
            KeyCode::Enter => {
                if !self.message_input.is_empty() {
                    let conv_id = self.current_conversation;
                    let text = self.message_input.clone();
                    if let (Some(conv_id), Some(sdk)) = (conv_id, &mut self.sdk) {
                        match sdk.send_message(conv_id, text) {
                            Ok(msg) => {
                                self.messages.push(msg);
                                self.message_input.clear();
                                self.scroll_offset = 0;
                            }
                            Err(e) => {
                                error!("Failed to send message: {e}");
                                self.status_message = Some(format!("Send failed: {e}"));
                            }
                        }
                    }
                }
            }
            KeyCode::Char(c) => {
                self.message_input.push(c);
            }
            KeyCode::Backspace => {
                self.message_input.pop();
            }
            KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
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
    }

    fn handle_sdk_event(&mut self, event: SdkEvent) {
        match event {
            SdkEvent::MessageReceived {
                conversation_id,
                msg,
            } => {
                if self.current_conversation == Some(conversation_id) {
                    self.messages.push(msg);
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
                conversation_id: _,
                peer_id: _,
            } => {
                // TODO: show typing indicator in UI
            }
            SdkEvent::ConnectionStateChanged(state) => {
                self.status_message = Some(format!("Connection: {:?}", state));
            }
        }
    }
}
