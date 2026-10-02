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

        ReedSolomonGF2MSchemeSpecific::parse(info[0], info[1])
    }

    /// Scheme from the m and G fields of the FEC OTI, where 0 means the default value (m = 8, G = 1)
    /// <https://www.rfc-editor.org/rfc/rfc5510.html#section-4.2.4.2>
    pub(crate) fn parse(m: u8, g: u8) -> Result<ReedSolomonGF2MSchemeSpecific> {
        let default = ReedSolomonGF2MSchemeSpecific::default();
        let m = if m == 0 { default.m } else { m };
        let g = if g == 0 { default.g } else { g };

        if !(2..=16).contains(&m) {
            return Err(FluteError::new(format!(
                "Reed-Solomon GF(2^m): m={} is not between 2 and 16",
                m
            )));
        }

        Ok(ReedSolomonGF2MSchemeSpecific { m, g })
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

    /// Creates and returns an instance of the `Oti` using the Forward Error Correction (FEC) Scheme `ReedSolomonGF2M`
    /// <https://www.rfc-editor.org/rfc/rfc5510.html#section-4>.
    ///
    /// Each ALC/LCT packet carries a single encoding symbol (G = 1).
    ///
    /// # Parameters
    ///
    ///   * `encoding_symbol_length`: A `u16` value representing the length of an encoding symbol in bytes.
    ///   An encoding symbol is made of `encoding_symbol_length * 8 / m` elements of `m` bits.
    ///
    ///   * `maximum_source_block_length`: A `u16` value representing the maximum number of source symbols in a source block.
    ///
    ///   * `max_number_of_parity_symbols`: A `u16` value representing the maximum number of parity (repair)
    ///   symbols that can be generated by the FEC Scheme for a given block of data.
    ///
    ///   * `m`: length of the finite field elements in bits, between 2 and 16.
    ///
    ///  # Returns
    ///
    /// An instance of the `Oti` struct
    ///
    /// # Errors
    /// Returns an error if
    /// * `m` is not between 2 and 16
    /// * `maximum_source_block_length` is 0
    /// * the maximum Encoded Block Length (`maximum_source_block_length` + `max_number_of_parity_symbols`) is greater than `2^m - 1`
    /// * `encoding_symbol_length` is 0 or `encoding_symbol_length * 8` is not a multiple of `m`
    ///
    /// # Example
    ///
    /// ```
    /// use flute::core::Oti;
    /// // Files are cut in blocks of 1000 source symbols and 100 parity (repair) symbols of 1400 bytes each, over GF(2^16)
    /// let oti = Oti::new_reed_solomon_rs2m(1400, 1000, 100, 16).unwrap();
    /// ```
    ///
    pub fn new_reed_solomon_rs2m(
        encoding_symbol_length: u16,
        maximum_source_block_length: u16,
        max_number_of_parity_symbols: u16,
        m: u8,
    ) -> Result<Oti> {
        crate::fec::rsgf2m::check_parameters(
            m,
            maximum_source_block_length as usize,
            maximum_source_block_length as usize + max_number_of_parity_symbols as usize,
            encoding_symbol_length as usize,
        )?;

        Ok(Oti {
            fec_encoding_id: FECEncodingID::ReedSolomonGF2M,
            fec_instance_id: 0,
            maximum_source_block_length: maximum_source_block_length as u32,
            encoding_symbol_length,
            max_number_of_parity_symbols: max_number_of_parity_symbols as u32,
            scheme_specific: Some(SchemeSpecific::ReedSolomon(ReedSolomonGF2MSchemeSpecific {
                m,
                g: 1,
            })),
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
    /// Returns an error if the maximum Encoded Block Length (`maximum_source_block_length` + `max_number_of_parity_symbols`) is greater than `255`.
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
        // Reed-Solomon over GF(2^8) <https://www.rfc-editor.org/rfc/rfc5510.html#section-7>
        let encoding_block_length: usize =
            maximum_source_block_length as usize + max_number_of_parity_symbols as usize;
        if encoding_block_length > 255 {
            return Err(FluteError::new("Encoding Block Length (Source Block Length + Number of parity symbols) must be <= 255"));
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
    /// Returns an error if
    /// * `symbol_alignment` is 0
    /// * the encoding symbols length is not a multiple of al parameter
    /// * `sub_blocks_length` is not between 1 and `encoding_symbol_length` / `symbol_alignment` (a sub-symbol is at least Al bytes) <https://www.rfc-editor.org/rfc/rfc6330.html#section-4.4.1.2>
    /// * `maximum_source_block_length` is not between 1 and 56403 (K'max) <https://www.rfc-editor.org/rfc/rfc6330.html#section-5.1.2>
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
        if symbol_alignment == 0 {
            return Err(FluteError::new("Al must be at least 1"));
        }

        if (encoding_symbol_length % symbol_alignment as u16) != 0 {
            return Err(FluteError::new(
                "Encoding symbols length must be a multiple of Al",
            ));
        }

        if sub_blocks_length == 0
            || sub_blocks_length > encoding_symbol_length / symbol_alignment as u16
        {
            return Err(FluteError::new(
                "Number of sub-blocks must be between 1 and encoding symbols length / Al",
            ));
        }

        if !(1..=56403).contains(&maximum_source_block_length) {
            return Err(FluteError::new(
                "Maximum source block length must be between 1 and 56403",
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
    ///   * `sub_blocks_length`: A `u8` value representing the number of sub-blocks inside a block.   
    ///      N parameter from <https://www.rfc-editor.org/rfc/rfc5053.html#section-3.2.3>.
    ///
    ///   * `symbol_alignment`: symbol alignment parameter (Al) <https://www.rfc-editor.org/rfc/rfc5053.html#section-3.2.3>.   
    ///      Recommended value is 4 <https://www.rfc-editor.org/rfc/rfc5053.html#section-4.2>.
    ///
    ///  # Returns
    ///
    /// An instance of the `Oti` struct
    ///     
    /// # Errors
    /// Returns an error if
    /// * `symbol_alignment` is 0
    /// * the encoding symbols length is not a multiple of al parameter
    /// * `sub_blocks_length` is not between 1 and `encoding_symbol_length` / `symbol_alignment` (a sub-symbol is at least Al bytes) <https://www.rfc-editor.org/rfc/rfc5053.html#section-5.3.1.2>
    /// * `maximum_source_block_length` is not between 4 and 8192 <https://www.rfc-editor.org/rfc/rfc5053.html#section-5.7>
    /// * `maximum_source_block_length` + `max_number_of_parity_symbols` is above 65536 (ESI is 16 bits) <https://www.rfc-editor.org/rfc/rfc5053.html#section-3.1>
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
        if symbol_alignment == 0 {
            return Err(FluteError::new("Al must be at least 1"));
        }

        if (encoding_symbol_length % symbol_alignment as u16) != 0 {
            return Err(FluteError::new(
                "Encoding symbols length must be a multiple of Al",
            ));
        }

        if sub_blocks_length == 0
            || sub_blocks_length as u16 > encoding_symbol_length / symbol_alignment as u16
        {
            return Err(FluteError::new(
                "Number of sub-blocks must be between 1 and encoding symbols length / Al",
            ));
        }

        if !(4..=8192).contains(&maximum_source_block_length) {
            return Err(FluteError::new(
                "Maximum source block length must be between 4 and 8192",
            ));
        }

        if maximum_source_block_length as u32 + max_number_of_parity_symbols as u32 > 65536 {
            return Err(FluteError::new(
                "Maximum source block length + max number of parity symbols must not exceed 65536",
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
        let transfer_length: u128 = match self.fec_encoding_id {
            FECEncodingID::NoCode => 0xFFFFFFFFFFFF, // 48 bits max
            FECEncodingID::ReedSolomonGF2M => 0xFFFFFFFFFFFF, // 48 bits max
            FECEncodingID::ReedSolomonGF28 => 0xFFFFFFFFFFFF, // 48 bits max
            FECEncodingID::ReedSolomonGF28UnderSpecified => 0xFFFFFFFFFFFF, // 48 bits max
            FECEncodingID::RaptorQ => 942574504275,  // RFC 6330 Errata 5548
            FECEncodingID::Raptor => 0xFFFFFFFFFFFF, // 48 bits max
        };

        let size = self.encoding_symbol_length as u128
            * self.maximum_source_block_length as u128
            * self.max_source_blocks_number() as u128;

        size.min(transfer_length).min(usize::MAX as u128) as usize
    }

    /// Returns the maximum number of source blocks that a file can be divided into, according to the FEC Scheme used.
    pub fn max_source_blocks_number(&self) -> usize {
        match self.fec_encoding_id {
            FECEncodingID::NoCode => u16::MAX as usize,
            // Source Block Number of 32 - m bits <https://www.rfc-editor.org/rfc/rfc5510.html#section-4.1>
            FECEncodingID::ReedSolomonGF2M => match self.reed_solomon_gf2m_scheme().m {
                m @ 2..=16 => 1usize << (32 - m),
                _ => 0,
            },
            FECEncodingID::ReedSolomonGF28 => u8::MAX as usize,
            FECEncodingID::ReedSolomonGF28UnderSpecified => u32::MAX as usize,
            FECEncodingID::RaptorQ => u8::MAX as usize,
            FECEncodingID::Raptor => u16::MAX as usize,
        }
    }

    /// Reed-Solomon scheme
    /// * FEC Encoding ID 2: m = 8 and G = 1 when it is not defined <https://www.rfc-editor.org/rfc/rfc5510.html#section-4.2.3>
    /// * FEC Encoding ID 5 and FEC Encoding ID 129 / FEC Instance ID 0: m = 8 and G = 1
    ///   <https://www.rfc-editor.org/rfc/rfc5510.html#section-5>
    pub(crate) fn reed_solomon_gf2m_scheme(&self) -> ReedSolomonGF2MSchemeSpecific {
        match (self.fec_encoding_id, self.scheme_specific.as_ref()) {
            (FECEncodingID::ReedSolomonGF2M, Some(SchemeSpecific::ReedSolomon(scheme))) => {
                scheme.clone()
            }
            _ => ReedSolomonGF2MSchemeSpecific::default(),
        }
    }

    /// Check that the FEC parameters can be used to send objects
    ///
    /// Fields of an `Oti` can be modified after its creation, and encoding errors are not reported once a transfer is started.
    pub(crate) fn check_sender_parameters(&self) -> Result<()> {
        match self.fec_encoding_id {
            FECEncodingID::ReedSolomonGF2M
            | FECEncodingID::ReedSolomonGF28
            | FECEncodingID::ReedSolomonGF28UnderSpecified => {
                let scheme = self.reed_solomon_gf2m_scheme();
                if scheme.g != 1 {
                    return Err(FluteError::new(format!(
                        "FEC Reed-Solomon GF(2^m): G={} is not supported, only one encoding symbol per packet (G=1) can be sent",
                        scheme.g
                    )));
                }

                crate::fec::rsgf2m::check_parameters(
                    scheme.m,
                    self.maximum_source_block_length as usize,
                    self.maximum_source_block_length as usize
                        + self.max_number_of_parity_symbols as usize,
                    self.encoding_symbol_length as usize,
                )
            }
            _ => Ok(()),
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

    use super::{FECEncodingID, Oti};

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

    #[test]
    fn new_raptor_checks_parameters() {
        assert!(Oti::new_raptor(1024, 64, 20, 1, 4).is_ok());
        assert!(Oti::new_raptor(1023, 64, 20, 1, 1).is_ok());
        assert!(Oti::new_raptor(1024, 4, 0, 1, 4).is_ok());
        assert!(Oti::new_raptor(1024, 8192, 57344, 1, 4).is_ok());

        // Al
        assert!(Oti::new_raptor(1024, 64, 20, 1, 0).is_err());
        assert!(Oti::new_raptor(1022, 64, 20, 1, 4).is_err());

        // N
        assert!(Oti::new_raptor(1024, 64, 20, 255, 4).is_ok());
        assert!(Oti::new_raptor(16, 64, 20, 4, 4).is_ok());
        assert!(Oti::new_raptor(1024, 64, 20, 0, 4).is_err());
        assert!(Oti::new_raptor(16, 64, 20, 5, 4).is_err());
        assert!(Oti::new_raptor(0, 64, 20, 1, 4).is_err());

        // Kmax
        assert!(Oti::new_raptor(1024, 3, 20, 1, 4).is_err());
        assert!(Oti::new_raptor(1024, 8193, 20, 1, 4).is_err());

        // ESI is 16 bits
        assert!(Oti::new_raptor(1024, 8192, 57345, 1, 4).is_err());
        assert!(Oti::new_raptor(1024, 4, u16::MAX, 1, 4).is_err());
    }

    #[test]
    fn max_transfer_length_does_not_overflow() {
        let exact = Oti::new_no_code(10, 20);
        assert_eq!(exact.max_transfer_length(), 10 * 20 * u16::MAX as usize);

        let oversized = Oti {
            fec_encoding_id: FECEncodingID::ReedSolomonGF28UnderSpecified,
            fec_instance_id: 0,
            maximum_source_block_length: u32::MAX,
            encoding_symbol_length: u16::MAX,
            max_number_of_parity_symbols: 0,
            scheme_specific: None,
            inband_fti: true,
        };
        assert_eq!(
            oversized.max_transfer_length(),
            usize::try_from(0xFFFFFFFFFFFFu64).unwrap_or(usize::MAX)
        );
    }

    #[test]
    fn new_reed_solomon_rs2m_checks_parameters() {
        assert!(Oti::new_reed_solomon_rs2m(1400, 200, 55, 8).is_ok());
        assert!(Oti::new_reed_solomon_rs2m(1400, 1000, 100, 16).is_ok());
        assert!(Oti::new_reed_solomon_rs2m(1, 3, 0, 2).is_ok());
        assert!(Oti::new_reed_solomon_rs2m(1401, 4, 3, 3).is_ok());

        // m
        assert!(Oti::new_reed_solomon_rs2m(1400, 1, 1, 1).is_err());
        assert!(Oti::new_reed_solomon_rs2m(1400, 1, 1, 17).is_err());

        // n <= 2^m - 1
        assert!(Oti::new_reed_solomon_rs2m(1400, 200, 56, 8).is_err());
        assert!(Oti::new_reed_solomon_rs2m(1400, 60000, 5536, 16).is_err());
        assert!(Oti::new_reed_solomon_rs2m(1400, 0, 4, 8).is_err());

        // E * 8 is a multiple of m
        assert!(Oti::new_reed_solomon_rs2m(1400, 4, 3, 3).is_err());
        assert!(Oti::new_reed_solomon_rs2m(1399, 4, 3, 16).is_err());
        assert!(Oti::new_reed_solomon_rs2m(0, 4, 3, 8).is_err());
    }

    #[test]
    fn reed_solomon_rs2m_max_source_blocks_number() {
        let oti = Oti::new_reed_solomon_rs2m(1400, 200, 55, 8).unwrap();
        assert_eq!(oti.max_source_blocks_number(), 1 << 24);

        let oti = Oti::new_reed_solomon_rs2m(1400, 1000, 100, 16).unwrap();
        assert_eq!(oti.max_source_blocks_number(), 1 << 16);
        assert_eq!(oti.max_transfer_length(), 1400 * 1000 * (1 << 16));

        // Default scheme m = 8
        let mut oti = Oti::new_reed_solomon_rs2m(1400, 200, 55, 8).unwrap();
        oti.scheme_specific = None;
        assert_eq!(oti.max_source_blocks_number(), 1 << 24);

        // Invalid m
        oti.scheme_specific = Some(super::SchemeSpecific::ReedSolomon(
            super::ReedSolomonGF2MSchemeSpecific { m: 17, g: 1 },
        ));
        assert_eq!(oti.max_source_blocks_number(), 0);
    }

    #[test]
    fn reed_solomon_gf2m_scheme_specific_decode() {
        use super::ReedSolomonGF2MSchemeSpecific;

        let scheme = ReedSolomonGF2MSchemeSpecific { m: 16, g: 3 };
        let decoded = ReedSolomonGF2MSchemeSpecific::decode(&scheme.scheme_specific()).unwrap();
        assert_eq!((decoded.m, decoded.g), (16, 3));

        // 0 means default value
        let scheme = ReedSolomonGF2MSchemeSpecific { m: 0, g: 0 };
        let decoded = ReedSolomonGF2MSchemeSpecific::decode(&scheme.scheme_specific()).unwrap();
        assert_eq!((decoded.m, decoded.g), (8, 1));

        for m in [1, 17, 255] {
            let scheme = ReedSolomonGF2MSchemeSpecific { m, g: 1 };
            assert!(ReedSolomonGF2MSchemeSpecific::decode(&scheme.scheme_specific()).is_err());
        }
    }

    #[test]
    fn check_sender_parameters() {
        use super::{ReedSolomonGF2MSchemeSpecific, SchemeSpecific};

        assert!(Oti::new_no_code(1400, 64).check_sender_parameters().is_ok());

        let mut oti = Oti::new_reed_solomon_rs2m(1400, 200, 55, 8).unwrap();
        assert!(oti.check_sender_parameters().is_ok());

        // Default scheme m = 8, G = 1
        oti.scheme_specific = None;
        assert!(oti.check_sender_parameters().is_ok());

        // Only G = 1 is supported by the sender
        oti.scheme_specific = Some(SchemeSpecific::ReedSolomon(ReedSolomonGF2MSchemeSpecific {
            m: 8,
            g: 2,
        }));
        assert!(oti.check_sender_parameters().is_err());

        // n > 2^m - 1
        let mut oti = Oti::new_reed_solomon_rs2m(1400, 200, 55, 8).unwrap();
        oti.max_number_of_parity_symbols = 56;
        assert!(oti.check_sender_parameters().is_err());

        // FEC Encoding ID 5 and 129 are Reed-Solomon over GF(2^8), the scheme specific is ignored
        for mut oti in [
            Oti::new_reed_solomon_rs28(1400, 200, 55).unwrap(),
            Oti::new_reed_solomon_rs28_under_specified(1400, 200, 55).unwrap(),
        ] {
            assert!(oti.check_sender_parameters().is_ok());
            oti.scheme_specific = Some(SchemeSpecific::ReedSolomon(
                ReedSolomonGF2MSchemeSpecific { m: 16, g: 2 },
            ));
            assert!(oti.check_sender_parameters().is_ok());
            oti.max_number_of_parity_symbols = 56;
            assert!(oti.check_sender_parameters().is_err());
        }
    }

    #[test]
    fn new_reed_solomon_rs28_under_specified_checks_parameters() {
        assert!(Oti::new_reed_solomon_rs28_under_specified(1400, 200, 55).is_ok());
        assert!(Oti::new_reed_solomon_rs28_under_specified(1400, 200, 56).is_err());
        assert!(Oti::new_reed_solomon_rs28_under_specified(1400, 1000, 0).is_err());
    }
}
