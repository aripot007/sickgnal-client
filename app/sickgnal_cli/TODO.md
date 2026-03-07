# Plan: Architecture SDK Client & Intégration UI

Vous avez un protocole E2E solide et une UI fonctionnelle, mais isolés. Le plan crée une couche SDK qui gère la persistance, la logique métier et connecte le protocole à l'interface. Priorité sur la synchronisation visible des messages, avec SQLite pour la persistence et architecture modulaire pour faciliter l'ajout futur des groupes.

**Décisions clés :** SQLite via tokio-rusqlite (mature, multiplateforme), architecture 3-couches (`core` → `sdk` → `cli`), connexion async via channels mpsc pour communiquer entre E2EClient et Slint UI.

---

**Steps**

## Phase 1: Fondations Persistence & SDK Structure

1. **Créer le schéma SQLite et la couche storage**
   - Créer lib/sickgnal_sdk/src/storage/mod.rs avec trait `StorageBackend`
   - Créer lib/sickgnal_sdk/src/storage/schema.rs avec DDL pour:
     - Table `accounts` (user_id, username, identity_key_priv, midterm_key, created_at)
     - Table `conversations` (id, peer_user_id, peer_name, last_message_at, unread_count)
     - Table `messages` (id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id)
     - Table `sessions` (peer_user_id, session_data_json, updated_at)
     - Table `keys` (key_id, key_type, key_data, created_at)
   - Créer lib/sickgnal_sdk/src/storage/sqlite.rs implémentant `StorageBackend` avec tokio-rusqlite
   - Chiffrement at-rest avec SQLCipher ou chacha20 sur les colonnes sensibles (identity_key, messages.content, session_data)

2. **Implémenter KeyStorageBackend pour SQLite**
   - Créer lib/sickgnal_core/src/e2e/storage/sqlite_key_storage.rs
   - Implémenter trait `KeyStorageBackend` en utilisant la table `keys`
   - Gère persistence de: identity keypair, midterm key, ephemeral keys, user public keys, sessions
   - Ajouter dans lib/sickgnal_core/src/e2e/storage/mod.rs

3. **Créer l'architecture du SDK Client**
   - Créer lib/sickgnal_sdk/src/client.rs avec struct `SdkClient`:
     - Contient: `E2EClient`, `StorageBackend`, `event_tx: mpsc::Sender<ClientEvent>`
     - Méthodes: `new()`, `connect()`, `disconnect()`, `send_message()`, `mark_as_read()`
   - Créer lib/sickgnal_sdk/src/events.rs avec enum `ClientEvent`:
     - `NewMessage(ConversationId, ChatMessage)`
     - `MessageStatusUpdate(MessageId, MessageStatus)`
     - `ConversationCreated(Conversation)`
     - `TypingIndicator(ConversationId, bool)`
     - `ConnectionStateChanged(ConnectionState)`
   - Permet à l'UI de s'abonner aux événements via channel

## Phase 2: Logique Métier & Account Management

4. **Implémenter la gestion de compte**
   - Créer lib/sickgnal_sdk/src/account.rs avec:
     - `create_account(username, password)` → génère clés, appelle `E2EClient::create_account()`
     - `load_account(password)` → charge depuis SQLite, déchiffre clés
     - `derive_encryption_key(password, salt)` avec Argon2 pour chiffrer la BDD
   - Modifier main.rs pour:
     - Détecter si compte existe (check BDD)
     - Afficher écran création compte si nécessaire
     - Prompt password et charge compte
   - Ajouter `AccountSetup` component dans components

5. **Implémenter la logique de conversation**
   - Créer lib/sickgnal_sdk/src/conversation.rs avec:
     - `get_or_create_conversation(peer_user_id)` → cherche en BDD ou crée nouvelle
     - `list_conversations()` → charge depuis SQLite avec derniers messages
     - `delete_conversation(conv_id)`
     - `update_last_activity(conv_id, timestamp)`
   - Gère la logique: message arrivant pour conversation inexistante → créer automatiquement

6. **Implémenter la logique d'envoi de messages**
   - Compléter le TODO dans client.rs:
     - Ajouter méthode `pub async fn send_chat_message(&mut self, peer_id: Uuid, message: ChatMessage)`
     - Récupère ou crée session avec peer
     - Chiffre `ChatMessage` avec session key
     - Envoie `E2EMessage::ConversationMessage` via `send_authenticated_e2e()`
   - Dans lib/sickgnal_sdk/src/client.rs:
     - `send_message(conv_id, text)` → crée `ChatMessage`, appelle `E2EClient::send_chat_message()`, sauve en BDD

## Phase 3: Synchronisation & Intégration UI

7. **Implémenter le sync handler**
   - Créer lib/sickgnal_sdk/src/sync.rs:
     - `start_sync(client: Arc<Mutex<SdkClient>>)` → lance task async
     - Utilise `SyncIterator` de sync_iterator.rs
     - Pour chaque `ChatMessage` reçu:
       - Identifie/crée conversation avec `conversation_id`
       - Sauve message en BDD
       - Envoie `ClientEvent::NewMessage` via channel
     - Gère `ControlMessage::AckReception` automatiquement

