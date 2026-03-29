//! Certificate message
//!
//! We only support the X509 certificate type without extensions
//!

use rustls_pki_types::CertificateDer;
use tracing::trace;

use crate::codec::Codec;
use crate::u24::U24;
use crate::{error::InvalidMessage, reader::Reader};

#[derive(Debug)]
pub struct CertificateMessage {
    /// The sever's certificate
    ///
    /// This should be the first entry in the `certificate_list`
    server_cert: CertificateDer<'static>,

    /// The certification chain to verify the server's certificate
    certification_path: Vec<CertificateDer<'static>>,
}

impl CertificateMessage {
    pub fn decode(reader: &mut Reader) -> Result<Self, InvalidMessage> {
        // certificate_request_context should be empty for the server
        let context_len = u8::decode(reader)?;

        if context_len != 0 {
            return Err(InvalidMessage::IllegalParameter);
        }

        let certificate_list_len = U24::decode(reader)?.0 as usize;
        // decode the server certificate
        let server_cert = decode_cert_entry(reader)?;

        // decode the certification chain
        let mut certification_path = Vec::new();

        while !reader.is_empty() {
            let cert = decode_cert_entry(reader)?;
            certification_path.push(cert);
        }

        Ok(CertificateMessage {
            server_cert,
            certification_path,
        })
    }
}

/// Decode a CertificateEntry
///
/// Expects a X509 entry with no extension
fn decode_cert_entry(reader: &mut Reader) -> Result<CertificateDer<'static>, InvalidMessage> {
    let data_len = U24::decode_for("cert_len", reader)?.0 as usize;

    let cert = CertificateDer::from(reader.take_for("cert", data_len)?);

    // extensions should be empty
    let exts_len = u16::decode(reader)?;
    if exts_len != 0 {
        return Err(InvalidMessage::UnsupportedExtension);
    }

    Ok(cert.into_owned())
}
