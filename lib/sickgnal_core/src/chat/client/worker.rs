use tokio::sync::mpsc;
use tracing::{debug, error, trace_span};

use crate::chat::{
    client::client::ChatClientHandle, message::ChatMessage, storage::SharedStorageBackend,
};

/// The worker to receive and handle [`ChatMessage`], and
/// dispatch the correct events to the client
pub(crate) async fn receive_loop<S>(
    mut state: ChatClientHandle<S>,
    mut msg_rx: mpsc::Receiver<ChatMessage>,
) where
    S: SharedStorageBackend + 'static,
{
    debug!("Starting chat messages receiving worker");

    while let Some(msg) = msg_rx.recv().await {
        let process_span = trace_span!("process_chat_msg", msg = ?msg);

        let _enter = process_span.enter();

        if let Err(err) = state.handle_incomming_message(msg).await {
            error!("error processing chat message : {}", err);
            break;
        }
    }

    debug!("Stopping");
}
