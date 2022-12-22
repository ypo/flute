use serde::Serialize;

///
/// FEC Type
/// FECEncodingID < 128 Fully-Specified FEC
/// FECEncodingID >= 128 Under-Specified
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FECEncodingID {
    NoCode = 0,
    ReedSolomonGF2M = 2,
    ReedSolomonGF28 = 5,
    ReedSolomonGF28SmallBlockSystematic = 129,
    // LDPCStaircase = 3,
}

impl TryFrom<u8> for FECEncodingID {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            x if x == FECEncodingID::NoCode as u8 => Ok(FECEncodingID::NoCode),
            x if x == FECEncodingID::ReedSolomonGF28SmallBlockSystematic as u8 => {
                Ok(FECEncodingID::ReedSolomonGF28SmallBlockSystematic)
            }
            x if x == FECEncodingID::ReedSolomonGF2M as u8 => Ok(FECEncodingID::ReedSolomonGF2M),
            x if x == FECEncodingID::ReedSolomonGF28 as u8 => Ok(FECEncodingID::ReedSolomonGF28),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Oti {
    pub fec_encoding_id: FECEncodingID,
    /// FEC Instance ID for Under-Specified spec
    pub fec_instance_id: u16,
    pub maximum_source_block_length: u32,
    pub encoding_symbol_length: u16,
    pub max_number_of_parity_symbols: u32,
    pub reed_solomon_m: Option<u8>,
    pub inband_oti: bool,
}

impl Default for Oti {
    fn default() -> Self {
        Oti {
            fec_encoding_id: FECEncodingID::NoCode,
            fec_instance_id: 0,
            maximum_source_block_length: 64,
            encoding_symbol_length: 1424,
            max_number_of_parity_symbols: 2,
            reed_solomon_m: None,
            inband_oti: true,
        }
    }
}

impl Oti {
    pub fn get_attributes(&self) -> OtiAttributes {
        OtiAttributes {
            fec_oti_fec_encoding_id: Some(self.fec_encoding_id as u8),
            fec_oti_fec_instance_id: Some(self.fec_instance_id as u64),
            fec_oti_maximum_source_block_length: Some(self.maximum_source_block_length as u64),
            fec_oti_encoding_symbol_length: Some(self.encoding_symbol_length as u64),
            fec_oti_max_number_of_encoding_symbols: Some(
                self.maximum_source_block_length as u64 + self.max_number_of_parity_symbols as u64,
            ),
            fec_oti_scheme_specific_info: self.scheme_specific_info(),
        }
    }

    fn scheme_specific_info(&self) -> Option<String> {
        if self.fec_encoding_id == FECEncodingID::ReedSolomonGF2M {
            return Some(base64::encode([self.reed_solomon_m.unwrap_or_default()]));
        }
        None
    }
}

#[derive(Debug, PartialEq, Serialize)]
pub struct OtiAttributes {
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
