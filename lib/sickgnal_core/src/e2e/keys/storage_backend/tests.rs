//! Unit tests for implementations of the [`E2EStorageBackend`] trait
//!

/// A test suite for a [`E2EStorageBackend`] implementation
///
/// Users of this library should invoke this macro in their test modules
/// to ensure their implementation upholds the trait's contract.
///
/// Takes an expression that constructs a fresh instance of the implementation. This instance
/// should not contain any values from previous tests.
///
/// # Exemple
///
/// ```
/// use sickgnal_core::e2e::keys::storage_backend::E2EStorageBackend;
///
/// pub struct MyStorageImpl {};
///
/// impl E2EStorageBackend for MyStorageImpl {
///     // ...
/// }
///
/// #[cfg(test)]
/// mod tests {
///     use super::*;
///     use sickgnal_core::e2e::keys::storage_backend::tests::test_e2e_storage_backend;
///     use rand::random::OsRng;   
///
///     test_e2e_storage_backend!(MyStorageImpl::new(), OsRng);
/// }
/// ```
#[macro_export]
macro_rules! test_e2e_storage_backend {
    ($setup:expr, $rng:expr) => {
        use ::sickgnal_core::e2e::keys::E2EStorageBackend;
        use ::uuid::Uuid;

        #[test]
        fn test_e2e_backend_clone() {
            let mut backend = $setup;

            let key = [1; 32];
            let key_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();

            assert_eq!(
                None,
                backend
                    .session_key(user_id, key_id)
                    .expect("error getting session key")
            );

            let mut backend_clone = backend.clone();

            backend
                .add_session_key(user_id, key_id, key.clone())
                .expect("error setting session key");

            assert_eq!(
                Some(key),
                backend
                    .session_key(user_id, key_id)
                    .expect("error getting session key")
            );
            assert_eq!(
                Some(key),
                backend_clone
                    .session_key(user_id, key_id)
                    .expect("error getting session key")
            );

            backend_clone
                .delete_session_key(user_id, key_id)
                .expect("error deleting session key");

            assert_eq!(
                None,
                backend
                    .session_key(user_id, key_id)
                    .expect("error getting session key")
            );
            assert_eq!(
                None,
                backend_clone
                    .session_key(user_id, key_id)
                    .expect("error getting session key")
            );
        }

        #[test]
        fn test_peer() {
            use ::sickgnal_core::e2e::peer::Peer;
            use ::uuid::Uuid;

            let mut backend = $setup;

            let peer = Peer {
                id: Uuid::new_v4(),
                username: Some("Bob".into()).into(),
                fingerprint: Some("0000000000000000000000000000000000000000".into()),
            };

            assert!(
                backend
                    .peer(&peer.id)
                    .expect("error getting peer")
                    .is_none()
            );

            backend.save_peer(&peer).expect("error saving peer");

            let stored_peer = backend
                .peer(&peer.id)
                .expect("error getting peer")
                .expect("the peer was not stored");

            assert_eq!(stored_peer, peer);

            backend.delete_peer(&peer.id).expect("error deleting peer");

            assert!(
                backend
                    .peer(&peer.id)
                    .expect("error getting peer")
                    .is_none()
            );

            // Try some peer without some fields
            backend
                .save_peer(&Peer {
                    id: Uuid::new_v4(),
                    username: None,
                    fingerprint: None,
                })
                .expect("error saving peer with only id");

            backend
                .save_peer(&Peer {
                    id: Uuid::new_v4(),
                    username: None,
                    fingerprint: Some("0000000000000000000000000000000000000000".into()),
                })
                .expect("error saving peer without username");

            backend
                .save_peer(&Peer {
                    id: Uuid::new_v4(),
                    username: Some("Bob".into()).into(),
                    fingerprint: None,
                })
                .expect("error saving peer without fingerprint");
        }

        #[test]
        fn test_identity_keypair() {
            use ::sickgnal_core::e2e::keys::IdentityKeyPair;

            let mut backend = $setup;

            assert!(backend.identity_keypair().is_err());
            assert!(
                backend
                    .identity_keypair_opt()
                    .expect("error getting identity keypair")
                    .is_none()
            );

            let keypair: IdentityKeyPair = IdentityKeyPair::new_from_rng($rng);

            backend
                .set_identity_keypair(keypair)
                .expect("error setting identity keypair");

            assert!(backend.identity_keypair().is_ok());
            assert!(
                backend
                    .identity_keypair_opt()
                    .expect("error getting identity keypair")
                    .is_some()
            );

            backend
                .clear_identity_keypair()
                .expect("error clearing identity keypair");

            assert!(backend.identity_keypair().is_err());
            assert!(
                backend
                    .identity_keypair_opt()
                    .expect("error getting identity keypair")
                    .is_none()
            );
        }

        #[test]
        fn test_midterm_key() {
            use ::sickgnal_core::e2e::keys::X25519Secret;

            let mut backend = $setup;

            assert!(backend.midterm_key().is_err());
            assert!(
                backend
                    .midterm_key_opt()
                    .expect("error getting midterm key")
                    .is_none()
            );

            let key: X25519Secret = X25519Secret::random_from_rng($rng);

            backend
                .set_midterm_key(key)
                .expect("error setting midterm key");

            assert!(backend.midterm_key().is_ok());
            assert!(
                backend
                    .midterm_key_opt()
                    .expect("error getting midterm key")
                    .is_some()
            );

            backend
                .clear_midterm_key()
                .expect("error clearing midterm key");

            assert!(backend.midterm_key().is_err());
            assert!(
                backend
                    .midterm_key_opt()
                    .expect("error getting midterm key")
                    .is_none()
            );
        }

        #[test]
        fn test_ephemeral_key() {
            use ::sickgnal_core::e2e::keys::EphemeralSecretKey;

            let mut backend = $setup;

            assert_eq!(
                0,
                backend
                    .available_ephemeral_keys()
                    .expect("error getting available ephemeral keys")
                    .len()
            );

            let key = EphemeralSecretKey::new_from_rng($rng);
            let id = key.id;

            backend
                .save_ephemeral_key(key.clone())
                .expect("error saving ephemeral key");

            assert_eq!(
                1,
                backend
                    .available_ephemeral_keys()
                    .expect("error getting available ephemeral keys")
                    .len()
            );
            assert!(
                backend
                    .ephemeral_key(&id)
                    .expect("error getting ephemeral key")
                    .is_some()
            );

            backend
                .delete_ephemeral_key(id)
                .expect("error deleting ephemeral key");

            assert_eq!(
                0,
                backend
                    .available_ephemeral_keys()
                    .expect("error getting available ephemeral keys")
                    .len()
            );
            assert!(
                backend
                    .ephemeral_key(&id)
                    .expect("error getting ephemeral key")
                    .is_none()
            );
            assert!(
                backend
                    .pop_ephemeral_key(&id)
                    .expect("error popping ephemeral key")
                    .is_none()
            );
        }

        #[test]
        fn test_pop_ephemeral_key() {
            use ::sickgnal_core::e2e::keys::EphemeralSecretKey;

            let mut backend = $setup;

            assert_eq!(
                0,
                backend
                    .available_ephemeral_keys()
                    .expect("error getting available ephemeral keys")
                    .len()
            );

            let key = EphemeralSecretKey::new_from_rng($rng);
            let id = key.id;

            backend
                .save_ephemeral_key(key.clone())
                .expect("error saving ephemeral key");

            assert!(
                backend
                    .pop_ephemeral_key(&id)
                    .expect("error popping ephemeral key")
                    .is_some()
            );

            assert!(
                backend
                    .pop_ephemeral_key(&id)
                    .expect("error popping ephemeral key")
                    .is_none()
            );
        }

        #[test]
        fn test_batch_ephemeral_key() {
            use ::sickgnal_core::e2e::keys::EphemeralSecretKey;

            let mut backend = $setup;

            assert_eq!(
                0,
                backend
                    .available_ephemeral_keys()
                    .expect("error getting available ephemeral keys")
                    .len()
            );

            let keys = [
                EphemeralSecretKey::new_from_rng($rng),
                EphemeralSecretKey::new_from_rng($rng),
                EphemeralSecretKey::new_from_rng($rng),
            ];

            backend
                .save_many_ephemeral_keys(keys.clone().into_iter())
                .expect("error saving many ephemeral keys");

            assert_eq!(
                3,
                backend
                    .available_ephemeral_keys()
                    .expect("error getting available ephemeral keys")
                    .len()
            );

            assert!(
                backend
                    .ephemeral_key(&keys[0].id)
                    .expect("error getting ephemeral key")
                    .is_some()
            );
            assert!(
                backend
                    .ephemeral_key(&keys[1].id)
                    .expect("error getting ephemeral key")
                    .is_some()
            );
            assert!(
                backend
                    .ephemeral_key(&keys[2].id)
                    .expect("error getting ephemeral key")
                    .is_some()
            );

            let ids = keys.iter().map(|k| k.id);
            backend
                .delete_many_ephemeral_key(ids)
                .expect("error deleting many ephemeral keys");

            assert_eq!(
                0,
                backend
                    .available_ephemeral_keys()
                    .expect("error getting available ephemeral keys")
                    .len()
            );

            // test clearing
            backend
                .save_many_ephemeral_keys(keys.clone().into_iter())
                .expect("error saving many ephemeral keys");

            assert_eq!(
                3,
                backend
                    .available_ephemeral_keys()
                    .expect("error getting available ephemeral keys")
                    .len()
            );

            backend
                .clear_ephemeral_keys()
                .expect("error clearing ephemeral keys");

            assert_eq!(
                0,
                backend
                    .available_ephemeral_keys()
                    .expect("error getting available ephemeral keys")
                    .len()
            );
        }

        #[test]
        fn test_session_keys() {
            use ::sickgnal_core::e2e::keys::SymetricKey;
            use ::uuid::Uuid;

            let mut backend = $setup;

            assert_eq!(
                0,
                backend
                    .available_ephemeral_keys()
                    .expect("error getting available ephemeral keys")
                    .len()
            );

            let mut key_1: SymetricKey = [0; 32];
            $rng.fill_bytes(&mut key_1);

            let mut key_2: SymetricKey = [0; 32];
            $rng.fill_bytes(&mut key_2);

            let mut key_3: SymetricKey = [0; 32];
            $rng.fill_bytes(&mut key_3);

            let id_1 = Uuid::new_v4();
            let id_2 = Uuid::new_v4();
            let id_3 = Uuid::new_v4();

            let user_1 = Uuid::new_v4();
            let user_2 = Uuid::new_v4();

            backend
                .add_session_key(user_1, id_1, key_1.clone())
                .expect("error saving session key");
            backend
                .add_session_key(user_1, id_2, key_2.clone())
                .expect("error saving session key");
            backend
                .add_session_key(user_1, id_3, key_3.clone())
                .expect("error saving session key");
            backend
                .add_session_key(user_2, id_3, key_3.clone())
                .expect("error saving session key");

            assert_eq!(
                Some(key_1),
                backend
                    .session_key(user_1, id_1)
                    .expect("error getting session key")
            );
            assert_eq!(
                Some(key_2),
                backend
                    .session_key(user_1, id_2)
                    .expect("error getting session key")
            );
            assert_eq!(
                Some(key_3),
                backend
                    .session_key(user_1, id_3)
                    .expect("error getting session key")
            );
            assert_eq!(
                Some(key_3),
                backend
                    .session_key(user_2, id_3)
                    .expect("error getting session key")
            );

            backend
                .delete_session_key(user_2, id_3)
                .expect("error deleting session key");

            assert_eq!(
                None,
                backend
                    .session_key(user_2, id_3)
                    .expect("error saving session key")
            );
            assert_eq!(
                Some(key_3),
                backend
                    .session_key(user_1, id_3)
                    .expect("error getting session key")
            );

            backend
                .cleanup_session_keys(&user_1, &id_1, &id_2)
                .expect("error cleaning up session keys");

            assert_eq!(
                Some(key_1),
                backend
                    .session_key(user_1, id_1)
                    .expect("error getting session key")
            );
            assert_eq!(
                Some(key_2),
                backend
                    .session_key(user_1, id_2)
                    .expect("error getting session key")
            );
            assert_eq!(
                None,
                backend
                    .session_key(user_1, id_3)
                    .expect("error getting session key")
            );

            backend
                .clear_session_keys()
                .expect("error clearing session keys");

            assert_eq!(
                None,
                backend
                    .session_key(user_1, id_1)
                    .expect("error getting session key")
            );
            assert_eq!(
                None,
                backend
                    .session_key(user_1, id_2)
                    .expect("error getting session key")
            );
        }

        #[test]
        fn test_sessions() {
            use ::sickgnal_core::e2e::client::session::E2ESession;
            let mut backend = $setup;

            let user_id = Uuid::new_v4();
            let sending_key = [1; 32];
            let receiving_key = [2; 32];

            let sess = E2ESession {
                correspondant_id: user_id,
                sending_key_id: Uuid::new_v4(),
                sending_key,
                key_msg_count: 42,
                receiving_key_id: Uuid::new_v4(),
                receiving_key,
            };

            assert!(
                backend
                    .load_all_sessions()
                    .expect("error loading all sessions")
                    .is_empty()
            );

            backend.save_session(&sess).expect("error saving session");

            assert_eq!(
                1,
                backend
                    .load_all_sessions()
                    .expect("error loading all sessions")
                    .len()
            );

            assert_eq!(
                Some(sending_key),
                backend
                    .session_key(user_id, sess.sending_key_id)
                    .expect("error getting session key")
            );
            assert_eq!(
                Some(receiving_key),
                backend
                    .session_key(user_id, sess.receiving_key_id)
                    .expect("error getting session key")
            );

            let stored_sess = backend
                .load_session(&user_id)
                .expect("error loading session")
                .expect("the session was not stored");

            assert_eq!(stored_sess.correspondant_id, sess.correspondant_id);
            assert_eq!(stored_sess.sending_key_id, sess.sending_key_id);
            assert_eq!(stored_sess.sending_key, sess.sending_key);
            assert_eq!(stored_sess.key_msg_count, sess.key_msg_count);
            assert_eq!(stored_sess.receiving_key_id, sess.receiving_key_id);
            assert_eq!(stored_sess.receiving_key, sess.receiving_key);

            backend
                .delete_session(&user_id)
                .expect("error deleting session");

            assert!(
                backend
                    .load_session(&user_id)
                    .expect("error loading session")
                    .is_none()
            );
            assert!(
                backend
                    .load_all_sessions()
                    .expect("error loading all sessions")
                    .is_empty()
            );
        }
    };
}
