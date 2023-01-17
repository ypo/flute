use crate::tools::error::{FluteError, Result};
use base64::Engine;
use quick_xml::de::from_reader;
use serde::{Deserialize, Serialize};

use super::oti::{self, RaptorQSchemeSpecific, ReedSolomonGF2MSchemeSpecific};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct FdtInstance {
    #[serde(rename = "xmlns:xsi")]
    pub xmlns_xsi: String,
    #[serde(rename = "xmlns:schemaLocation")]
    pub xsi_schema_location: String,
    // An FDT Instance is valid until its expiration time.  The
    //  expiration time is expressed within the FDT Instance payload as a
    //  UTF-8 decimal representation of a 32-bit unsigned integer.  The
    //  value of this integer represents the 32 most significant bits of a
    //  64-bit Network Time Protocol (NTP) [RFC5905] time value
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
    pub file: Option<Vec<File>>,
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

fn reed_solomon_scheme_specific(
    fec_oti_scheme_specific_info: &Option<String>,
) -> Result<Option<ReedSolomonGF2MSchemeSpecific>> {
    if fec_oti_scheme_specific_info.is_none() {
        return Ok(None);
    }

    let info = base64::engine::general_purpose::STANDARD
        .decode(fec_oti_scheme_specific_info.as_ref().unwrap())
        .map_err(|_| FluteError::new("Fail to decode base64 specific scheme"))?;

    if info.len() != 2 {
        return Err(FluteError::new("Wrong size of Scheme-Specific-Info"));
    }

    Ok(Some(ReedSolomonGF2MSchemeSpecific {
        m: info[0],
        g: info[1],
    }))
}

fn _raptorq_scheme_specific(
    fec_oti_scheme_specific_info: &Option<String>,
) -> Result<Option<RaptorQSchemeSpecific>> {
    if fec_oti_scheme_specific_info.is_none() {
        return Ok(None);
    }

    let info = base64::engine::general_purpose::STANDARD
        .decode(fec_oti_scheme_specific_info.as_ref().unwrap())
        .map_err(|_| FluteError::new("Fail to decode base64 specific scheme"))?;

    if info.len() != 4 {
        return Err(FluteError::new("Wrong size of Scheme-Specific-Info"));
    }

    Ok(Some(RaptorQSchemeSpecific {
        source_blocks_length: info[0],
        sub_blocks_length: u16::from_be_bytes(info[1..3].as_ref().try_into().unwrap()),
        symbol_alignment: info[3],
    }))
}

impl FdtInstance {
    pub fn parse(buffer: &[u8]) -> Result<FdtInstance> {
        let instance: Result<FdtInstance> =
            from_reader(buffer).map_err(|err| FluteError::new(err.to_string()));
        instance
    }

    pub fn get_file(&self, toi: &u128) -> Option<&File> {
        let toi = toi.to_string();
        self.file
            .as_ref()
            .and_then(|file| file.into_iter().find(|&file| file.toi == toi))
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

        let reed_solomon_scheme_specific = match fec_encoding_id {
            oti::FECEncodingID::ReedSolomonGF2M => {
                reed_solomon_scheme_specific(&self.fec_oti_scheme_specific_info).unwrap_or(None)
            }
            _ => None,
        };

        /*
        let raptorq_scheme_specific = match fec_encoding_id {
            oti::FECEncodingID::RaptorQ => {
                reed_solomon_scheme_specific(&self.fec_oti_scheme_specific_info).unwrap_or(None)
            }
            _ => None,
        };
        */

        Some(oti::Oti {
            fec_encoding_id: fec_encoding_id,
            fec_instance_id: self.fec_oti_fec_instance_id.unwrap() as u16,
            maximum_source_block_length: self.fec_oti_maximum_source_block_length.unwrap() as u32,
            encoding_symbol_length: self.fec_oti_encoding_symbol_length.unwrap() as u16,
            max_number_of_parity_symbols: (self.fec_oti_max_number_of_encoding_symbols.unwrap()
                - self.fec_oti_maximum_source_block_length.unwrap())
                as u32,
            reed_solomon_scheme_specific: reed_solomon_scheme_specific,
            raptorq_scheme_specific: None,
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

        let reed_solomon_scheme_specific = match fec_encoding_id {
            oti::FECEncodingID::ReedSolomonGF2M => {
                reed_solomon_scheme_specific(&self.fec_oti_scheme_specific_info).unwrap_or(None)
            }
            _ => None,
        };

        Some(oti::Oti {
            fec_encoding_id: fec_encoding_id,
            fec_instance_id: self.fec_oti_fec_instance_id.unwrap() as u16,
            maximum_source_block_length: self.fec_oti_maximum_source_block_length.unwrap() as u32,
            encoding_symbol_length: self.fec_oti_encoding_symbol_length.unwrap() as u16,
            max_number_of_parity_symbols: (self.fec_oti_max_number_of_encoding_symbols.unwrap()
                - self.fec_oti_maximum_source_block_length.unwrap())
                as u32,
            reed_solomon_scheme_specific: reed_solomon_scheme_specific,
            raptorq_scheme_specific: None,
            inband_oti: false,
        })
    }
}
