use crate::tools::error::{FluteError, Result};
use quick_xml::de::from_reader;
use serde::{Deserialize, Serialize};

use super::oti;

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

    pub fn get_file(&self, toi: &u128) -> Option<&File> {
        let toi = toi.to_string();
        self.file.iter().find(|file| {
            file.toi == toi
        })
    }

    pub fn get_oti_for_file(&self, file: &File) -> Option<oti::Oti> {
        let oti = file.get_oti();
        if oti.is_some() {
            return oti;
        }

        self.get_oti()
    }

    pub fn get_oti(&self) -> Option<oti::Oti> {
        if self.fec_oti_fec_encoding_id.is_none()
            || self.fec_oti_fec_instance_id.is_none()
            || self.fec_oti_maximum_source_block_length.is_none()
            || self.fec_oti_encoding_symbol_length.is_none()
            || self.fec_oti_max_number_of_encoding_symbols.is_none()
        {
            return None;
        }
        let fec_encoding_id: oti::FECEncodingID =
            self.fec_oti_fec_encoding_id.unwrap().try_into().ok()?;
        Some(oti::Oti {
            fec_encoding_id: fec_encoding_id,
            fec_instance_id: self.fec_oti_fec_instance_id.unwrap() as u16,
            maximum_source_block_length: self.fec_oti_maximum_source_block_length.unwrap() as u32,
            encoding_symbol_length: self.fec_oti_encoding_symbol_length.unwrap() as u16,
            max_number_of_parity_symbols: (self.fec_oti_max_number_of_encoding_symbols.unwrap()
                - self.fec_oti_maximum_source_block_length.unwrap())
                as u32,
            reed_solomon_m: None, // TODO read fec_oti_scheme_specific_info to decode scheme info
            inband_oti: false,
        })
    }
}

impl File {
    pub fn get_oti(&self) -> Option<oti::Oti> {
        if self.fec_oti_fec_encoding_id.is_none()
            || self.fec_oti_fec_instance_id.is_none()
            || self.fec_oti_maximum_source_block_length.is_none()
            || self.fec_oti_encoding_symbol_length.is_none()
            || self.fec_oti_max_number_of_encoding_symbols.is_none()
        {
            return None;
        }
        let fec_encoding_id: oti::FECEncodingID =
            self.fec_oti_fec_encoding_id.unwrap().try_into().ok()?;
        Some(oti::Oti {
            fec_encoding_id: fec_encoding_id,
            fec_instance_id: self.fec_oti_fec_instance_id.unwrap() as u16,
            maximum_source_block_length: self.fec_oti_maximum_source_block_length.unwrap() as u32,
            encoding_symbol_length: self.fec_oti_encoding_symbol_length.unwrap() as u16,
            max_number_of_parity_symbols: (self.fec_oti_max_number_of_encoding_symbols.unwrap()
                - self.fec_oti_maximum_source_block_length.unwrap())
                as u32,
            reed_solomon_m: None, // TODO read fec_oti_scheme_specific_info to decode scheme info
            inband_oti: false,
        })
    }
}
