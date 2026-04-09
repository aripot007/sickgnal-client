//! Unit tests for implementations of the [`StorageBackend`] trait
//!

/// A test suite for a [`StorageBackend`] implementation
///
/// Users of this library should invoke this macro in their test modules
/// to ensure their implementation upholds the trait's contract.
///
/// Takes an expression that constructs a fresh instance of the implementation.
/// This instance should not contain any values from previous tests, but **must**
/// have an account set (via [`E2EStorageBackend::set_account`]) because some
/// methods rely on knowing the current user's id.
///
/// # Example
///
/// ```
/// use sickgnal_core::chat::storage::StorageBackend;
///
/// pub struct MyStorageImpl {};
///
/// impl StorageBackend for MyStorageImpl {
///     // ...
/// }
///
/// #[cfg(test)]
/// mod tests {
///     use super::*;
///     use sickgnal_core::test_chat_storage_backend;
///
///     test_chat_storage_backend!(MyStorageImpl::new());
/// }
/// ```
#[macro_export]
macro_rules! test_chat_storage_backend {
    ($setup:expr) => {
        #[test]
        fn test_conversation_lifecycle() {
            use ::sickgnal_core::chat::storage::{ConversationInfo, StorageBackend};
            use ::sickgnal_core::e2e::keys::E2EStorageBackend;
            use ::sickgnal_core::e2e::peer::Peer;
            use ::uuid::Uuid;

            let mut backend = $setup;

            let peer = Peer {
                id: Uuid::new_v4(),
                username: Some("Alice".into()),
                fingerprint: None,
            };
            backend.save_peer(&peer).expect("error saving peer");

            let conv_id = Uuid::new_v4();
            let info = ConversationInfo {
                id: conv_id,
                custom_title: None,
            };

            // Conversation should not exist yet
            assert!(
                !backend
                    .conversation_exists(&conv_id)
                    .expect("error checking conversation exists")
            );

            // Create conversation
            backend
                .create_conversation(&info, peer.id)
                .expect("error creating conversation");

            // Conversation should now exist
            assert!(
                backend
                    .conversation_exists(&conv_id)
                    .expect("error checking conversation exists")
            );

            // Check conversation_has_peer
            assert!(
                backend
                    .conversation_has_peer(&conv_id, &peer.id)
                    .expect("error checking conversation has peer")
            );
            assert!(
                !backend
                    .conversation_has_peer(&conv_id, &Uuid::new_v4())
                    .expect("error checking conversation has peer")
            );

            // Get conversation info
            let stored_info = backend
                .get_conversation_info(&conv_id)
                .expect("error getting conversation info")
                .expect("conversation info not found");
            assert_eq!(stored_info, info);

            // Update conversation info
            let updated_info = ConversationInfo {
                id: conv_id,
                custom_title: Some("My Chat".into()),
            };
            backend
                .update_conversation_info(&updated_info)
                .expect("error updating conversation info");

            let stored_updated = backend
                .get_conversation_info(&conv_id)
                .expect("error getting conversation info")
                .expect("conversation info not found");
            assert_eq!(stored_updated, updated_info);

            // Get full conversation
            let conversation = backend
                .get_conversation(&conv_id)
                .expect("error getting conversation")
                .expect("conversation not found");
            assert_eq!(conversation.id, conv_id);
            assert_eq!(conversation.title, "My Chat");

            // Get conversation peers
            let peers = backend
                .get_conversation_peers(&conv_id)
                .expect("error getting conversation peers")
                .expect("conversation peers not found");
            assert_eq!(peers.len(), 1);
            assert_eq!(peers[0].id, peer.id);
            assert_eq!(peers[0].username, Some("Alice".into()));
        }

        #[test]
        fn test_group_conversation() {
            use ::sickgnal_core::chat::storage::{ConversationInfo, StorageBackend};
            use ::sickgnal_core::e2e::keys::E2EStorageBackend;
            use ::sickgnal_core::e2e::peer::Peer;
            use ::uuid::Uuid;

            let mut backend = $setup;

            let peers: Vec<Peer> = (0..3)
                .map(|i| Peer {
                    id: Uuid::new_v4(),
                    username: Some(format!("Peer{}", i)),
                    fingerprint: None,
                })
                .collect();

            for p in &peers {
                backend.save_peer(p).expect("error saving peer");
            }

            let conv_id = Uuid::new_v4();
            let info = ConversationInfo {
                id: conv_id,
                custom_title: Some("Group Chat".into()),
            };

            let peer_ids: Vec<Uuid> = peers.iter().map(|p| p.id).collect();
            backend
                .create_group_conversation(&info, &peer_ids)
                .expect("error creating group conversation");

            assert!(
                backend
                    .conversation_exists(&conv_id)
                    .expect("error checking conversation exists")
            );

            let stored_peers = backend
                .get_conversation_peers(&conv_id)
                .expect("error getting conversation peers")
                .expect("conversation peers not found");
            assert_eq!(stored_peers.len(), 3);

            for p in &peers {
                assert!(
                    backend
                        .conversation_has_peer(&conv_id, &p.id)
                        .expect("error checking conversation has peer"),
                    "conversation should contain peer {}",
                    p.id
                );
            }

            // A random peer should not be in the conversation
            assert!(
                !backend
                    .conversation_has_peer(&conv_id, &Uuid::new_v4())
                    .expect("error checking conversation has peer")
            );
        }

        #[test]
        fn test_message_lifecycle() {
            use ::sickgnal_core::chat::storage::{
                ConversationInfo, Message, MessageStatus, StorageBackend,
            };
            use ::sickgnal_core::e2e::keys::E2EStorageBackend;
            use ::sickgnal_core::e2e::peer::Peer;
            use ::uuid::Uuid;

            let mut backend = $setup;

            let peer = Peer {
                id: Uuid::new_v4(),
                username: Some("Bob".into()),
                fingerprint: None,
            };
            backend.save_peer(&peer).expect("error saving peer");

            let conv_id = Uuid::new_v4();
            let info = ConversationInfo {
                id: conv_id,
                custom_title: None,
            };
            backend
                .create_conversation(&info, peer.id)
                .expect("error creating conversation");

            let msg = Message {
                id: Uuid::new_v4(),
                conversation_id: conv_id,
                sender_id: peer.id,
                content: "Hello, world!".into(),
                issued_at: ::chrono::Utc::now(),
                status: MessageStatus::Delivered,
                reply_to_id: None,
            };

            // Message should not exist yet
            assert!(
                backend
                    .get_message(&conv_id, &msg.id)
                    .expect("error getting message")
                    .is_none()
            );

            // Save message
            backend.save_message(&msg).expect("error saving message");

            // Retrieve and verify
            let stored = backend
                .get_message(&conv_id, &msg.id)
                .expect("error getting message")
                .expect("message not found");
            assert_eq!(stored.id, msg.id);
            assert_eq!(stored.conversation_id, msg.conversation_id);
            assert_eq!(stored.sender_id, msg.sender_id);
            assert_eq!(stored.content, msg.content);
            assert_eq!(stored.status, msg.status);
            assert_eq!(stored.reply_to_id, msg.reply_to_id);

            // Delete message
            backend
                .delete_message(&conv_id, &msg.id)
                .expect("error deleting message");

            assert!(
                backend
                    .get_message(&conv_id, &msg.id)
                    .expect("error getting message")
                    .is_none()
            );
        }

        #[test]
        fn test_update_message_status() {
            use ::sickgnal_core::chat::storage::{
                ConversationInfo, Message, MessageStatus, StorageBackend,
            };
            use ::sickgnal_core::e2e::keys::E2EStorageBackend;
            use ::sickgnal_core::e2e::peer::Peer;
            use ::uuid::Uuid;

            let mut backend = $setup;

            let peer = Peer {
                id: Uuid::new_v4(),
                username: Some("Carol".into()),
                fingerprint: None,
            };
            backend.save_peer(&peer).expect("error saving peer");

            let conv_id = Uuid::new_v4();
            backend
                .create_conversation(
                    &ConversationInfo {
                        id: conv_id,
                        custom_title: None,
                    },
                    peer.id,
                )
                .expect("error creating conversation");

            let msg = Message {
                id: Uuid::new_v4(),
                conversation_id: conv_id,
                sender_id: peer.id,
                content: "Test message".into(),
                issued_at: ::chrono::Utc::now(),
                status: MessageStatus::Sending,
                reply_to_id: None,
            };

            backend.save_message(&msg).expect("error saving message");

            // Update status
            backend
                .update_message_status(&conv_id, [msg.id], MessageStatus::Delivered)
                .expect("error updating message status");

            let stored = backend
                .get_message(&conv_id, &msg.id)
                .expect("error getting message")
                .expect("message not found");
            assert_eq!(stored.status, MessageStatus::Delivered);
            // Other fields should be unchanged
            assert_eq!(stored.id, msg.id);
            assert_eq!(stored.content, msg.content);
        }

        #[test]
        fn test_mark_conversation_as_read() {
            use ::sickgnal_core::chat::storage::{
                ConversationInfo, Message, MessageStatus, StorageBackend,
            };
            use ::sickgnal_core::e2e::keys::E2EStorageBackend;
            use ::sickgnal_core::e2e::peer::Peer;
            use ::uuid::Uuid;

            let mut backend = $setup;

            // Load our account to get our user id
            let account = backend
                .load_account()
                .expect("error loading account")
                .expect("account not found - setup must call set_account()");
            let my_id = account.id;

            // Also save ourselves as a peer so messages reference a valid sender
            backend
                .save_peer(&Peer {
                    id: my_id,
                    username: Some(account.username.clone()),
                    fingerprint: None,
                })
                .expect("error saving self as peer");

            let other_peer = Peer {
                id: Uuid::new_v4(),
                username: Some("Dave".into()),
                fingerprint: None,
            };
            backend.save_peer(&other_peer).expect("error saving peer");

            let conv_id = Uuid::new_v4();
            backend
                .create_conversation(
                    &ConversationInfo {
                        id: conv_id,
                        custom_title: None,
                    },
                    other_peer.id,
                )
                .expect("error creating conversation");

            let now = ::chrono::Utc::now();

            // Messages from the other peer (should be marked as read)
            let peer_msg_1 = Message {
                id: Uuid::new_v4(),
                conversation_id: conv_id,
                sender_id: other_peer.id,
                content: "Hey!".into(),
                issued_at: now,
                status: MessageStatus::Delivered,
                reply_to_id: None,
            };
            let peer_msg_2 = Message {
                id: Uuid::new_v4(),
                conversation_id: conv_id,
                sender_id: other_peer.id,
                content: "How are you?".into(),
                issued_at: now,
                status: MessageStatus::Delivered,
                reply_to_id: None,
            };

            // Messages from ourselves (should NOT be marked as read)
            let my_msg_1 = Message {
                id: Uuid::new_v4(),
                conversation_id: conv_id,
                sender_id: my_id,
                content: "Hi there!".into(),
                issued_at: now,
                status: MessageStatus::Delivered,
                reply_to_id: None,
            };
            let my_msg_2 = Message {
                id: Uuid::new_v4(),
                conversation_id: conv_id,
                sender_id: my_id,
                content: "I'm good".into(),
                issued_at: now,
                status: MessageStatus::Delivered,
                reply_to_id: None,
            };

            backend
                .save_message(&peer_msg_1)
                .expect("error saving message");
            backend
                .save_message(&peer_msg_2)
                .expect("error saving message");
            backend
                .save_message(&my_msg_1)
                .expect("error saving message");
            backend
                .save_message(&my_msg_2)
                .expect("error saving message");

            // get_received_unread_messages should return only the peer's messages
            let unread = backend
                .get_received_unread_messages(&conv_id)
                .expect("error getting unread messages")
                .expect("conversation should exist");
            assert_eq!(unread.len(), 2, "should have 2 unread messages from peer");
            assert!(unread.contains(&peer_msg_1.id));
            assert!(unread.contains(&peer_msg_2.id));

            // Mark conversation as read
            backend
                .mark_conversation_as_read(&conv_id)
                .expect("error marking conversation as read");

            // Peer's messages should now be Read
            let stored_peer_1 = backend
                .get_message(&conv_id, &peer_msg_1.id)
                .expect("error getting message")
                .expect("message not found");
            assert_eq!(
                stored_peer_1.status,
                MessageStatus::Read,
                "peer message 1 should be marked as read"
            );
            let stored_peer_2 = backend
                .get_message(&conv_id, &peer_msg_2.id)
                .expect("error getting message")
                .expect("message not found");
            assert_eq!(
                stored_peer_2.status,
                MessageStatus::Read,
                "peer message 2 should be marked as read"
            );

            // Our messages should still be Delivered
            let stored_my_1 = backend
                .get_message(&conv_id, &my_msg_1.id)
                .expect("error getting message")
                .expect("message not found");
            assert_eq!(
                stored_my_1.status,
                MessageStatus::Delivered,
                "our message 1 should still be delivered"
            );
            let stored_my_2 = backend
                .get_message(&conv_id, &my_msg_2.id)
                .expect("error getting message")
                .expect("message not found");
            assert_eq!(
                stored_my_2.status,
                MessageStatus::Delivered,
                "our message 2 should still be delivered"
            );

            // get_received_unread_messages should now return empty
            let unread_after = backend
                .get_received_unread_messages(&conv_id)
                .expect("error getting unread messages")
                .expect("conversation should exist");
            assert!(
                unread_after.is_empty(),
                "should have no unread messages after marking as read"
            );
        }

        #[test]
        fn test_conversation_not_found() {
            use ::sickgnal_core::chat::storage::StorageBackend;
            use ::uuid::Uuid;

            let mut backend = $setup;
            let fake_id = Uuid::new_v4();

            assert!(
                !backend
                    .conversation_exists(&fake_id)
                    .expect("error checking conversation exists")
            );

            assert!(
                backend
                    .get_conversation_info(&fake_id)
                    .expect("error getting conversation info")
                    .is_none()
            );

            assert!(
                backend
                    .get_conversation_peers(&fake_id)
                    .expect("error getting conversation peers")
                    .is_none()
            );

            assert!(
                backend
                    .get_received_unread_messages(&fake_id)
                    .expect("error getting unread messages")
                    .is_none()
            );
        }

        #[test]
        fn test_message_not_found() {
            use ::sickgnal_core::chat::storage::StorageBackend;
            use ::uuid::Uuid;

            let backend = $setup;
            let fake_conv_id = Uuid::new_v4();
            let fake_msg_id = Uuid::new_v4();

            assert!(
                backend
                    .get_message(&fake_conv_id, &fake_msg_id)
                    .expect("error getting message")
                    .is_none()
            );
        }
    };
}
