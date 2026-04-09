use tokio::sync::mpsc;
use tracing::{error, info};

use crate::e2e::{message::E2EPacket, message_stream::E2EMessageWriter};

/// The sending worker loop
///
/// Sends messages from `channel_out` using the `writer`
pub async fn send_loop<W>(mut writer: W, mut channel_out: mpsc::Receiver<E2EPacket>)
where
    W: E2EMessageWriter,
{
    info!("Starting sending worker");

    loop {
        let msg = match channel_out.recv().await {
            Some(msg) => msg,

            // Sending channel closed, stop the worker
            None => break,
        };

        if let Err(e) = writer.send(msg).await {
            error!("Error sending message : {}", e);
            break;
        }
    }

    info!("Stopping sending worker");
}
