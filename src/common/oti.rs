use crate::tools::error::{FluteError, Result};
use base64::Engine;
use serde::Serialize;

///
/// FEC Type
/// FECEncodingID < 128 Fully-Specified FEC  
/// FECEncodingID >= 128 Under-Specified  
///
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum FECEncodingID {
    /// No FEC
    NoCode = 0,
    /// Raptor
    Raptor = 1,
    /// Reed Solomon GF2M
    ReedSolomonGF2M = 2,
    /// Reed Solomon GF28
    ReedSolomonGF28 = 5,
    /// RaptorQ
    RaptorQ = 6,
    /// Reed Solomon GF28, under specified Small Block Systematic
    ReedSolomonGF28UnderSpecified = 129,
}

impl TryFrom<u8> for FECEncodingID {
    type Error = ();

    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            x if x == FECEncodingID::NoCode as u8 => Ok(FECEncodingID::NoCode),
            x if x == FECEncodingID::Raptor as u8 => Ok(FECEncodingID::Raptor),
            x if x == FECEncodingID::ReedSolomonGF28UnderSpecified as u8 => {
                Ok(FECEncodingID::ReedSolomonGF28UnderSpecified)
            }
            x if x == FECEncodingID::ReedSolomonGF2M as u8 => Ok(FECEncodingID::ReedSolomonGF2M),
            x if x == FECEncodingID::ReedSolomonGF28 as u8 => Ok(FECEncodingID::ReedSolomonGF28),
            x if x == FECEncodingID::RaptorQ as u8 => Ok(FECEncodingID::RaptorQ),
            _ => Err(()),
        }
    }
}

///
/// Reed Solomon GS2M Scheme Specific parameters
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ReedSolomonGF2MSchemeSpecific {
    /// Length of the finite field elements, in bits
    pub m: u8,
    /// number of encoding symbols per group used for the object
    /// The default value is 1, meaning that each packet contains exactly one symbol
    pub g: u8,
}

///
/// RaptorQ Scheme Specific parameters
/// <https://www.rfc-editor.org/rfc/rfc6330.html#section-3.3.3>
#[derive(Clone, Debug, Default, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RaptorQSchemeSpecific {
    /// The number of source blocks (Z): 8-bit unsigned integer.  
    pub source_blocks_length: u8,
    /// The number of sub-blocks (N): 16-bit unsigned integer for Raptor.
    pub sub_blocks_length: u16,
    /// A symbol alignment parameter (Al): 8-bit unsigned integer.
    pub symbol_alignment: u8,
}

///
/// Raptor Scheme Specific parameters
/// <https://www.rfc-editor.org/rfc/rfc5053.html#section-3.2.3>
///         0                   1                   2                   3
///0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
///+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///|             Z                 |      N        |       Al      |
///+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RaptorSchemeSpecific {
    /// The number of source blocks (Z): 16-bit unsigned integer.  
    pub source_blocks_length: u16,
    /// The number of sub-blocks (N): 8-bit unsigned integer for Raptor.
    pub sub_blocks_length: u8,
    /// A symbol alignment parameter (Al): 8-bit unsigned integer.
    pub symbol_alignment: u8,
}

impl ReedSolomonGF2MSchemeSpecific {
    pub fn scheme_specific(&self) -> String {
        let data = vec![self.m, self.g];
        base64::engine::general_purpose::STANDARD.encode(data)
    }

    pub fn decode(fec_oti_scheme_specific_info: &str) -> Result<ReedSolomonGF2MSchemeSpecific> {
        let info = base64::engine::general_purpose::STANDARD
            .decode(fec_oti_scheme_specific_info)
            .map_err(|_| FluteError::new("Fail to decode base64 specific scheme"))?;

        if info.len() != 2 {
            return Err(FluteError::new("Wrong size of Scheme-Specific-Info"));
        }

        Ok(ReedSolomonGF2MSchemeSpecific {
            m: info[0],
            g: info[1],
        })
    }
}

impl RaptorQSchemeSpecific {
    pub fn scheme_specific(&self) -> String {
        let mut data: Vec<u8> = Vec::new();
        data.push(self.source_blocks_length);
        data.extend(self.sub_blocks_length.to_be_bytes());
        data.push(self.symbol_alignment);
        base64::engine::general_purpose::STANDARD.encode(data)
    }

