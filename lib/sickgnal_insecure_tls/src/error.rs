use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum Error {
    /// When we receive an invalid TLS message from the peer
    #[error("received an invalid message : {0:?}")]
    InvalidMessage(InvalidMessage),
}

#[derive(Debug, Clone)]
pub enum InvalidMessage {
    /// When the message was shorter than the expected length
    TooShort,

    /// When the protocol version is not known
    UnknownProtocolVersion,

    /// When the content type is not known
    UnknownContentType
}
