use crate::macros::codec_enum;

codec_enum! {

    /// A cipher suite that can be used in TLS
    pub struct CipherSuite(u16);

    #[allow(non_camel_case_types)]
    pub enum CipherSuiteName {
        TLS_AES_128_GCM_SHA256 = 0x1301,
        TLS_AES_256_GCM_SHA384 = 0x1302,
        TLS_CHACHA20_POLY1305_SHA256 = 0x1303,
        TLS_AES_128_CCM_SHA256 = 0x1304,
        TLS_AES_128_CCM_8_SHA256 = 0x1305,
    }
}