    pub fn decode(fec_oti_scheme_specific_info: &str) -> Result<RaptorQSchemeSpecific> {
        let info = base64::engine::general_purpose::STANDARD
            .decode(fec_oti_scheme_specific_info)
            .map_err(|_| FluteError::new("Fail to decode base64 specific scheme"))?;

        if info.len() != 4 {
            return Err(FluteError::new("Wrong size of Scheme-Specific-Info"));
        }

        Ok(RaptorQSchemeSpecific {
            source_blocks_length: info[0],
            sub_blocks_length: u16::from_be_bytes(info[1..3].try_into().unwrap()),
            symbol_alignment: info[3],
        })
    }
}

impl RaptorSchemeSpecific {
    pub fn scheme_specific(&self) -> String {
        let mut data: Vec<u8> = Vec::new();
        data.extend(self.source_blocks_length.to_be_bytes());
        data.push(self.sub_blocks_length);
        data.push(self.symbol_alignment);
        base64::engine::general_purpose::STANDARD.encode(data)
    }

    pub fn decode(fec_oti_scheme_specific_info: &str) -> Result<RaptorSchemeSpecific> {
        let info = base64::engine::general_purpose::STANDARD
            .decode(fec_oti_scheme_specific_info)
            .map_err(|_| FluteError::new("Fail to decode base64 specific scheme"))?;

        if info.len() != 4 {
            return Err(FluteError::new("Wrong size of Scheme-Specific-Info"));
        }

        Ok(RaptorSchemeSpecific {
            source_blocks_length: u16::from_be_bytes(info[0..2].try_into().unwrap()),
            sub_blocks_length: info[2],
            symbol_alignment: info[3],
        })
    }
}

///
/// Scheme Specific information
///
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum SchemeSpecific {
    /// if `fec_encoding_id` is `FECEncodingID::ReedSolomonGF2M`
    ReedSolomon(ReedSolomonGF2MSchemeSpecific),
    /// if `fec_encoding_id` is `FECEncodingID::RaptorQ`
    RaptorQ(RaptorQSchemeSpecific),
    /// if `fec_encoding_id` is `FECEncodingID::Raptor`
    Raptor(RaptorSchemeSpecific),
}

///
/// FEC Object Transmission Information
/// Contains the parameters using the build the blocks and FEC for the objects transmission
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    /// Optional, FEC scheme specific
    pub scheme_specific: Option<SchemeSpecific>,
    /// If `true`, FTI is added to every ALC/LCT packets
    /// If `false`, FTI is only available inside the FDT
    pub inband_fti: bool,
}

impl Default for Oti {
    fn default() -> Self {
        Oti::new_no_code(1424, 64)
    }
}

impl Default for ReedSolomonGF2MSchemeSpecific {
    fn default() -> Self {
        ReedSolomonGF2MSchemeSpecific { m: 8, g: 1 }
    }
}

impl Oti {
    /// Creates and returns an instance of the `Oti` using the Forward Error Correction (FEC) Scheme `NoCode`.
    ///
    /// # Parameters
    ///
    /// * `encoding_symbol_length`: A `u16` value representing the length of an encoding symbol in bytes.
    ///   An encoding symbol is a piece of data that is generated by the FEC Scheme and added to the source block to create a coded block.
    ///   It is the payload of an ALC/LCT packet. The ALC/LCT header plus the encoding symbol length should be less than the maximum transmission unit (MTU).
    ///
    /// * `maximum_source_block_length`: A `u16` value representing the maximum length of a source block in bytes.
    /// A source block is a contiguous portion of the original data that is encoded using the FEC Scheme.  
    pub fn new_no_code(encoding_symbol_length: u16, maximum_source_block_length: u16) -> Oti {
        Oti {
            fec_encoding_id: FECEncodingID::NoCode,
            fec_instance_id: 0,
            maximum_source_block_length: maximum_source_block_length as u32,
            encoding_symbol_length,
            max_number_of_parity_symbols: 0,
            scheme_specific: None,
            inband_fti: true,
        }
    }

