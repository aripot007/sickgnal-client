use futures::channel::mpsc;

use crate::e2e::{message::E2EMessage, message_stream::E2EMessageWriter};

/// The sending worker loop
///
/// Sends messages from `channel_out` using the `writer`
pub async fn send_loop<W>(mut writer: W, mut channel_out: mpsc::Receiver<E2EMessage>)
where
    W: E2EMessageWriter,
{
    loop {
        let msg = match channel_out.recv().await {
            Ok(msg) => msg,

            // Sending channel closed, stop the worker
            Err(_) => break,
        };

        if let Err(e) = writer.send(msg).await {
            println!("Error sending message : {}", e);
            break;
        }
    }

    println!("Stopping sending worker");
}