8. **Créer le bridge SDK ↔ Slint**
   - Créer app/sickgnal_cli/src/bridge.rs:
     - `struct UiBridge { sdk_client: Arc<Mutex<SdkClient>>, ui_handle: Weak<AppWindow> }`
     - `start_event_loop()` → écoute `ClientEvent` du SDK via mpsc receiver
     - Pour chaque event, met à jour les modèles Slint via `slint::invoke_from_event_loop()`
     - Map `ChatMessage` → `MessageData`, `Conversation` → `Conversation` Slint
   - Créer app/sickgnal_cli/src/state.rs:
     - `AppState` avec `sdk_client`, `conversations: HashMap<i32, ConversationState>`
     - Gère sync entre BDD et modèles Slint

9. **Connecter l'UI au SDK**
   - Modifier main.rs:
     - Remplacer les données mock par chargement depuis `SdkClient::list_conversations()`
     - Dans callback `on_send_message`: appeler `sdk_client.send_message(active_id, text)`
     - Lancer `UiBridge::start_event_loop()` dans thread séparé
     - Connecter à serveur avec `sdk_client.connect(server_addr)`
     - Lancer sync initial avec `start_sync()`
   - Gérer états UI: Loading, Connected, Error avec indicateur visuel

10. **Compléter les TODOs dans E2EClient**
    - Implémenter client.rs:
      - Charge identity keypair et midterm key depuis `KeyStorageBackend`
      - Reconstruit sessions depuis storage
    - Implémenter client.rs:
      - Appelle `load()` puis `authenticate_with_challenge()`
    - Compléter sync_iterator.rs:
      - Implémenter gestion des types manquants dans le match
      - Remplacer `todo!()` par traitement approprié ou log

## Phase 4: Robustesse & Features Finales

11. **Implémenter le statut des messages**
    - Dans lib/sickgnal_sdk/src/message.rs:
      - Enum `MessageStatus`: Sending, Sent, Delivered, Read, Failed
      - `update_message_status(message_id, status)` → met à jour BDD + envoie event
    - Gérer les ACKs reçus depuis le protocole (`AckReception`, `AckRead`)
    - Mettre à jour indicateurs dans l'UI (✓, ✓✓, ✓✓ bleu)

12. **Implémenter typing indicators**
    - Dans lib/sickgnal_sdk/src/client.rs:
      - `send_typing_indicator(conv_id, is_typing)` → envoie `ControlMessage::IsTyping`
    - Dans sync handler: propager `IsTyping` reçus vers UI via `ClientEvent::TypingIndicator`
    - Dans main.rs: connecter input text change → debounced typing indicator

13. **Gestion des erreurs et reconnexion**
    - Créer lib/sickgnal_sdk/src/errors.rs avec erreurs user-facing
    - Implémenter reconnexion automatique avec backoff exponentiel dans `SdkClient::connect()`
    - Ajouter `ClientEvent::Error(String)` pour afficher erreurs dans l'UI
    - Gérer perte de connexion pendant sync: sauvegarder état, retry

14. **Configuration et logging**
    - Créer lib/sickgnal_sdk/src/config.rs:
      - Struct `ClientConfig` (server_addr, db_path, key_rotation_interval)
      - Charger depuis fichier TOML ou env vars
    - Ajouter `tracing` dans dependencies, remplacer tous les `println!()` par `tracing::info/debug/error`
    - Créer subscriber dans main.rs

---

**Verification**

1. **Test création de compte**
   ```bash
   cargo run --bin sickgnal_cli
   # → Affiche écran de création, entrer username/password
   # Vérifier que BDD créée dans ~/.config/sickgnal/data.db
   ```

2. **Test sync initial**
   - Lancer serveur de test
   - Démarrer client, vérifier logs de connexion
   - Envoyer messages depuis autre client → doivent apparaître dans l'UI en temps réel

3. **Test envoi message**
   - Taper message dans l'UI, cliquer Send
   - Vérifier logs: chiffrement, envoi réseau, sauvegarde BDD
   - Vérifier statut passe de "sending" → "sent" → "delivered"

4. **Test persistence**
   - Envoyer des messages, fermer app
   - Redémarrer → vérifier que conversations et messages rechargés depuis BDD

5. **Test conversation auto-créée**
   - Depuis autre compte, envoyer message à compte test
   - Vérifier que nouvelle conversation apparaît automatiquement dans liste

---

**Decisions**

- **SQLite over sled/redb**: Maturité, tooling, support multiplateforme excellent
- **Channel-based events**: Découple SDK async de main thread Slint, simple à débugger
- **Architecture 3-couches**: `sickgnal_core` (protocol), `sickgnal_sdk` (business logic), `sickgnal_cli` (UI) → séparation claire des responsabilités
- **Chiffrement at-rest**: Dérive clé depuis password avec Argon2, chiffre colonnes sensibles
- **Conversation auto-creation**: Message arrivant pour conv inconnue → créer silencieusement, évite erreurs
- **1-to-1 d'abord**: Architecture extensible pour groupes (conversation.peer_user_id → conversation_members table) mais implémentation simple d'abord