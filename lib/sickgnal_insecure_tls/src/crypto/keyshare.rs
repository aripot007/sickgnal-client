use crate::{
    codec::{Codec, LengthSize, encode_length_prefixed_vector},
    crypto::NamedGroup,
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

    fn decode(&self, buf: impl std::io::Read) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}
