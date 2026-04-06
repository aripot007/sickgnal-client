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
        fn test_identity_keypair() {
            use ::sickgnal_core::e2e::keys::IdentityKeyPair;

            let mut backend = $setup;

            assert!(backend.identity_keypair().is_err());
            assert!(backend.identity_keypair_opt().unwrap().is_none());

            let keypair: IdentityKeyPair = IdentityKeyPair::new_from_rng($rng);

            backend.set_identity_keypair(keypair).unwrap();

            assert!(backend.identity_keypair().is_ok());
            assert!(backend.identity_keypair_opt().unwrap().is_some());

            backend.clear_identity_keypair().unwrap();

            assert!(backend.identity_keypair().is_err());
            assert!(backend.identity_keypair_opt().unwrap().is_none());
        }

        #[test]
        fn test_midterm_key() {
            use ::sickgnal_core::e2e::keys::X25519Secret;

            let mut backend = $setup;

            assert!(backend.midterm_key().is_err());
            assert!(backend.midterm_key_opt().unwrap().is_none());

            let key: X25519Secret = X25519Secret::random_from_rng($rng);

            backend.set_midterm_key(key).unwrap();

            assert!(backend.midterm_key().is_ok());
            assert!(backend.midterm_key_opt().unwrap().is_some());

            backend.clear_midterm_key().unwrap();

            assert!(backend.midterm_key().is_err());
            assert!(backend.midterm_key_opt().unwrap().is_none());
        }

        #[test]
        fn test_ephemeral_key() {
            use ::sickgnal_core::e2e::keys::EphemeralSecretKey;

            let mut backend = $setup;

            assert_eq!(0, backend.available_ephemeral_keys().unwrap().count());

            let key = EphemeralSecretKey::new_from_rng($rng);
            let id = key.id;

            backend.save_ephemeral_key(key.clone()).unwrap();

            assert_eq!(1, backend.available_ephemeral_keys().unwrap().count());
            assert!(backend.ephemeral_key(&id).unwrap().is_some());

            backend.delete_ephemeral_key(id).unwrap();

            assert_eq!(0, backend.available_ephemeral_keys().unwrap().count());
            assert!(backend.ephemeral_key(&id).unwrap().is_none());
            assert!(backend.pop_ephemeral_key(&id).unwrap().is_none());
        }

        #[test]
        fn test_pop_ephemeral_key() {
            use ::sickgnal_core::e2e::keys::EphemeralSecretKey;

            let mut backend = $setup;

            assert_eq!(0, backend.available_ephemeral_keys().unwrap().count());

            let key = EphemeralSecretKey::new_from_rng($rng);
            let id = key.id;

            backend.save_ephemeral_key(key.clone()).unwrap();

            assert!(backend.pop_ephemeral_key(&id).unwrap().is_some());

            assert!(backend.pop_ephemeral_key(&id).unwrap().is_none());
        }

        #[test]
        fn test_batch_ephemeral_key() {
            use ::sickgnal_core::e2e::keys::EphemeralSecretKey;

            let mut backend = $setup;

            assert_eq!(0, backend.available_ephemeral_keys().unwrap().count());

            let keys = [
                EphemeralSecretKey::new_from_rng($rng),
                EphemeralSecretKey::new_from_rng($rng),
                EphemeralSecretKey::new_from_rng($rng),
            ];

            backend
                .save_many_ephemeral_keys(keys.clone().into_iter())
                .unwrap();

            assert_eq!(3, backend.available_ephemeral_keys().unwrap().count());

            assert!(backend.ephemeral_key(&keys[0].id).unwrap().is_some());
            assert!(backend.ephemeral_key(&keys[1].id).unwrap().is_some());
            assert!(backend.ephemeral_key(&keys[2].id).unwrap().is_some());

            let ids = keys.iter().map(|k| k.id);
            backend.delete_many_ephemeral_key(ids).unwrap();

            assert_eq!(0, backend.available_ephemeral_keys().unwrap().count());

            // test clearing
            backend
                .save_many_ephemeral_keys(keys.clone().into_iter())
                .unwrap();

            assert_eq!(3, backend.available_ephemeral_keys().unwrap().count());

            backend.clear_ephemeral_keys().unwrap();

            assert_eq!(0, backend.available_ephemeral_keys().unwrap().count());
        }

        #[test]
        fn test_session_keys() {
            use ::sickgnal_core::e2e::keys::SymetricKey;
            use ::uuid::Uuid;

            let mut backend = $setup;

            assert_eq!(0, backend.available_ephemeral_keys().unwrap().count());

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
                .unwrap();
            backend
                .add_session_key(user_1, id_2, key_2.clone())
                .unwrap();
            backend
                .add_session_key(user_1, id_3, key_3.clone())
                .unwrap();
            backend
                .add_session_key(user_2, id_3, key_3.clone())
                .unwrap();

            assert_eq!(Some(key_1), backend.session_key(user_1, id_1).unwrap());
            assert_eq!(Some(key_2), backend.session_key(user_1, id_2).unwrap());
            assert_eq!(Some(key_3), backend.session_key(user_1, id_3).unwrap());
            assert_eq!(Some(key_3), backend.session_key(user_2, id_3).unwrap());

            backend.delete_session_key(user_2, id_3).unwrap();

            assert_eq!(None, backend.session_key(user_2, id_3).unwrap());
            assert_eq!(Some(key_3), backend.session_key(user_1, id_3).unwrap());

            backend.cleanup_session_keys(&user_1, &id_1, &id_2).unwrap();

            assert_eq!(Some(key_1), backend.session_key(user_1, id_1).unwrap());
            assert_eq!(Some(key_2), backend.session_key(user_1, id_2).unwrap());
            assert_eq!(None, backend.session_key(user_1, id_3).unwrap());

            backend.clear_session_keys().unwrap();

            assert_eq!(None, backend.session_key(user_1, id_1).unwrap());
            assert_eq!(None, backend.session_key(user_1, id_2).unwrap());
        }

        #[test]
        fn test_public_keys() {
            use ::sickgnal_core::e2e::keys::IdentityKeyPair;

            let mut backend = $setup;

            let keypair = IdentityKeyPair::new_from_rng($rng);
            let user_id = Uuid::new_v4();

            let public_keypair = keypair.public_keys();

            assert!(backend.user_public_keys(&user_id).unwrap().is_none());

            backend
                .set_user_public_keys(user_id, public_keypair.clone())
                .unwrap();

            assert!(backend.user_public_keys(&user_id).unwrap().is_some());

            let pks = backend.user_public_keys(&user_id).unwrap().unwrap();

            assert_eq!(public_keypair.ed25519, pks.ed25519);
            assert_eq!(public_keypair.x25519, pks.x25519);

            backend.delete_user_public_keys(&user_id).unwrap();

            assert!(backend.user_public_keys(&user_id).unwrap().is_none());

            backend
                .set_user_public_keys(user_id, public_keypair.clone())
                .unwrap();
            backend.delete_user_public_keys(&user_id).unwrap();

            assert!(backend.user_public_keys(&user_id).unwrap().is_none());
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

            assert!(backend.load_all_sessions().unwrap().is_empty());

            backend.save_session(&sess).unwrap();

            assert_eq!(1, backend.load_all_sessions().unwrap().len());

            assert_eq!(
                Some(sending_key),
                backend.session_key(user_id, sess.sending_key_id).unwrap()
            );
            assert_eq!(
                Some(receiving_key),
                backend.session_key(user_id, sess.receiving_key_id).unwrap()
            );

            let stored_sess = backend.load_session(&user_id).unwrap().unwrap();

            assert_eq!(stored_sess.correspondant_id, sess.correspondant_id);
            assert_eq!(stored_sess.sending_key_id, sess.sending_key_id);
            assert_eq!(stored_sess.sending_key, sess.sending_key);
            assert_eq!(stored_sess.key_msg_count, sess.key_msg_count);
            assert_eq!(stored_sess.receiving_key_id, sess.receiving_key_id);
            assert_eq!(stored_sess.receiving_key, sess.receiving_key);

            backend.delete_session(&user_id).unwrap();

            assert!(backend.load_session(&user_id).unwrap().is_none());
            assert!(backend.load_all_sessions().unwrap().is_empty());
        }

        #[test]
        fn test_batch_sessions() {
            use ::sickgnal_core::e2e::client::session::E2ESession;

            let mut backend = $setup;

            let sess_1 = E2ESession {
                correspondant_id: Uuid::new_v4(),
                sending_key_id: Uuid::new_v4(),
                sending_key: [1; 32],
                key_msg_count: 42,
                receiving_key_id: Uuid::new_v4(),
                receiving_key: [2; 32],
            };

            let sess_2 = E2ESession {
                correspondant_id: Uuid::new_v4(),
                sending_key_id: Uuid::new_v4(),
                sending_key: [3; 32],
                key_msg_count: 69,
                receiving_key_id: Uuid::new_v4(),
                receiving_key: [4; 32],
            };

            assert!(backend.load_all_sessions().unwrap().is_empty());

            backend.save_many_sessions(&[&sess_1, &sess_2]).unwrap();

            assert_eq!(2, backend.load_all_sessions().unwrap().len());
        }
    };
}