    /// Creates and returns an instance of the `Oti` using the Forward Error Correction (FEC) Scheme `ReedSolomonGF28`.
    ///
    /// # Parameters
    ///
    ///   * `encoding_symbol_length`: A `u16` value representing the length of an encoding symbol in bytes.
    ///   An encoding symbol is a piece of data that is generated by the FEC Scheme and added to the source block to create a coded block.
    ///   It is the payload of an ALC/LCT packet. The ALC/LCT header plus the encoding symbol length should be less than the maximum transmission unit (MTU).
    ///
    ///   * `maximum_source_block_length`: A `u8` value representing the maximum length of a source block in bytes.
    ///   A source block is a contiguous portion of the original data that is encoded using the FEC Scheme.
    ///
    ///   * `max_number_of_parity_symbols`: A `u8` value representing the maximum number of parity (repair)
    ///   symbols that can be generated by the FEC Scheme for a given block of data.
    ///
    ///  # Returns
    ///
    /// An instance of the `Oti` struct
    ///     
    /// # Errors
    /// Returns an error if the maximum Encoded Block Length (`maximum_source_block_length` + `max_number_of_parity_symbols`) is greater than `255`.
    ///
    /// # Example
    ///
    /// ```
    /// use flute::core::Oti;
    /// // Files are cut in blocks of 60 source symbols and 4 parity (repair) symbols of 1400 bytes each
    /// let oti = Oti::new_reed_solomon_rs28(1400, 60, 4).unwrap();
    /// ```
    ///
    pub fn new_reed_solomon_rs28(
        encoding_symbol_length: u16,
        maximum_source_block_length: u8,
        max_number_of_parity_symbols: u8,
    ) -> Result<Oti> {
        let encoding_block_length: u32 =
            maximum_source_block_length as u32 + max_number_of_parity_symbols as u32;
        if encoding_block_length > 255 {
            return Err(FluteError::new("Encoding Block Length (Source Block Length + Number of parity symbols) must be <= 255"));
        }

        Ok(Oti {
            fec_encoding_id: FECEncodingID::ReedSolomonGF28,
            fec_instance_id: 0,
            maximum_source_block_length: maximum_source_block_length as u32,
            encoding_symbol_length,
            max_number_of_parity_symbols: max_number_of_parity_symbols as u32,
            scheme_specific: None,
            inband_fti: true,
        })
    }

    /// Creates and returns an instance of the `Oti` using the Forward Error Correction (FEC) Scheme `ReedSolomonGF28UnderSpecified`.
    ///
    /// # Parameters
    ///
    ///   * `encoding_symbol_length`: A `u16` value representing the length of an encoding symbol in bytes.
    ///   An encoding symbol is a piece of data that is generated by the FEC Scheme and added to the source block to create a coded block.
    ///   It is the payload of an ALC/LCT packet. The ALC/LCT header plus the encoding symbol length should be less than the maximum transmission unit (MTU).
    ///
    ///   * `maximum_source_block_length`: A `u16` value representing the maximum length of a source block in bytes.
    ///   A source block is a contiguous portion of the original data that is encoded using the FEC Scheme.
    ///
    ///   * `max_number_of_parity_symbols`: A `u16` value representing the maximum number of parity (repair)
    ///   symbols that can be generated by the FEC Scheme for a given block of data.
    ///
    ///  # Returns
    ///
    /// An instance of the `Oti` struct
    ///     
    /// # Errors
    /// Returns an error if the maximum Encoded Block Length (`maximum_source_block_length` + `max_number_of_parity_symbols`) is greater than `u16::MAX`.
    ///
    /// # Example
    ///
    /// ```
    /// use flute::core::Oti;
    /// // Files are cut in blocks of 60 source symbols and 4 parity (repair) symbols of 1400 bytes each
    /// let oti = Oti::new_reed_solomon_rs28_under_specified(1400, 60, 4).unwrap();
    /// ```
    ///
    pub fn new_reed_solomon_rs28_under_specified(
        encoding_symbol_length: u16,
        maximum_source_block_length: u16,
        max_number_of_parity_symbols: u16,
    ) -> Result<Oti> {
        let encoding_block_length: usize =
            maximum_source_block_length as usize + max_number_of_parity_symbols as usize;
        if encoding_block_length > u16::MAX as usize {
            return Err(FluteError::new("Encoding Block Length (Source Block Length + Number of parity symbols) must be <= u16::MAX"));
        }

        Ok(Oti {
            fec_encoding_id: FECEncodingID::ReedSolomonGF28UnderSpecified,
            fec_instance_id: 0,
            maximum_source_block_length: maximum_source_block_length as u32,
            encoding_symbol_length,
            max_number_of_parity_symbols: max_number_of_parity_symbols as u32,
            scheme_specific: None,
            inband_fti: true,
        })
    }

