use tokio::io::{AsyncReadExt, AsyncWrite};

use crate::connection::Connection;

#[derive(Debug)]
pub struct TlsStream<S>
where
    S: AsyncReadExt + AsyncWrite,
{
    connection: Connection,
    stream: S,
}

impl<S> TlsStream<S>
where
    S: AsyncReadExt + AsyncWrite,
{
    #[inline]
    pub(crate) fn new(connection: Connection, stream: S) -> Self {
        Self { connection, stream }
    }

    pub(crate) fn inner(&mut self) -> &mut S {
        &mut self.stream
    }
}
