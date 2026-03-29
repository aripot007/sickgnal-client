//! Utility reader for parsing incoming data

use std::mem;

use crate::error::InvalidMessage;

/// A non-consuming reader over a slice
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    /// Create a new Reader from an underlying buffer
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Take exacly `count` bytes from the buffer
    ///
    /// Returns a [`InvalidMessage::TooShort`] if the buffer does not contain
    /// enough bytes
    pub fn take(&mut self, count: usize) -> Result<&'a [u8], InvalidMessage> {
        let (consumed, remaining) = self
            .buf
            .split_at_checked(count)
            .ok_or(InvalidMessage::TooShort)?;

        self.buf = remaining;
        Ok(consumed)
    }

    /// Take all the bytes available in the buffer.
    ///
    /// Returns an empty slice if the buffer is empty.
    #[inline]
    pub fn take_all(&mut self) -> &'a [u8] {
        mem::take(&mut self.buf)
    }

    /// Take a single byte from the buffer
    pub fn take_byte(&mut self) -> Result<u8, InvalidMessage> {
        Ok(self.take(1)?[0])
    }

    /// Returns the number of bytes available in this reader
    #[inline]
    pub fn available(&self) -> usize {
        self.buf.len()
    }
}
