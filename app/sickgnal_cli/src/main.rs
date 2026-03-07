use sickgnal_sdk::{client::SdkClient, core::chat::client::Event};
/*use clap::Parser;
use sickgnal_cli::cli;
use tokio::net::TcpStream;
use slint::{Model, ModelRc, VecModel};*/
use std::{fmt::Debug, path::PathBuf, rc::Rc};
slint::include_modules!();

#[tokio::main]
async fn main() {

    let mut sdk = SdkClient::new("test".into(), PathBuf::new(), "1234", "127.0.0.1").await.unwrap_or_else(|e| {
            eprintln!("Erreur SDK : {}", e);
            panic!("Impossible de continuer.");
        });
    
    let ui = AppWindow::new().expect("Failed to load UI");
    let ui_handle = ui.as_weak();

    // 3. Lancer la tâche d'écoute (Background Task)
    tokio::spawn(async move {
        // On boucle sur le receiver du SDK
        loop {
            // On attend le prochain message
            match sdk.event_rx.recv().await {
                Ok(event) => {
                    let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                        handle_sdk_event(ui, event);
                    });
                }
                
                Err(e) => {
                    println!("{:?}", e);
                }
            }
        }
    });

    ui.run().unwrap();

}

fn handle_sdk_event(ui: AppWindow, event: Event) {
    match event {
        Event::NewMessage(id, msg) => todo!(),
        Event::MessageStatusUpdate(uuid, message_status) => todo!(),
        Event::ConversationCreated(conversation) => todo!(),
        Event::ConversationDeleted(uuid) => todo!(),
        Event::TypingIndicator(uuid) => todo!(),
        Event::ConnectionStateChanged(connection_state) => todo!(),
    }
}

 /*   // Create sample messages for conversation 1
    let msg1 = MessageData {
        text: "Salut depuis Rust !".into(),
        time: "10:00".into(),
        status: "read".into(),
        is_me: false,
    };

    let msg2 = MessageData {
        text: "Salut ! Le protocole est prêt ?".into(),
        time: "15:12".into(),
        status: "read".into(),
        is_me: false,
    };

    let msg3 = MessageData {
        text: "Lorem ipsum dolor sit amet consectetur adipiscing elit. Quisque faucibus ex sapien vitae pellentesque sem placerat. In id cursus mi pretium tellus duis convallis. Tempus leo eu aenean sed diam urna tempor. Pulvinar vivamus fringilla lacus nec metus bibendum egestas. Iaculis massa nisl malesuada lacinia integer nunc posuere. Ut hendrerit semper vel class aptent taciti sociosqu. Ad litora torquent per conubia nostra inceptos himenaeos.\n\nLorem ipsum dolor sit amet consectetur adipiscing elit. Quisque faucibus ex sapien vitae pellentesque sem placerat. In id cursus mi pretium tellus duis convallis. Tempus leo eu aenean sed diam urna tempor. Pulvinar vivamus fringilla lacus nec metus bibendum egestas. Iaculis massa nisl malesuada lacinia integer nunc posuere. Ut hendrerit semper vel class aptent taciti sociosqu. Ad litora torquent per conubia nostra inceptos himenaeos.\nLorem ipsum dolor sit amet consectetur adipiscing elit. Quisque faucibus ex sapien vitae pellentesque sem placerat. In id cursus mi pretium tellus duis convallis. Tempus leo eu aenean sed diam urna tempor. Pulvinar vivamus fringilla lacus nec metus bibendum egestas. Iaculis massa nisl malesuada lacinia integer nunc posuere. Ut hendrerit semper vel class aptent taciti sociosqu. Ad litora torquent per conubia nostra inceptos himenaeos.".into(),
        time: "15:12".into(),
        status: "read".into(),
        is_me: false,
    };

    let msg4 = MessageData {
        text: "Presque, je finalise l'UI en Slint.".into(),
        time: "15:15".into(),
        status: "read".into(),
        is_me: true,
    };

    let msg5 = MessageData {
        text: "Super ! Ça avance bien alors.".into(),
        time: "15:16".into(),
        status: "sent".into(),
        is_me: true,
    };

    let msg6 = MessageData {
        text: "Oui, j'utilise les nouveaux composants.".into(),
        time: "15:17".into(),
        status: "sending".into(),
        is_me: true,
    };

    let messages1 = vec![msg1, msg2, msg3, msg4, msg5, msg6];

    // Create sample messages for conversation 2
    let msg7 = MessageData {
        text: "Bonjour ! Comment vas-tu ?".into(),
        time: "09:30".into(),
        status: "read".into(),
        is_me: false,
    };

    let msg8 = MessageData {
        text: "Très bien, merci ! Et toi ?".into(),
        time: "09:32".into(),
        status: "read".into(),
        is_me: true,
    };

    let messages2 = vec![msg7, msg8];

    // Create sample messages for conversation 3
    let msg9 = MessageData {
        text: "On se voit demain ?".into(),
        time: "14:20".into(),
        status: "read".into(),
        is_me: false,
    };

    let messages3 = vec![msg9];

    // Create conversations
    let conv1 = Conversation {
        id: 0,
        name: "Alice".into(),
        last_message: "Oui, j'utilise les nouveaux composants.".into(),
        last_message_time: "15:17".into(),
        unread_count: 0,
        is_typing: false,
        messages: ModelRc::new(VecModel::from(messages1)),
    };

    let conv2 = Conversation {
        id: 2,
        name: "Bob".into(),
        last_message: "Très bien, merci ! Et toi ?".into(),
        last_message_time: "09:32".into(),
        unread_count: 2,
        is_typing: false,
        messages: ModelRc::new(VecModel::from(messages2)),
    };

    let conv3 = Conversation {
        id: 3,
        name: "Charlie".into(),
        last_message: "On se voit demain ?".into(),
        last_message_time: "14:20".into(),
        unread_count: 1,
        is_typing: false,
        messages: ModelRc::new(VecModel::from(messages3)),
    };

    let conversations = Rc::new(VecModel::from(vec![conv1, conv2, conv3]));

    // Create UI
    let ui = AppWindow::new().unwrap();

    // Set conversations
    ui.global::<Chat>().set_chats(ModelRc::from(conversations.clone()));
    ui.global::<Chat>().set_active_chat_id(0);

    // Set up callbacks
    let ui_weak = ui.as_weak();
    ui.global::<Chat>().on_switch_conversation(move |id| {
        if let Some(ui) = ui_weak.upgrade() {
            ui.global::<Chat>().set_active_chat_id(id);
            println!("Switched to conversation: {}", id);
        }
    });

    let ui_weak = ui.as_weak();
    let conversations_clone = conversations.clone();
    ui.global::<Chat>().on_delete_conversation(move |index| {
        if let Some(ui) = ui_weak.upgrade() {
            let idx = index as usize;
            if idx < conversations_clone.row_count() {
                conversations_clone.remove(idx);
                println!("Deleted conversation at index: {}", index);

                // Adjust active index after deletion
                let active = ui.global::<Chat>().get_active_chat_id();
                let count = conversations_clone.row_count() as i32;
                if count == 0 {
                    ui.global::<Chat>().set_active_chat_id(-1);
                } else if active >= index {
                    // Shift active index down if needed, clamp to valid range
                    ui.global::<Chat>().set_active_chat_id((active - 1).max(0));
                }
            }
        }
    });

    let ui_weak = ui.as_weak();
    ui.global::<Chat>().on_send_message(move |message| {
        if let Some(ui) = ui_weak.upgrade() {
            let active_id = ui.global::<Chat>().get_active_chat_id();
            println!("Send message to conversation {}: {}", active_id, message);
            
            // TODO: Implement actual message sending logic
            // For now, just log the message
        }
    });

    // Exemple : simuler un événement "typing" sur la conversation active
    let ui_weak_typing = ui.as_weak();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));

            let ui_handle = ui_weak_typing.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle.upgrade() {
                    let chats = ui.global::<Chat>().get_chats();
                    let active_idx = ui.global::<Chat>().get_active_chat_id() as usize;
                    if let Some(mut conv) = chats.row_data(active_idx) {
                        conv.is_typing = true;
                        chats.set_row_data(active_idx, conv);
                    }
                }
            }).unwrap();
        }
    });

    ui.run().unwrap(); */
