use tokio::sync::mpsc;
use tracing::{debug, error};

use crate::chat::{
    client::state::ChatClientState, message::ChatMessage, storage::SharedStorageBackend,
};

/// The worker to receive and handle [`ChatMessage`], and
/// dispatch the correct events to the client
pub async fn receive_loop<S>(mut state: ChatClientState<S>, mut msg_rx: mpsc::Receiver<ChatMessage>)
where
    S: SharedStorageBackend + 'static,
{
    debug!("Starting chat messages receiving worker");

    while let Some(msg) = msg_rx.recv().await {
        if let Err(err) = state.handle_incomming_message(msg).await {
            error!("error processing chat message : {}", err);
            break;
        }
    }

    debug!("Stopping");
}
