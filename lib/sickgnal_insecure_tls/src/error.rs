use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum Error {
    /// When we receive an invalid TLS message from the peer
    #[error("received an invalid message : {0:?}")]
    InvalidMessage(InvalidMessage),

    #[error("io error : {0}")]
    IoError(std::io::ErrorKind),

    /// When we are trying to do something invalid for the state we are in
    #[error("invalid state")]
    InvalidState,

    /// When we got an error during the handshake and still try
    /// to use the [`Connection`]
    ///
    /// [`Connection`]: crate::connection::Connection
    #[error("invalid state : unfinished handshake")]
    UnfinishedHandshake,

    /// When we receive a HelloRetryRequest, which we don't support yet
    #[error("hello_retry_request are not supported yet")]
    UnsupportedHelloRetryRequest,
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IoError(error.kind())
    }
}

#[derive(Debug, Clone)]
pub enum InvalidMessage {
    /// We received an unexpected message
    UnexpectedMessage,

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

    /// We received an unencrypted message while record protection was enabled
    UnencryptedMessage,

    /// When the content type is not known
    InvalidContentType,

    /// When the handshake type is invalid
    InvalidHandshakeType,

    /// When the cipher suite is invalid
    InvalidCipherSuite,

    /// When we receive an invalid [`NamedGroup`](crate::crypto::NamedGroup)
    InvalidNamedGroup,

    /// When we receive a ChangeCipherSpec message with an invalid value
    InvalidChangeCipherSpec,

    /// When we receive a parameter with an invalid value
    IllegalParameter,

    /// When a record is too long
    RecordOverflow,

    /// When we are missing an extension in the handshake
    MissingExtension,

    /// When we failed to decrypt a message
    BadMacError,
}

impl From<InvalidMessage> for Error {
    fn from(value: InvalidMessage) -> Self {
        Error::InvalidMessage(value)
    }
}
