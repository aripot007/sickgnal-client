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
    /// Returns `None` if the buffer does not contain
    /// enough bytes
    pub fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let (consumed, remaining) = self.buf.split_at_checked(count)?;

        self.buf = remaining;
        Some(consumed)
    }

    /// Convenience function for [`Self::take`] to return an [`InvalidMessage::TooShortFor`] error
    /// instead of `None`
    #[inline]
    pub fn take_for(
        &mut self,
        name: &'static str,
        count: usize,
    ) -> Result<&'a [u8], InvalidMessage> {
        self.take(count).ok_or(InvalidMessage::TooShortFor(name))
    }

    /// Take all the bytes available in the buffer.
    ///
    /// Returns an empty slice if the buffer is empty.
    #[inline]
    pub fn take_all(&mut self) -> &'a [u8] {
        mem::take(&mut self.buf)
    }

    /// Take a single byte from the buffer
    pub fn take_byte(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    /// Convenience function for [`Self::take_byte`] to return an [`InvalidMessage::TooShortFor`] error
    /// instead of `None`
    #[inline]
    pub fn take_byte_for(&mut self, name: &'static str) -> Result<u8, InvalidMessage> {
        self.take_byte().ok_or(InvalidMessage::TooShortFor(name))
    }

    /// Returns the number of bytes available in this reader
    #[inline]
    pub fn available(&self) -> usize {
        self.buf.len()
    }
}
