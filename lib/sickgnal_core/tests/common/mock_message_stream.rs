use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use sickgnal_core::e2e::{
    message::{E2EMessage, E2EPacket},
    message_stream::{E2EMessageReader, E2EMessageStream, E2EMessageWriter, MessageStreamError},
};
use tokio::sync::mpsc::{
    UnboundedReceiver, UnboundedSender, error::TryRecvError, unbounded_channel,
};
use tokio::time;
use tracing::{debug, trace, warn};
use uuid::Uuid;

#[allow(unused)]
pub struct MockMessageStreamInner {
    next_request_id: u16,
    incoming_count: usize,
    outgoing_count: usize,
    outgoing_wait_timeout: Duration,

    incoming_tx: Option<UnboundedSender<E2EPacket>>,
    incoming_rx: Option<UnboundedReceiver<E2EPacket>>,

    outgoing_tx: Option<UnboundedSender<E2EPacket>>,
    outgoing_rx: Option<UnboundedReceiver<E2EPacket>>,

    prepared_on_next_request: VecDeque<PreparedResponse>,
}

#[allow(unused)]
enum PreparedResponse {
    MirrorRequestId(E2EMessage),
    Untagged(E2EMessage),
    Exact(E2EPacket),
}

impl Default for MockMessageStreamInner {
    fn default() -> Self {
        let (incoming_tx, incoming_rx) = unbounded_channel();
        let (outgoing_tx, outgoing_rx) = unbounded_channel();

        Self {
            next_request_id: 1,
            incoming_count: 0,
            outgoing_count: 0,
            outgoing_wait_timeout: Duration::from_secs(1),
            incoming_tx: Some(incoming_tx),
            incoming_rx: Some(incoming_rx),
            outgoing_tx: Some(outgoing_tx),
            outgoing_rx: Some(outgoing_rx),
            prepared_on_next_request: VecDeque::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct MockMessageStream {
    inner: Arc<Mutex<MockMessageStreamInner>>,
}

#[derive(Clone)]
pub struct MockMessageReader {
    inner: Arc<Mutex<MockMessageStreamInner>>,
}

#[derive(Clone)]
pub struct MockMessageWriter {
    inner: Arc<Mutex<MockMessageStreamInner>>,
}

#[allow(unused)]
impl MockMessageStream {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add the responses for a chat client start flow to the incoming queue
    ///
    /// `available_keys` should only contain keys that the client has, or it would send
    /// an additional PreKeyDelete request.
    pub fn push_start_flow(
        &mut self,
        available_keys: Vec<Uuid>,
        expect_prekey_upload: bool,
        new_sessions: Vec<E2EMessage>,
        initial_msgs_in_new_sessions: usize,
        new_messages: Vec<E2EMessage>,
    ) {
        debug!(
            available_keys = available_keys.len(),
            expect_prekey_upload,
            new_sessions = new_sessions.len(),
            new_messages = new_messages.len(),
            "mock stream: enqueue start flow"
        );

        let prekey_status = E2EMessage::PreKeyStatus {
            limit: 1000,
            keys: available_keys,
        };

        self.push_incoming_untagged(prekey_status.clone());

        if expect_prekey_upload {
            self.push_incoming_untagged(E2EMessage::Ok);
        }

        let new_sessions_len = new_sessions.len();

        self.push_incoming_untagged(E2EMessage::MessagesList {
            messages: new_sessions,
        });

        if new_sessions_len > 0 {
            // Client will send a delivered acknowledgement for each initial message
            for _ in 0..initial_msgs_in_new_sessions {
                self.push_incoming_untagged(E2EMessage::Ok);
            }

            self.push_incoming_untagged(E2EMessage::MessagesList {
                messages: Vec::new(),
            });
        }

        let new_messages_len = new_messages.len();

        self.push_incoming_untagged(E2EMessage::MessagesList {
            messages: new_messages,
        });

        if new_messages_len > 0 {
            // Client will send a delivered acknowledgement for each message
            for _ in 0..new_messages_len {
                self.push_incoming_untagged(E2EMessage::Ok);
            }
            self.push_incoming_untagged(E2EMessage::MessagesList {
                messages: Vec::new(),
            });
        }

        self.push_incoming_untagged(prekey_status);

        // client will delete keys on new sessions opening
        for _ in 0..new_sessions_len {
            self.push_incoming_untagged(E2EMessage::Ok);
        }

        // Instant relay
        self.push_incoming_untagged(E2EMessage::Ok);
    }

    /// Push the messages for the account creation flow to the incoming messages
    pub fn push_create_account_flow(&mut self, id: Uuid) {
        debug!(%id, "mock stream: enqueue create-account flow");

        self.push_incoming_untagged(E2EMessage::AuthToken {
            id,
            token: "mock-token".into(),
        });
        self.push_incoming_untagged(E2EMessage::Ok);
    }

    pub fn push_incoming(&self, packet: E2EPacket) {
        trace!(request_id = packet.request_id, message = ?packet.message, "mock stream: push incoming packet");

        let tx = {
            let inner = self.inner.lock().expect("mock stream mutex poisoned");
            inner.incoming_tx.clone()
        };

        if let Some(tx) = tx {
            if tx.send(packet).is_err() {
                warn!("mock stream: failed to push incoming packet (incoming side closed)");
            } else {
                let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
                inner.incoming_count += 1;
            }
        } else {
            warn!("mock stream: incoming stream already closed");
        }
    }

    pub fn push_incoming_tagged(&self, message: E2EMessage) {
        let request_id = {
            let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
            let request_id = inner.next_request_id;
            inner.next_request_id = inner.next_request_id.wrapping_add(1);
            if inner.next_request_id == 0 {
                inner.next_request_id = 1;
            }
            request_id
        };

        self.push_incoming(E2EPacket {
            request_id,
            message,
        });
    }

    pub fn push_incoming_untagged(&self, message: E2EMessage) {
        self.push_incoming(E2EPacket::untagged(message));
    }

    pub fn set_outgoing_wait_timeout(&self, timeout: Duration) {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.outgoing_wait_timeout = timeout;
    }

    pub fn outgoing_wait_timeout(&self) -> Duration {
        let inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.outgoing_wait_timeout
    }

    pub fn set_outgoing_wait_timeout_millis(&self, timeout_ms: u64) {
        self.set_outgoing_wait_timeout(Duration::from_millis(timeout_ms));
    }

    /// Queue a server response that will be emitted when the client sends its next request.
    ///
    /// The response packet mirrors the triggering request id.
    pub fn queue_response_on_next_request(&self, message: E2EMessage) {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner
            .prepared_on_next_request
            .push_back(PreparedResponse::MirrorRequestId(message));
        debug!(
            queued = inner.prepared_on_next_request.len(),
            "mock stream: queued mirrored response for next request"
        );
    }

    /// Queue an untagged server response that will be emitted on the next client request.
    pub fn queue_untagged_response_on_next_request(&self, message: E2EMessage) {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner
            .prepared_on_next_request
            .push_back(PreparedResponse::Untagged(message));
        debug!(
            queued = inner.prepared_on_next_request.len(),
            "mock stream: queued untagged response for next request"
        );
    }

    /// Queue an exact packet that will be emitted on the next client request.
    pub fn queue_packet_on_next_request(&self, packet: E2EPacket) {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner
            .prepared_on_next_request
            .push_back(PreparedResponse::Exact(packet));
        debug!(
            queued = inner.prepared_on_next_request.len(),
            "mock stream: queued exact packet for next request"
        );
    }

    pub fn drain_outgoing(&self) -> Vec<E2EPacket> {
        let mut rx = {
            let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
            inner.outgoing_rx.take()
        };

        let Some(mut rx) = rx.take() else {
            return Vec::new();
        };

        let mut out = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(packet) => {
                    let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
                    inner.outgoing_count = inner.outgoing_count.saturating_sub(1);
                    out.push(packet);
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }

        {
            let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
            inner.outgoing_rx = Some(rx);
        }

        out
    }

    pub async fn wait_for_outgoing(&self) -> Result<E2EPacket, MessageStreamError> {
        let timeout = self.outgoing_wait_timeout();
        trace!(
            timeout_ms = timeout.as_millis(),
            "mock stream: waiting for one outgoing packet"
        );

        let mut rx = {
            let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
            inner.outgoing_rx.take().ok_or_else(|| {
                MessageStreamError::new(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "mock message stream outgoing receiver unavailable",
                ))
            })?
        };

        let recv_result = time::timeout(timeout, rx.recv()).await;

        {
            let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
            inner.outgoing_rx = Some(rx);
        }

        match recv_result {
            Ok(Some(packet)) => {
                let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
                inner.outgoing_count = inner.outgoing_count.saturating_sub(1);
                Ok(packet)
            }
            Ok(None) => Err(MessageStreamError::new(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "mock message stream outgoing channel is closed",
            ))),
            Err(_) => {
                trace!(
                    timeout_ms = timeout.as_millis(),
                    "mock stream: wait_for_outgoing timeout"
                );
                Err(MessageStreamError::new(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timeout waiting for outgoing packet",
                )))
            }
        }
    }

