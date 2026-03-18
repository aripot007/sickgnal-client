use crate::{
    codec::Codec,
    crypto::{NamedGroup, NamedGroupName},
    error::InvalidMessage,
    reader::Reader,
};

#[derive(Debug, Clone)]
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

impl Codec for KeyShareEntry {
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
                bytes.copy_from_slice(buf.take(len as usize)?);
                let key = x25519_dalek::PublicKey::from(bytes);

                Ok(KeyShareEntry::X25519(key))
            }

            _ => Err(InvalidMessage::UnsupportedNamedGroup),
        }
    }
}
