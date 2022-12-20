use crate::tools::error::{FluteError, Result};
use quick_xml::de::from_reader;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct FdtInstance {
    #[serde(rename = "xmlns:xsi")]
    pub xmlns_xsi: String,
    #[serde(rename = "xmlns:schemaLocation")]
    pub xsi_schema_location: String,
    #[serde(rename = "Expires")]
    pub expires: String,
    #[serde(rename = "Complete")]
    pub complete: Option<bool>,
    #[serde(rename = "Content-Type")]
    pub content_type: Option<String>,
    #[serde(rename = "Content-Encoding")]
    pub content_encoding: Option<String>,
    #[serde(rename = "FEC-OTI-FEC-Encoding-ID")]
    pub fec_oti_fec_encoding_id: Option<u8>,
    #[serde(rename = "FEC-OTI-FEC-Instance-ID")]
    pub fec_oti_fec_instance_id: Option<u64>,
    #[serde(rename = "FEC-OTI-Maximum-Source-Block-Length")]
    pub fec_oti_maximum_source_block_length: Option<u64>,
    #[serde(rename = "FEC-OTI-Encoding-Symbol-Length")]
    pub fec_oti_encoding_symbol_length: Option<u64>,
    #[serde(rename = "FEC-OTI-Max-Number-of-Encoding-Symbols")]
    pub fec_oti_max_number_of_encoding_symbols: Option<u64>,
    #[serde(rename = "FEC-OTI-Scheme-Specific-Info")]
    pub fec_oti_scheme_specific_info: Option<String>, // Base64
    #[serde(rename = "File")]
    pub file: Vec<File>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct File {
    #[serde(rename = "Content-Location")]
    pub content_location: String,
    #[serde(rename = "TOI")]
    pub toi: String,
    #[serde(rename = "Content-Length")]
    pub content_length: Option<u64>,
    #[serde(rename = "Transfer-Length")]
    pub transfer_length: Option<u64>,
    #[serde(rename = "Content-Type")]
    pub content_type: Option<String>,
    #[serde(rename = "Content-Encoding")]
    pub content_encoding: Option<String>,
    #[serde(rename = "Content-MD5")]
    pub content_md5: Option<String>,
    #[serde(rename = "FEC-OTI-FEC-Encoding-ID")]
    pub fec_oti_fec_encoding_id: Option<u8>,
    #[serde(rename = "FEC-OTI-FEC-Instance-ID")]
    pub fec_oti_fec_instance_id: Option<u64>,
    #[serde(rename = "FEC-OTI-Maximum-Source-Block-Length")]
    pub fec_oti_maximum_source_block_length: Option<u64>,
    #[serde(rename = "FEC-OTI-Encoding-Symbol-Length")]
    pub fec_oti_encoding_symbol_length: Option<u64>,
    #[serde(rename = "FEC-OTI-Max-Number-of-Encoding-Symbols")]
    pub fec_oti_max_number_of_encoding_symbols: Option<u64>,
    #[serde(rename = "FEC-OTI-Scheme-Specific-Info")]
    pub fec_oti_scheme_specific_info: Option<String>, // Base64
}

impl FdtInstance {
    pub fn parse(buffer: &[u8]) -> Result<FdtInstance> {
        let instance: Result<FdtInstance> =
            from_reader(buffer).map_err(|err| FluteError::new(err.to_string()));
        instance
    }
}
