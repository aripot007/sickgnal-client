use std::{
    io::ErrorKind,
    pin::{Pin, pin},
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};

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
    /// Create a new [`TlsStream`].
    ///
    /// The handshake should be finished before giving the stream to the client
    #[inline]
    pub(crate) fn new(connection: Connection, stream: S) -> Self {
        Self { connection, stream }
    }

    pub(crate) fn inner(&mut self) -> &mut S {
        &mut self.stream
    }
}

impl<S> AsyncRead for TlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = Pin::into_inner(self);

        // Read data from the socket if we need to
        while this.connection.wants_read() {
            // TODO: check empty reads ?

            // Scope the pinned future so we can borrow `this` as mutable after
            {
                let read_fut = this.connection.read_tls(&mut this.stream);
                let mut pinned_fut = pin!(read_fut);

                match pinned_fut.as_mut().poll(cx) {
                    // Reached EOF
                    Poll::Ready(Ok(0)) => {
                        return Poll::Ready(Ok(()));
                    }

                    // Process the packets
                    Poll::Ready(Ok(_nread)) => (),

                    Poll::Ready(Err(err)) => {
                        return Poll::Ready(Err(std::io::Error::new(ErrorKind::InvalidData, err)));
                    }

                    // The inner stream is not ready yet
                    Poll::Pending => return Poll::Pending,
                }
            }

            if let Err(e) = this.connection.process_new_packets() {
                // error processing the packets
                let err = std::io::Error::new(ErrorKind::InvalidData, e);
                return Poll::Ready(Err(err));
            }
        }

        let dest = buf.initialize_unfilled();
        let nread = this.connection.read(dest)?;
        buf.advance(nread);

        Poll::Ready(Ok(()))
    }
}

impl<S> AsyncWrite for TlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.connection.write(buf);

        match self.poll_flush(cx) {
            Poll::Ready(_) => Poll::Ready(Ok(buf.len())),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = Pin::into_inner(self);

        let write_fut = this.connection.write_tls(&mut this.stream);
        let mut pinned_fut = pin!(write_fut);

        match pinned_fut.as_mut().poll(cx) {
            Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
            Poll::Ready(Err(err)) => {
                Poll::Ready(Err(std::io::Error::new(ErrorKind::InvalidData, err)))
            }

            // The inner stream is not ready yet
            Poll::Pending => Poll::Pending,
        }
    }

    // TODO: implement clean shutdown
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        todo!()
    }
}
