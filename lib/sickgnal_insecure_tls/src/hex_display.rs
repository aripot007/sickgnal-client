//! Utility hex printing functions

use std::fmt::{Debug, Display, LowerHex, UpperHex};

/// Display bytes in hexadecimal
pub(crate) struct HexDisplay<'a>(pub &'a [u8]);

macro_rules! display_impl {
    ($trait:ty, $fmt:literal) => {
        impl<'a> $trait for HexDisplay<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                for byte in self.0 {
                    write!(f, $fmt, byte)?;
                }
                Ok(())
            }
        }
    };
}

display_impl! {Display, "{:02x}"}
display_impl! {Debug, "{:02x}"}
display_impl! {UpperHex, "{:02X}"}
display_impl! {LowerHex, "{:02x}"}

/// Display bytes in a pretty hexadecimal with spaces
pub(crate) struct PrettyHexDisplay<'a>(pub &'a [u8]);

macro_rules! pretty_display_impl {
    ($trait:ty, $fmt:literal) => {
        impl<'a> $trait for PrettyHexDisplay<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                for byte in &self.0[..self.0.len() - 1] {
                    write!(f, concat!($fmt, " "), byte)?;
                }

                if let Some(byte) = self.0.last() {
                    write!(f, $fmt, byte)?;
                }
                Ok(())
            }
        }
    };
}

pretty_display_impl! {Display, "{:02x}"}
pretty_display_impl! {Debug, "{:02x}"}
pretty_display_impl! {UpperHex, "{:02X}"}
pretty_display_impl! {LowerHex, "{:02x}"}

/// Utility trait to easily construct [`HexDisplay`] and [`PrettyHexDisplay`]
pub(crate) trait HexDisplayExt {
    /// Display as a hex string
    fn hex(&self) -> HexDisplay<'_>;
    /// Display as a space-separated hex string
    fn pretty_hex(&self) -> PrettyHexDisplay<'_>;
}

impl<T> HexDisplayExt for T
where
    T: AsRef<[u8]>,
{
    fn hex(&self) -> HexDisplay<'_> {
        HexDisplay(self.as_ref())
    }

    fn pretty_hex(&self) -> PrettyHexDisplay<'_> {
        PrettyHexDisplay(self.as_ref())
    }
}
