//! LRU Cache for [`EncryptedPayload`]s

use std::{collections::VecDeque, num::NonZeroUsize};

use lru::LruCache;
use uuid::Uuid;

use crate::e2e::message::encrypted_payload::EncryptedPayload;

/// LRU Cache for undecipherable messages, to handle out-of-order messages with key rotation
///
/// Limits the number of users and the total number of messages per user
pub struct PayloadCache {
    /// Maximum number of messages for each users
    max_msgs_per_user: usize,

    /// Caches for each users
    user_caches: LruCache<Uuid, UserPayloadCache>,
}

/// A LRU Cache for undecipherable messages for a specific user
///
/// # Invariant
///
/// `cache` never contains empty queues
///
/// `capacity` is never 0
///
/// `total_messages` is the total number of payloads stored in the cache, ie
/// the sum of the length of each `VecDeque` for each key :
///
/// ```ignore
/// let mut count = 0;
/// for (_, msgs) in self.cache.iter() {
///     assert!(msgs.len() > 0);
///     count += msgs.len();
/// }
///
/// assert!(self.capacity != 0);
/// assert_eq!(self.total_messages, count);
/// ```
struct UserPayloadCache {
    /// Total number of messages across all keys
    total_messages: usize,

    /// The total number of messages allowed
    capacity: usize,

    /// Cached messages for each key
    cache: LruCache<Uuid, VecDeque<EncryptedPayload>>,
}

impl PayloadCache {
    /// Create a new [`PayloadCache`].
    ///
    /// `users_capacity` is the maximum number of users allowed to have a cache. If a new
    /// user is inserted, the cache will evict the least-recently used one.
    ///
    /// `messages_capacity` is the maximum number of messages stored per user. When going over
    /// the limit, the cache will try to evict the messages of the least-recently used key, or
    /// the oldest message for the key.
    ///
    /// # Panic
    ///
    /// Panics if `users_capacity` or `messages_capacity` is 0
    pub fn new(users_capacity: usize, messages_capacity: usize) -> Self {
        if messages_capacity == 0 {
            panic!("messages_capacity must not be 0");
        }

        Self {
            max_msgs_per_user: messages_capacity,
            user_caches: LruCache::new(
                NonZeroUsize::new(users_capacity).expect("users_capacity must not be 0"),
            ),
        }
    }

    // Get the cached payloads for the given user and key
    pub fn pop(&mut self, user_id: &Uuid, key_id: &Uuid) -> Option<VecDeque<EncryptedPayload>> {
        match self.user_caches.pop(user_id) {
            None => None,
            Some(mut cache) => cache.pop(key_id),
        }
    }

    // return the evicted payloads for the user
    pub fn push(
        &mut self,
        user_id: Uuid,
        key_id: Uuid,
        payload: EncryptedPayload,
    ) -> Option<Vec<EncryptedPayload>> {
        let cache = self
            .user_caches
            .get_or_insert_mut(user_id, || UserPayloadCache::new(self.max_msgs_per_user));

        cache.push(key_id, payload)
    }
}

impl UserPayloadCache {
    /// Create a new [`UserPayloadCache`]
    ///
    /// `capacity` is the maximum number of messages stored per user. When going over
    /// the limit, the cache will try to evict the messages of the least-recently used key, or
    /// the oldest message for the key.
    ///
    /// # Panic
    ///
    /// Panics if `capacity` is 0
    pub fn new(capacity: usize) -> Self {
        if capacity == 0 {
            panic!("capacity must not be 0");
        }

        Self {
            total_messages: 0,
            capacity,
            cache: LruCache::unbounded(),
        }
    }

    // Get the cached payloads for the given key
    pub fn pop(&mut self, key_id: &Uuid) -> Option<VecDeque<EncryptedPayload>> {
        let msgs = self.cache.pop(key_id);

        if let Some(queue) = &msgs {
            self.total_messages -= queue.len();
        }

        msgs
    }

    // return the evicted payloads
    pub fn push(
        &mut self,
        key_id: Uuid,
        payload: EncryptedPayload,
    ) -> Option<Vec<EncryptedPayload>> {
        let mut evicted = None;

        // Evict messages if pushing exceeds capacity
        if self.total_messages >= self.capacity {
            // Bump the current key to MRU in case it was LRU
            self.cache.promote(&key_id);

            let (id, mut queue) = self
                .cache
                .pop_lru()
                .expect("full cache should not be empty");

            // The only queue is for this key, pop the oldest message
            if id == key_id {
                let oldest_msg = queue
                    .pop_front()
                    .expect("cache should not contain empty queues");

                evicted = Some(vec![oldest_msg]);

                // Put back the queue in the cache
                self.cache.push(key_id, queue);
                self.total_messages -= 1;
            } else {
                self.total_messages -= queue.len();
                evicted = Some(Vec::from(queue));
            }
        }

        // Add the message to the cache
        self.cache
            .get_or_insert_mut(key_id, VecDeque::new)
            .push_back(payload);

        self.total_messages += 1;

        evicted
    }
}
