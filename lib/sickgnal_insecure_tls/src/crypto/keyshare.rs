use std::fmt::Debug;

use crate::{
    codec::{Decode, Encode},
    crypto::{NamedGroup, NamedGroupName},
    error::InvalidMessage,
    hex_display::HexDisplayExt,
    reader::Reader,
};

#[derive(Clone)]
pub(crate) enum KeyShareEntry {
    X25519(x25519_dalek::PublicKey),
}

impl KeyShareEntry {
    /// Get the [`NamedGroup`] corresponding to this key share
    pub fn named_group(&self) -> NamedGroup {
        match self {
            KeyShareEntry::X25519(_) => NamedGroup::x25519,
        }
    }
}

impl Encode for KeyShareEntry {
    fn encode(&self, dest: &mut Vec<u8>) {
        self.named_group().encode(dest);

        match self {
            // For X25519 and X448, the content is the byte string of the public value
            KeyShareEntry::X25519(public_key) => {
                let bytes = public_key.as_bytes();
                (bytes.len() as u16).encode(dest);
                dest.extend(bytes);
            }
        }
    }
}

impl Decode for KeyShareEntry {
    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        let group: NamedGroupName = NamedGroup::decode(buf)?
            .try_into()
            .map_err(|_| InvalidMessage::InvalidNamedGroup)?;

        match group {
            NamedGroupName::x25519 => {
                let len = u16::decode(buf)?;

                if len != 32 {
                    return Err(InvalidMessage::IllegalParameter);
                }

                let mut bytes = [0; 32];
                bytes.copy_from_slice(buf.take_for("keyshare", len as usize)?);
                let key = x25519_dalek::PublicKey::from(bytes);

                Ok(KeyShareEntry::X25519(key))
            }

            _ => Err(InvalidMessage::UnsupportedNamedGroup),
        }
    }
}

impl Debug for KeyShareEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::X25519(arg0) => f
                .debug_tuple("X25519")
                .field(&arg0.as_bytes().hex())
                .finish(),
        }
    }
}

/// The secret corresponding to a [`KeyShareEntry`]
pub(crate) enum KeyShareSecret {
    X25519(x25519_dalek::EphemeralSecret),
}

impl KeyShareSecret {
    /// Get the [`NamedGroup`] corresponding to this key share
    pub fn named_group(&self) -> NamedGroup {
        match self {
            KeyShareSecret::X25519(_) => NamedGroup::x25519,
        }
    }
}

impl Debug for KeyShareSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::X25519(_) => f.debug_tuple("X25519").finish_non_exhaustive(),
        }
    }
}
