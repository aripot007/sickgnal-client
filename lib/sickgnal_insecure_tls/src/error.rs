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

    /// When we don't support a protocol version
    UnsupportedProtocolVersion,

    /// When we don't support or recognise an extension
    UnsupportedExtension,

    /// When we don't support a named group
    UnsupportedNamedGroup,

    /// When the content type is not known
    InvalidContentType,

    /// When the handshake type is invalid
    InvalidHandshakeType,

    /// When the cipher suite is invalid
    InvalidCipherSuite,

    /// When we receive an invalid [`NamedGroup`](crate::crypto::NamedGroup)
    InvalidNamedGroup,

    /// When we receive a parameter with an invalid value
    IllegalParameter,

    /// When a record is too long
    RecordOverflow,
}

impl From<InvalidMessage> for Error {
    fn from(value: InvalidMessage) -> Self {
        Error::InvalidMessage(value)
    }
}