    /// Creates and returns an instance of the `Oti` using the FEC Scheme `RaptorQ`.
    ///
    /// # Parameters
    ///
    ///   * `encoding_symbol_length`: A `u16` value representing the length of an encoding symbol in bytes.
    ///   An encoding symbol is a piece of data that is generated by the FEC Scheme and added to the source block to create a coded block.
    ///   It is the payload of an ALC/LCT packet. The ALC/LCT header plus the encoding symbol length should be less than the maximum transmission unit (MTU).
    ///
    ///   * `maximum_source_block_length`: A `u16` value representing the maximum length of a source block in bytes.
    ///   A source block is a contiguous portion of the original data that is encoded using the FEC Scheme.
    ///
    ///   * `max_number_of_parity_symbols`: A `u16` value representing the maximum number of parity (repair)
    ///   symbols that can be generated by the FEC Scheme for a given block of data.
    ///
    ///   * `sub_blocks_length`: A `u16` value representing the number of sub-block inside a block.   
    ///      N parameter from <https://www.rfc-editor.org/rfc/rfc6330.html#section-3.3.3>.
    ///
    ///   * `symbol_alignment`: symbol alignment parameter (Al) <https://www.rfc-editor.org/rfc/rfc6330.html#section-3.3.3>.   
    ///      Recommended value is 4.
    ///
    ///  # Returns
    ///
    /// An instance of the `Oti` struct
    ///     
    /// # Errors
    /// Returns an error if the encoding symbols length is not a multiple of al parameter
    ///
    /// # Example
    ///
    /// ```
    /// use flute::core::Oti;
    /// let oti = Oti::new_raptorq(1400, 60, 4, 1, 4).unwrap();
    /// ```
    ///
    pub fn new_raptorq(
        encoding_symbol_length: u16,
        maximum_source_block_length: u16,
        max_number_of_parity_symbols: u16,
        sub_blocks_length: u16,
        symbol_alignment: u8,
    ) -> Result<Oti> {
        if (encoding_symbol_length % symbol_alignment as u16) != 0 {
            return Err(FluteError::new(
                "Encoding symbols length must be a multiple of Al",
            ));
        }

        Ok(Oti {
            fec_encoding_id: FECEncodingID::RaptorQ,
            fec_instance_id: 0,
            maximum_source_block_length: maximum_source_block_length as u32,
            encoding_symbol_length,
            max_number_of_parity_symbols: max_number_of_parity_symbols as u32,
            scheme_specific: Some(SchemeSpecific::RaptorQ(RaptorQSchemeSpecific {
                source_blocks_length: 0,
                sub_blocks_length,
                symbol_alignment,
            })),
            inband_fti: true,
        })
    }

    /// Creates and returns an instance of the `Oti` using the FEC Scheme `Raptor`.
    ///
    /// # Parameters
    ///
    ///   * `encoding_symbol_length`: A `u16` value representing the length of an encoding symbol in bytes.
    ///   An encoding symbol is a piece of data that is generated by the FEC Scheme and added to the source block to create a coded block.
    ///   It is the payload of an ALC/LCT packet. The ALC/LCT header plus the encoding symbol length should be less than the maximum transmission unit (MTU).
    ///
    ///   * `maximum_source_block_length`: A `u16` value representing the maximum length of a source block in bytes.
    ///   A source block is a contiguous portion of the original data that is encoded using the FEC Scheme.
    ///
    ///   * `max_number_of_parity_symbols`: A `u16` value representing the maximum number of parity (repair)
    ///   symbols that can be generated by the FEC Scheme for a given block of data.
    ///
    ///   * `sub_blocks_length`: A `u16` value representing the number of sub-block inside a block.   
    ///      N parameter from <https://www.rfc-editor.org/rfc/rfc6330.html#section-3.3.3>.
    ///
    ///   * `symbol_alignment`: symbol alignment parameter (Al) <https://www.rfc-editor.org/rfc/rfc6330.html#section-3.3.3>.   
    ///      Recommended value is 4.
    ///
    ///  # Returns
    ///
    /// An instance of the `Oti` struct
    ///     
    /// # Errors
    /// Returns an error if the encoding symbols length is not a multiple of al parameter
    ///
    /// # Example
    ///
    /// ```
    /// use flute::core::Oti;
    /// let oti = Oti::new_raptor(1400, 60, 4, 1, 4).unwrap();
    /// ```
    ///
    pub fn new_raptor(
        encoding_symbol_length: u16,
        maximum_source_block_length: u16,
        max_number_of_parity_symbols: u16,
        sub_blocks_length: u8,
        symbol_alignment: u8,
    ) -> Result<Oti> {
        if (encoding_symbol_length % symbol_alignment as u16) != 0 {
            return Err(FluteError::new(
                "Encoding symbols length must be a multiple of Al",
            ));
        }

        Ok(Oti {
            fec_encoding_id: FECEncodingID::Raptor,
            fec_instance_id: 0,
            maximum_source_block_length: maximum_source_block_length as u32,
            encoding_symbol_length,
            max_number_of_parity_symbols: max_number_of_parity_symbols as u32,
            scheme_specific: Some(SchemeSpecific::Raptor(RaptorSchemeSpecific {
                source_blocks_length: 0,
                sub_blocks_length,
                symbol_alignment,
            })),
            inband_fti: true,
        })
    }