    pub async fn wait_for_n_outgoing(
        &self,
        n: usize,
    ) -> Result<Vec<E2EPacket>, MessageStreamError> {
        trace!(count = n, "mock stream: waiting for outgoing packets");

        let mut packets = Vec::with_capacity(n);
        for _ in 0..n {
            packets.push(self.wait_for_outgoing().await?);
        }

        Ok(packets)
    }

    pub fn incoming_len(&self) -> usize {
        let inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.incoming_count
    }

    pub fn outgoing_len(&self) -> usize {
        let inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.outgoing_count
    }

    /// Close the server->client side of the stream.
    ///
    /// After this, `receive()` eventually returns EOF once buffered messages are consumed.
    pub fn close_incoming(&self) {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.incoming_tx = None;
        debug!("mock stream: incoming side closed");
    }

    /// Close the client->server side of the stream.
    ///
    /// After this, `send()` fails with a broken pipe error.
    pub fn close_outgoing(&self) {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.outgoing_tx = None;
        debug!("mock stream: outgoing side closed");
    }

    /// Close both directions of the stream.
    pub fn close(&self) {
        self.close_incoming();
        self.close_outgoing();
    }
}

#[async_trait]
impl E2EMessageStream for MockMessageStream {
    type Reader = MockMessageReader;
    type Writer = MockMessageWriter;

