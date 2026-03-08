use thiserror::Error;
use tokio_rustls::rustls::{self, pki_types::pem};

#[derive(Debug, Error)]
pub enum Error {
    /// Errors that may arise when parsing the contents of a PEM file
    #[error("error parsing PEM file : {0}")]
    Pem(#[from] pem::Error),

    #[error("rustls error : {0}")]
    Rustls(#[from] rustls::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
