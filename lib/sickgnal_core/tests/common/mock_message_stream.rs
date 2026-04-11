use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use sickgnal_core::e2e::{
    message::{E2EMessage, E2EPacket},
    message_stream::{E2EMessageReader, E2EMessageStream, E2EMessageWriter, MessageStreamError},
};

#[derive(Default)]
struct MockMessageStreamInner {
    incoming: VecDeque<E2EPacket>,
    outgoing: VecDeque<E2EPacket>,
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

impl MockMessageStream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_incoming(incoming: Vec<E2EPacket>) -> Self {
        let inner = MockMessageStreamInner {
            incoming: incoming.into(),
            outgoing: VecDeque::new(),
        };

        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn push_incoming(&self, packet: E2EPacket) {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.incoming.push_back(packet);
    }

    pub fn push_incoming_untagged(&self, message: E2EMessage) {
        self.push_incoming(E2EPacket::untagged(message));
    }

    pub fn pop_outgoing(&self) -> Option<E2EPacket> {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.outgoing.pop_front()
    }

    pub fn drain_outgoing(&self) -> Vec<E2EPacket> {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.outgoing.drain(..).collect()
    }

    pub fn incoming_len(&self) -> usize {
        let inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.incoming.len()
    }

    pub fn outgoing_len(&self) -> usize {
        let inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.outgoing.len()
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
            MockMessageWriter { inner: self.inner },
        )
    }
}

#[async_trait]
impl E2EMessageWriter for MockMessageWriter {
    async fn send(&mut self, packet: E2EPacket) -> Result<(), MessageStreamError> {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.outgoing.push_back(packet);
        Ok(())
    }
}

#[async_trait]
impl E2EMessageReader for MockMessageReader {
    async fn receive(&mut self) -> Result<E2EPacket, MessageStreamError> {
        let mut inner = self.inner.lock().expect("mock stream mutex poisoned");
        inner.incoming.pop_front().ok_or_else(|| {
            MessageStreamError::new(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "mock message stream incoming queue is empty",
            ))
        })
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
