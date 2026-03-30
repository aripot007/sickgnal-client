//! Certificate message
//!
//! We only support the X509 certificate type without extensions
//!

use std::fmt::Debug;

use rustls_pki_types::CertificateDer;

use crate::codec::Decode;
use crate::crypto::{SignatureScheme, SignatureSchemeName};
use crate::hex_display::HexDisplayExt;
use crate::u24::U24;
use crate::{error::InvalidMessage, reader::Reader};

#[derive(Debug)]
pub struct CertificateMessage {
    /// The sever's certificate
    ///
    /// This should be the first entry in the `certificate_list`
    pub(crate) server_cert: CertificateDer<'static>,

    /// The certification chain to verify the server's certificate
    pub(crate) certification_path: Vec<CertificateDer<'static>>,
}

impl Decode for CertificateMessage {
    fn decode(reader: &mut Reader) -> Result<Self, InvalidMessage> {
        // certificate_request_context should be empty for the server
        let context_len = u8::decode(reader)?;

        if context_len != 0 {
            return Err(InvalidMessage::IllegalParameter);
        }

        let certificate_list_len = U24::decode(reader)?.0 as usize;

        if certificate_list_len == 0 {
            return Err(InvalidMessage::EmptyCertificates);
        }

        let payload = reader.take_for("certificate_list", certificate_list_len)?;
        let mut reader = Reader::new(payload);

        // decode the server certificate
        let server_cert = decode_cert_entry(&mut reader)?;

        // decode the certification chain
        let mut certification_path = Vec::new();

        while !reader.is_empty() {
            let cert = decode_cert_entry(&mut reader)?;
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

pub struct CertificateVerify {
    pub(crate) algorithm: SignatureSchemeName,
    pub(crate) signature: Vec<u8>,
}

impl Decode for CertificateVerify {
    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        let algorithm = SignatureScheme::decode(buf)?;

        let algorithm = SignatureSchemeName::try_from(algorithm)
            .map_err(|_| InvalidMessage::InvalidSignatureScheme)?;

        let sig_length = u16::decode(buf)?;

        let sig = buf.take_for("CertificateVerify", sig_length as usize)?;

        Ok(CertificateVerify {
            algorithm,
            signature: Vec::from(sig),
        })
    }
}

impl Debug for CertificateVerify {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertificateVerify")
            .field("algorithm", &self.algorithm)
            .field("signature", &self.signature.hex())
            .finish()
    }
}