    fn split(self) -> (Self::Reader, Self::Writer) {
        (
            MockMessageReader {
                inner: Arc::clone(&self.inner),
            },
            MockMessageWriter {
                inner: Arc::clone(&self.inner),
            },
        )
    }
}

#[async_trait]
impl E2EMessageWriter for MockMessageWriter {
    async fn send(&mut self, packet: E2EPacket) -> Result<(), MessageStreamError> {
        debug!(request_id = packet.request_id, message = ?packet.message, "mock stream: send packet");
        let request_id = packet.request_id;

        let outgoing_tx = {
            let inner = self.inner.lock().expect("mock stream mutex poisoned");
            inner.outgoing_tx.clone()
        };

        match outgoing_tx {
            Some(tx) => tx.send(packet).map_err(|_| {
                MessageStreamError::new(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "mock message stream outgoing side is closed",
                ))
            }).map(|_| {
                let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
                inner.outgoing_count += 1;

                if let Some(prepared) = inner.prepared_on_next_request.pop_front() {
                    if let Some(incoming_tx) = inner.incoming_tx.clone() {
                        let response = match prepared {
                            PreparedResponse::MirrorRequestId(message) => E2EPacket {
                                request_id,
                                message,
                            },
                            PreparedResponse::Untagged(message) => E2EPacket::untagged(message),
                            PreparedResponse::Exact(packet) => packet,
                        };

                        trace!(
                            request_id = response.request_id,
                            message = ?response.message,
                            "mock stream: dispatching prepared response"
                        );

                        if incoming_tx.send(response).is_ok() {
                            inner.incoming_count += 1;
                        } else {
                            warn!("mock stream: failed to dispatch prepared response (incoming side closed)");
                        }
                    } else {
                        warn!("mock stream: prepared response dropped (incoming side closed)");
                    }
                } else {
                    trace!("mock stream: no prepared response for this request");
                }
            }),
            None => Err(MessageStreamError::new(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "mock message stream outgoing side is closed",
            ))),
        }
    }
}

#[async_trait]
impl E2EMessageReader for MockMessageReader {
    async fn receive(&mut self) -> Result<E2EPacket, MessageStreamError> {
        let mut rx = {
            let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
            inner.incoming_rx.take().ok_or_else(|| {
                MessageStreamError::new(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "mock message stream incoming receiver unavailable",
                ))
            })?
        };

        let result = rx.recv().await;

        {
            let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
            inner.incoming_rx = Some(rx);
        }

        match result {
            Some(packet) => {
                let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
                inner.incoming_count = inner.incoming_count.saturating_sub(1);
                debug!(request_id = packet.request_id, message = ?packet.message, "mock stream: receive packet");
                Ok(packet)
            }
            None => {
                warn!("mock stream: incoming channel closed");
                Err(MessageStreamError::new(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "mock message stream incoming queue is closed",
                )))
            }
        }
    }
}

#[async_trait]
impl E2EMessageWriter for MockMessageStream {
    async fn send(&mut self, packet: E2EPacket) -> Result<(), MessageStreamError> {
        let mut writer = MockMessageWriter {
            inner: Arc::clone(&self.inner),
        };
        writer.send(packet).await
    }
}

#[async_trait]
impl E2EMessageReader for MockMessageStream {
    async fn receive(&mut self) -> Result<E2EPacket, MessageStreamError> {
        let mut reader = MockMessageReader {
            inner: Arc::clone(&self.inner),
        };
        reader.receive().await
    }
}
