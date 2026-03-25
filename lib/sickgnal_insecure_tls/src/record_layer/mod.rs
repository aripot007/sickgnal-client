use crate::macros::codec_enum;

pub mod deframer;
pub mod record;

/// Length of a record header in bytes
pub(self) const RECORD_HEADER_LEN: usize = 5;

/// The maximum length of a TLSPlaintext.fragment
///
/// "The length MUST NOT exceed 2^14 bytes. An
///  endpoint that receives a record that exceeds this length MUST
///  terminate the connection with a "record_overflow" alert."
pub(self) const PLAINTEXT_FRAGMENT_MAX_LEN: u16 = 2 << 14;

/// The maximum length of a TLSCiphertext.fragment
///
/// "The length MUST NOT exceed 2^14 bytes. An
///  endpoint that receives a record that exceeds this length MUST
///  terminate the connection with a "record_overflow" alert."
pub(self) const CIPHERTEXT_FRAGMENT_MAX_LEN: u16 = (2 << 14) + 256;

codec_enum! {

    /// The content type of a record.
    pub struct ContentType(pub u8);

    #[allow(non_camel_case_types)]
    pub enum ContentTypeName {
        // Only defined in the spec to reserve the 0 value since it would
        // prevent padding
        // Invalid = 0,
        ChangeCipherSpec = 20,
        Alert = 21,
        Handshake = 22,
        ApplicationData = 23,
        Heartbeat = 24, /* RFC 6520 */
    }
}
