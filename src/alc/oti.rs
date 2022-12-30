use serde::Serialize;

///
/// FEC Type
/// FECEncodingID < 128 Fully-Specified FEC  
/// FECEncodingID >= 128 Under-Specified  
///
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FECEncodingID {
    /// No FEC
    NoCode = 0,
    /// Reed Solomon GF2M
    ReedSolomonGF2M = 2,
    /// Reed Solomon GF28
    ReedSolomonGF28 = 5,
    // RaptorQ = 6,
    /// Reed Solomon GF28, building block Small Block Systematic
    ReedSolomonGF28SmallBlockSystematic = 129,
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

///
/// Reed Solomon GS2M Scheme Specific parameters
#[derive(Clone, Debug)]
pub struct ReedSolomonGF2MSchemeSpecific {
    /// Length of the finite field elements, in bits
    pub m: u8,
    /// number of encoding symbols per group used for the object
    /// The default value is 1, meaning that each packet contains exactly one symbol
    pub g: u8,
}

///
/// FEC Object Transmission Information
/// Contains the parameters using the build the blocks and FEC for the objects transmission
#[derive(Clone, Debug)]
pub struct Oti {
    /// Select the FEC for the object transmission
    pub fec_encoding_id: FECEncodingID,
    /// FEC Instance ID for Under-Specified spec (`FECEncodingID` > 0)
    /// Should be 0 for `FECEncodingID::ReedSolomonGF28SmallBlockSystematic`
    pub fec_instance_id: u16,
    /// Maximum number of encoding symbol per block
    pub maximum_source_block_length: u32,
    /// Size (in bytes) of an encoding symbol
    pub encoding_symbol_length: u16,
    /// Maximum number of repairing symbols (FEC)
    pub max_number_of_parity_symbols: u32,
    /// Optional, only if `fec_encoding_id` is `FECEncodingID::ReedSolomonGF2M`
    pub reed_solomon_scheme_specific: Option<ReedSolomonGF2MSchemeSpecific>,
    /// If `true`, OTI is added to every ALC/LCT packets
    /// If `false`, OTI is only available inside the FDT
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
            reed_solomon_scheme_specific: None,
            inband_oti: true,
        }
    }
}

impl Default for ReedSolomonGF2MSchemeSpecific {
    fn default() -> Self {
        ReedSolomonGF2MSchemeSpecific { m: 8, g: 1 }
    }
}

impl Oti {
    /// Convert `Oti` to `OtiAttributes`
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
            let data = match self.reed_solomon_scheme_specific.as_ref() {
                Some(info) => Some(vec![info.m, info.g]),
                None => None,
            };

            return data.map(|d| base64::encode(d));
        }
        None
    }
}

/// Oti Attributes that can be serialized to XML
#[derive(Debug, PartialEq, Serialize)]
pub struct OtiAttributes {
    /// See [rfc6726 Section 5](https://www.rfc-editor.org/rfc/rfc6726.html#section-5)
    #[serde(rename = "FEC-OTI-FEC-Encoding-ID")]
    pub fec_oti_fec_encoding_id: Option<u8>,
    /// See [rfc6726 Section 5](https://www.rfc-editor.org/rfc/rfc6726.html#section-5)
    #[serde(rename = "FEC-OTI-FEC-Instance-ID")]
    pub fec_oti_fec_instance_id: Option<u64>,
    /// See [rfc6726 Section 5](https://www.rfc-editor.org/rfc/rfc6726.html#section-5)
    #[serde(rename = "FEC-OTI-Maximum-Source-Block-Length")]
    pub fec_oti_maximum_source_block_length: Option<u64>,
    /// See [rfc6726 Section 5](https://www.rfc-editor.org/rfc/rfc6726.html#section-5)
    #[serde(rename = "FEC-OTI-Encoding-Symbol-Length")]
    pub fec_oti_encoding_symbol_length: Option<u64>,
    /// See [rfc6726 Section 5](https://www.rfc-editor.org/rfc/rfc6726.html#section-5)
    #[serde(rename = "FEC-OTI-Max-Number-of-Encoding-Symbols")]
    pub fec_oti_max_number_of_encoding_symbols: Option<u64>,
    /// See [rfc6726 Section 5](https://www.rfc-editor.org/rfc/rfc6726.html#section-5)
    #[serde(rename = "FEC-OTI-Scheme-Specific-Info")]
    pub fec_oti_scheme_specific_info: Option<String>, // Base64
}