    /// Return the maximum file transfer length that the Oti can handle.  
    /// Files with an encoding size (CENC) greater than this value cannot be transferred via FLUTE.
    ///
    /// The maximum file transfer length is calculated as the product of the maximum number of source blocks,
    /// the size of each source block, and the length of an encoding symbol.  
    /// However, the returned value is limited to a maximum of 48 bits, which is the maximum transfer length supported by the FLUTE protocol.
    ///
    pub fn max_transfer_length(&self) -> usize {
        let transfer_length: usize = match self.fec_encoding_id {
            FECEncodingID::NoCode => 0xFFFFFFFFFFFF, // 48 bits max
            FECEncodingID::ReedSolomonGF2M => 0xFFFFFFFFFFFF, // 48 bits max
            FECEncodingID::ReedSolomonGF28 => 0xFFFFFFFFFFFF, // 48 bits max
            FECEncodingID::ReedSolomonGF28UnderSpecified => 0xFFFFFFFFFFFF, // 48 bits max
            FECEncodingID::RaptorQ => 0xFFFFFFFFFFF, // 40 bits max
            FECEncodingID::Raptor => 0xFFFFFFFFFFFF, // 48 bits max
        };

        let max_sbn = self.max_source_blocks_number();
        let block_size =
            self.encoding_symbol_length as usize * self.maximum_source_block_length as usize;
        let size = block_size * max_sbn;
        if size > transfer_length {
            return transfer_length;
        }
        size
    }

    /// Returns the maximum number of source blocks that a file can be divided into, according to the FEC Scheme used.
    pub fn max_source_blocks_number(&self) -> usize {
        match self.fec_encoding_id {
            FECEncodingID::NoCode => u16::MAX as usize,
            FECEncodingID::ReedSolomonGF2M => todo!(),
            FECEncodingID::ReedSolomonGF28 => u8::MAX as usize,
            FECEncodingID::ReedSolomonGF28UnderSpecified => u32::MAX as usize,
            FECEncodingID::RaptorQ => u8::MAX as usize,
            FECEncodingID::Raptor => u16::MAX as usize,
        }
    }

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
        match self.fec_encoding_id {
            FECEncodingID::NoCode => None,
            FECEncodingID::ReedSolomonGF2M => match self.scheme_specific.as_ref() {
                Some(SchemeSpecific::ReedSolomon(scheme)) => Some(scheme.scheme_specific()),
                _ => None,
            },
            FECEncodingID::ReedSolomonGF28 => None,
            FECEncodingID::RaptorQ => match self.scheme_specific.as_ref() {
                Some(SchemeSpecific::RaptorQ(scheme)) => Some(scheme.scheme_specific()),
                _ => None,
            },
            FECEncodingID::Raptor => match self.scheme_specific.as_ref() {
                Some(SchemeSpecific::Raptor(scheme)) => Some(scheme.scheme_specific()),
                _ => None,
            },
            FECEncodingID::ReedSolomonGF28UnderSpecified => None,
        }
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

#[cfg(test)]
mod tests {

    #[test]
    pub fn test_oti() {
        crate::tests::init();
        let no_code = super::Oti::new_no_code(1400, 255);
        log::info!(
            "No Code Max Transfer Length = {} bytes",
            no_code.max_transfer_length()
        );

        let rs28 = super::Oti::new_reed_solomon_rs28(1400, 250, 5).unwrap();
        log::info!(
            "RS28 Max Transfer Length = {} bytes",
            rs28.max_transfer_length()
        );

        let rs28_under_specified =
            super::Oti::new_reed_solomon_rs28_under_specified(1400, 250, 5).unwrap();
        log::info!(
            "RS28 (US) Max Transfer Length = {} bytes",
            rs28_under_specified.max_transfer_length()
        );
    }
}
