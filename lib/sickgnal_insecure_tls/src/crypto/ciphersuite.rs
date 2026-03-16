use crate::{codec::Codec, error::InvalidMessage, reader::Reader};

#[derive(Debug, Clone)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum CipherSuite {
    TLS_AES_128_GCM_SHA256,
    TLS_AES_256_GCM_SHA384,
    TLS_CHACHA20_POLY1305_SHA256,
    TLS_AES_128_CCM_SHA256,
    TLS_AES_128_CCM_8_SHA256,
}

impl Codec for CipherSuite {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.push(0x13);
        dest.push(match self {
            CipherSuite::TLS_AES_128_GCM_SHA256 => 0x01,
            CipherSuite::TLS_AES_256_GCM_SHA384 => 0x02,
            CipherSuite::TLS_CHACHA20_POLY1305_SHA256 => 0x03,
            CipherSuite::TLS_AES_128_CCM_SHA256 => 0x04,
            CipherSuite::TLS_AES_128_CCM_8_SHA256 => 0x05,
        });
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        // First byte should always be 0x13
        if buf.take_byte()? != 0x13 {
            return Err(InvalidMessage::InvalidCipherSuite);
        }

        let val = u8::decode(buf)?;
        Ok(match val {
            0x01 => CipherSuite::TLS_AES_128_GCM_SHA256,
            0x02 => CipherSuite::TLS_AES_256_GCM_SHA384,
            0x03 => CipherSuite::TLS_CHACHA20_POLY1305_SHA256,
            0x04 => CipherSuite::TLS_AES_128_CCM_SHA256,
            0x05 => CipherSuite::TLS_AES_128_CCM_8_SHA256,
            _ => return Err(InvalidMessage::InvalidCipherSuite),
        })
    }
}
