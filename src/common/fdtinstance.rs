use std::time::SystemTime;

use crate::{
    receiver::writer::ObjectCacheControl,
    tools::{
        self,
        error::{FluteError, Result},
    },
};

use quick_xml::de::from_reader;
use serde::{Deserialize, Serialize};

#[cfg(feature = "opentelemetry")]
use opentelemetry::{
    global::BoxedSpan,
    trace::{Span, Tracer},
    KeyValue,
};

use super::oti::{
    self, RaptorQSchemeSpecific, RaptorSchemeSpecific, ReedSolomonGF2MSchemeSpecific,
    SchemeSpecific,
};

fn xmlns_mbms_2005<S>(os: &Option<String>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(s) = os {
        serializer.serialize_str(s)
    } else {
        serializer.serialize_str("urn:3GPP:metadata:2005:MBMS:FLUTE:FDT")
    }
}

fn xmlns_mbms_2007<S>(os: &Option<String>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(s) = os {
        serializer.serialize_str(s)
    } else {
        serializer.serialize_str("urn:3GPP:metadata:2007:MBMS:FLUTE:FDT")
    }
}

fn xmlns_mbms_2008<S>(os: &Option<String>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(s) = os {
        serializer.serialize_str(s)
    } else {
        serializer.serialize_str("urn:3GPP:metadata:2008:MBMS:FLUTE:FDT_ext")
    }
}

fn xmlns_mbms_2009<S>(os: &Option<String>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(s) = os {
        serializer.serialize_str(s)
    } else {
        serializer.serialize_str("urn:3GPP:metadata:2009:MBMS:FLUTE:FDT_ext")
    }
}

fn xmlns_mbms_2012<S>(os: &Option<String>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(s) = os {
        serializer.serialize_str(s)
    } else {
        serializer.serialize_str("urn:3GPP:metadata:2012:MBMS:FLUTE:FDT")
    }
}

fn xmlns_mbms_2015<S>(os: &Option<String>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(s) = os {
        serializer.serialize_str(s)
    } else {
        serializer.serialize_str("urn:3GPP:metadata:2015:MBMS:FLUTE:FDT")
    }
}

fn xmlns_sv<S>(os: &Option<String>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(s) = os {
        serializer.serialize_str(s)
    } else {
        serializer.serialize_str("urn:3gpp:metadata:2009:MBMS:schemaVersion")
    }
}

fn xmlns_xsi<S>(os: &Option<String>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(s) = os {
        serializer.serialize_str(s)
    } else {
        serializer.serialize_str("http://www.w3.org/2001/XMLSchema-instance")
    }
}

fn xmlns<S>(os: &Option<String>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(s) = os {
        serializer.serialize_str(s)
    } else {
        serializer.serialize_str("urn:IETF:metadata:2005:FLUTE:FDT")
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct FdtInstance {
    #[serde(rename = "@xmlns", serialize_with = "xmlns")]
    pub xmlns: Option<String>,
    #[serde(rename = "@xmlns:xsi", serialize_with = "xmlns_xsi")]
    pub xmlns_xsi: Option<String>,
    #[serde(rename = "@xmlns:mbms2005", serialize_with = "xmlns_mbms_2005")]
    pub xmlns_mbms_2005: Option<String>,
    #[serde(rename = "@xmlns:mbms2007", serialize_with = "xmlns_mbms_2007")]
    pub xmlns_mbms_2007: Option<String>,
    #[serde(rename = "@xmlns:mbms2008", serialize_with = "xmlns_mbms_2008")]
    pub xmlns_mbms_2008: Option<String>,
    #[serde(rename = "@xmlns:mbms2009", serialize_with = "xmlns_mbms_2009")]
    pub xmlns_mbms_2009: Option<String>,
    #[serde(rename = "@xmlns:mbms2012", serialize_with = "xmlns_mbms_2012")]
    pub xmlns_mbms_2012: Option<String>,
    #[serde(rename = "@xmlns:mbms2015", serialize_with = "xmlns_mbms_2015")]
    pub xmlns_mbms_2015: Option<String>,
    #[serde(rename = "@xmlns:sv", serialize_with = "xmlns_sv")]
    pub xmlns_sv: Option<String>,

    // An FDT Instance is valid until its expiration time.  The
    //  expiration time is expressed within the FDT Instance payload as a
    //  UTF-8 decimal representation of a 32-bit unsigned integer.  The
    //  value of this integer represents the 32 most significant bits of a
    //  64-bit Network Time Protocol (NTP) [RFC5905] time value
    #[serde(rename = "@Expires")]
    pub expires: String,
    #[serde(rename = "@Complete", skip_serializing_if = "Option::is_none")]
    pub complete: Option<bool>,
    #[serde(rename = "@Content-Type", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(rename = "@Content-Encoding", skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,

    #[serde(
        rename = "@FEC-OTI-FEC-Encoding-ID",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_fec_encoding_id: Option<u8>,

    #[serde(
        rename = "@FEC-OTI-FEC-Instance-ID",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_fec_instance_id: Option<u64>,

    #[serde(
        rename = "@FEC-OTI-Maximum-Source-Block-Length",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_maximum_source_block_length: Option<u64>,

    #[serde(
        rename = "@FEC-OTI-Encoding-Symbol-Length",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_encoding_symbol_length: Option<u64>,

    #[serde(
        rename = "@FEC-OTI-Max-Number-of-Encoding-Symbols",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_max_number_of_encoding_symbols: Option<u64>,

    #[serde(
        rename = "@FEC-OTI-Scheme-Specific-Info",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_scheme_specific_info: Option<String>, // Base64

    #[serde(rename = "@mbms2008:FullFDT", skip_serializing_if = "Option::is_none")]
    #[serde(alias = "@FullFDT")]
    pub full_fdt: Option<bool>,

    #[serde(rename = "File", skip_serializing_if = "Option::is_none")]
    pub file: Option<Vec<File>>,

    #[serde(rename = "sv:schemaVersion", skip_serializing_if = "Option::is_none")]
    #[serde(alias = "schemaVersion")]
    pub schema_version: Option<u32>,

    #[serde(
        rename = "mbms2012:Base-URL-1",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(alias = "Base-URL-1")]
    pub base_url_1: Option<Vec<String>>,

    #[serde(
        rename = "mbms2012:Base-URL-2",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(alias = "Base-URL-2")]
    pub base_url_2: Option<Vec<String>>,

    #[serde(
        rename = "sv:delimiter",
        alias = "delimiter",
        skip_serializing_if = "Option::is_none",
        skip_deserializing
    )]
    #[serde(alias = "delimiter")]
    pub delimiter: Option<u8>,

    #[serde(
        rename = "mbms2005:Group",
        alias = "Group",
        skip_serializing_if = "Option::is_none"
    )]
    pub group: Option<Vec<String>>,

    #[serde(
        rename = "mbms2005:MBMS-Session-Identity-Expiry",
        alias = "MBMS-Session-Identity-Expiry",
        skip_serializing_if = "Option::is_none"
    )]
    pub mbms_session_identity_expiry: Option<Vec<MBMSSessionIdentityExpiry>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MBMSSessionIdentityExpiry {
    #[serde(rename = "$value")]
    content: u8,

    #[serde(rename = "@value")]
    pub value: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum CacheControlChoice {
    #[serde(rename = "mbms2007:no-cache")]
    #[serde(alias = "no-cache")]
    NoCache(Option<bool>),
    #[serde(rename = "mbms2007:max-stale")]
    #[serde(alias = "max-stale")]
    MaxStale(Option<bool>),
    #[serde(rename = "mbms2007:Expires")]
    #[serde(alias = "Expires")]
    Expires(u32),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CacheControl {
    #[serde(rename = "$value")]
    pub value: CacheControlChoice,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct File {
    #[serde(
        rename = "mbms2007:Cache-Control",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(alias = "Cache-Control")]
    pub cache_control: Option<CacheControl>,

    #[serde(
        rename = "sv:delimiter",
        skip_serializing_if = "Option::is_none",
        skip_deserializing
    )]
    #[serde(alias = "delimiter")]
    pub delimiter: Option<u8>,

    #[serde(
        rename = "mbms2012:Alternate-Content-Location-1",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(alias = "Alternate-Content-Location-1")]
    pub alternate_content_location_1: Option<Vec<String>>,

    #[serde(
        rename = "mbms2012:Alternate-Content-Location-2",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(alias = "Alternate-Content-Location-2")]
    pub alternate_content_location_2: Option<Vec<String>>,

    #[serde(
        rename = "sv:delimiter",
        skip_serializing_if = "Option::is_none",
        skip_deserializing
    )]
    #[serde(alias = "delimiter")]
    pub delimiter2: Option<u8>,

    #[serde(
        rename = "mbms2005:Group",
        alias = "Group",
        skip_serializing_if = "Option::is_none"
    )]
    pub group: Option<Vec<String>>,

    #[serde(
        rename = "mbms2005:MBMS-Session-Identity",
        alias = "MBMS-Session-Identity",
        skip_serializing_if = "Option::is_none"
    )]
    pub mbms_session_identity: Option<Vec<u8>>,

    #[serde(rename = "@Content-Location")]
    pub content_location: String,
    #[serde(rename = "@TOI")]
    pub toi: String,
    #[serde(rename = "@Content-Length", skip_serializing_if = "Option::is_none")]
    pub content_length: Option<u64>,
    #[serde(rename = "@Transfer-Length", skip_serializing_if = "Option::is_none")]
    pub transfer_length: Option<u64>,
    #[serde(rename = "@Content-Type", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(rename = "@Content-Encoding", skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,
    #[serde(rename = "@Content-MD5", skip_serializing_if = "Option::is_none")]
    pub content_md5: Option<String>,
    #[serde(
        rename = "@FEC-OTI-FEC-Encoding-ID",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_fec_encoding_id: Option<u8>,
    #[serde(
        rename = "@FEC-OTI-FEC-Instance-ID",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_fec_instance_id: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Maximum-Source-Block-Length",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_maximum_source_block_length: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Encoding-Symbol-Length",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_encoding_symbol_length: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Max-Number-of-Encoding-Symbols",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_max_number_of_encoding_symbols: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Scheme-Specific-Info",
        skip_serializing_if = "Option::is_none"
    )]
    pub fec_oti_scheme_specific_info: Option<String>, // Base64

    #[serde(
        rename = "@mbms2009:Decryption-KEY-URI",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(alias = "@Decryption-KEY-URI")]
    pub decryption_key_uri: Option<String>,

    #[serde(
        rename = "@mbms2012:FEC-Redundancy-Level",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(alias = "@FEC-Redundancy-Level")]
    pub fec_redundancy_level: Option<String>,

    #[serde(
        rename = "@mbms2012:File-ETag",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(alias = "@File-ETag")]
    pub file_etag: Option<String>,

    #[serde(
        rename = "@mbms2015:IndependentUnitPositions",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(alias = "@IndependentUnitPositions")]
    pub independent_unit_positions: Option<String>,

    #[serde(
        rename = "@X-Optel-Propagator",
        skip_serializing_if = "Option::is_none"
    )]
    pub optel_propagator: Option<String>,
}

fn reed_solomon_scheme_specific(
    fec_oti_scheme_specific_info: Option<&str>,
) -> Result<Option<SchemeSpecific>> {
    let Some(fec_oti_scheme_specific_info) = fec_oti_scheme_specific_info else {
        return Ok(None);
    };

    let scheme = ReedSolomonGF2MSchemeSpecific::decode(fec_oti_scheme_specific_info)?;
    Ok(Some(SchemeSpecific::ReedSolomon(scheme)))
}

/// Number of source symbols of the largest source block (KL)
///
/// Kt = ceil(F/T), (KL, KS, ZL, ZS) = Partition\[Kt, Z\]
/// <https://www.rfc-editor.org/rfc/rfc5053.html#section-5.3.1.2>
/// <https://www.rfc-editor.org/rfc/rfc6330.html#section-4.4.1.2>
fn raptor_largest_source_block_length(
    transfer_length: u64,
    encoding_symbol_length: u64,
    z: u64,
) -> Option<u64> {
    if encoding_symbol_length == 0 || z == 0 {
        return None;
    }

    let kt = transfer_length.div_ceil(encoding_symbol_length);
    Some(kt.div_ceil(z))
}

fn parse_oti(
    fec_encoding_id: Option<u8>,
    fec_instance_id: Option<u64>,
    maximum_source_block_length: Option<u64>,
    encoding_symbol_length: Option<u64>,
    max_number_of_encoding_symbols: Option<u64>,
    fec_oti_scheme_specific_info: Option<&str>,
    transfer_length: Option<u64>,
) -> Option<oti::Oti> {
    let fec_encoding_id = fec_encoding_id?;
    let encoding_symbol_length = encoding_symbol_length?;
    let fec_encoding: oti::FECEncodingID = fec_encoding_id.try_into().ok()?;

    // RFC 5053 §3.2 and RFC 6330 §3.3 do not define a Maximum Source Block Length
    // nor a Max Number of Encoding Symbols: the source blocks are derived from F, T and Z
    let (scheme_specific, z) = match fec_encoding {
        oti::FECEncodingID::Raptor => {
            let scheme = RaptorSchemeSpecific::decode(fec_oti_scheme_specific_info?).ok()?;
            let z = scheme.source_blocks_length as u64;
            (SchemeSpecific::Raptor(scheme), z)
        }
        oti::FECEncodingID::RaptorQ => {
            let scheme = RaptorQSchemeSpecific::decode(fec_oti_scheme_specific_info?).ok()?;
            let z = scheme.source_blocks_length as u64;
            (SchemeSpecific::RaptorQ(scheme), z)
        }
        _ => {
            let scheme_specific = match fec_encoding {
                oti::FECEncodingID::ReedSolomonGF2M => {
                    reed_solomon_scheme_specific(fec_oti_scheme_specific_info).unwrap_or(None)
                }
                _ => None,
            };

            return build_oti(
                fec_encoding_id,
                fec_instance_id,
                maximum_source_block_length?,
                encoding_symbol_length,
                max_number_of_encoding_symbols,
                scheme_specific,
            );
        }
    };

    let maximum_source_block_length =
        raptor_largest_source_block_length(transfer_length?, encoding_symbol_length, z)?;

    build_oti(
        fec_encoding_id,
        fec_instance_id,
        maximum_source_block_length,
        encoding_symbol_length,
        None,
        Some(scheme_specific),
    )
}

fn build_oti(
    fec_encoding_id: u8,
    fec_instance_id: Option<u64>,
    maximum_source_block_length: u64,
    encoding_symbol_length: u64,
    maximum_number_of_encoding_symbols: Option<u64>,
    scheme_specific: Option<SchemeSpecific>,
) -> Option<oti::Oti> {
    let maximum_number_of_encoding_symbols =
        maximum_number_of_encoding_symbols.unwrap_or(maximum_source_block_length);
    let max_number_of_parity_symbols =
        maximum_number_of_encoding_symbols.checked_sub(maximum_source_block_length)?;

    Some(oti::Oti {
        fec_encoding_id: fec_encoding_id.try_into().ok()?,
        fec_instance_id: u16::try_from(fec_instance_id.unwrap_or(0)).ok()?,
        maximum_source_block_length: u32::try_from(maximum_source_block_length).ok()?,
        encoding_symbol_length: u16::try_from(encoding_symbol_length).ok()?,
        max_number_of_parity_symbols: u32::try_from(max_number_of_parity_symbols).ok()?,
        scheme_specific,
        inband_fti: false,
    })
}

impl FdtInstance {
    #[cfg(feature = "opentelemetry")]
    fn op_start(buffer: &[u8]) -> BoxedSpan {
        let tracer = opentelemetry::global::tracer("FdtInstance");
        let mut span = tracer.start("FdtInstance");
        let str = String::from_utf8_lossy(buffer);
        span.set_attribute(KeyValue::new("content", str.to_string()));
        span
    }

    pub fn parse(buffer: &[u8]) -> Result<FdtInstance> {
        #[cfg(feature = "opentelemetry")]
        let _span = Self::op_start(buffer);

        let instance: Result<FdtInstance> =
            from_reader(buffer).map_err(|err| FluteError::new(err.to_string()));
        instance
    }

    pub fn get_expiration_date(&self) -> Option<SystemTime> {
        let ntp_timestap_seconds: u64 = self.expires.parse().ok()?;
        let time = match tools::ntp_to_system_time(ntp_timestap_seconds << 32) {
            Ok(time) => time,
            Err(e) => {
                log::error!("{:?}", e);
                return None;
            }
        };

        Some(time)
    }

    pub fn get_file(&self, toi: &u128) -> Option<&File> {
        let toi = toi.to_string();
        self.file
            .as_ref()
            .and_then(|file| file.iter().find(|file| file.toi == toi))
    }

    pub fn get_oti_for_file(&self, file: &File) -> Option<oti::Oti> {
        file.get_oti(Some(self))
    }

    /// Content-Encoding of the File, inherited from the FDT-Instance element if missing in the File element
    /// <https://www.rfc-editor.org/rfc/rfc6726.html#section-3.4.2>
    pub fn get_content_encoding_for_file<'a>(&'a self, file: &'a File) -> Option<&'a str> {
        file.content_encoding
            .as_deref()
            .or(self.content_encoding.as_deref())
    }

    /// Content-Type of the File, inherited from the FDT-Instance element if missing in the File element
    /// <https://www.rfc-editor.org/rfc/rfc6726.html#section-3.4.2>
    pub fn get_content_type_for_file<'a>(&'a self, file: &'a File) -> Option<&'a str> {
        file.content_type.as_deref().or(self.content_type.as_deref())
    }
}

impl File {
    pub fn get_object_cache_control(
        &self,
        fdt_expiration_time: Option<SystemTime>,
    ) -> ObjectCacheControl {
        if let Some(cc) = &self.cache_control {
            let ret = match cc.value {
                CacheControlChoice::NoCache(_) => Some(ObjectCacheControl::NoCache),
                CacheControlChoice::MaxStale(_) => Some(ObjectCacheControl::MaxStale),
                CacheControlChoice::Expires(time) => {
                    match tools::ntp_to_system_time((time as u64) << 32) {
                        Ok(res) => Some(ObjectCacheControl::ExpiresAt(res)),
                        Err(_) => {
                            log::warn!("Invalid NTP timestamp in Cache-Control Expires");
                            None
                        }
                    }
                }
            };

            if let Some(ret) = ret {
                return ret;
            }
        }

        // If no Cache-Control is set, we use the FDT expiration time
        let guess_cache_duration: Option<ObjectCacheControl> =
            fdt_expiration_time.map(|v| ObjectCacheControl::ExpiresAtHint(v));

        guess_cache_duration.unwrap_or(ObjectCacheControl::NoCache)
    }

    pub fn get_transfer_length(&self) -> u64 {
        if self.transfer_length.is_some() {
            return self.transfer_length.unwrap();
        }

        if self.content_length.is_some() {
            return self.content_length.unwrap();
        }

        log::warn!("Transfer Length is not set");
        0
    }

    /// FEC Object Transmission Information of the File
    ///
    /// Each FEC-OTI attribute missing in the File element is inherited from the `fdt` FDT-Instance element
    /// <https://www.rfc-editor.org/rfc/rfc6726.html#section-3.4.2>
    pub fn get_oti(&self, fdt: Option<&FdtInstance>) -> Option<oti::Oti> {
        parse_oti(
            self.fec_oti_fec_encoding_id
                .or_else(|| fdt.and_then(|fdt| fdt.fec_oti_fec_encoding_id)),
            self.fec_oti_fec_instance_id
                .or_else(|| fdt.and_then(|fdt| fdt.fec_oti_fec_instance_id)),
            self.fec_oti_maximum_source_block_length
                .or_else(|| fdt.and_then(|fdt| fdt.fec_oti_maximum_source_block_length)),
            self.fec_oti_encoding_symbol_length
                .or_else(|| fdt.and_then(|fdt| fdt.fec_oti_encoding_symbol_length)),
            self.fec_oti_max_number_of_encoding_symbols
                .or_else(|| fdt.and_then(|fdt| fdt.fec_oti_max_number_of_encoding_symbols)),
            self.fec_oti_scheme_specific_info.as_deref().or_else(|| {
                fdt.and_then(|fdt| fdt.fec_oti_scheme_specific_info.as_deref())
            }),
            // Content-Length is the transfer length when Transfer-Length is not set
            self.transfer_length.or(self.content_length),
        )
    }

    #[cfg(feature = "opentelemetry")]
    pub fn get_optel_propagator(&self) -> Option<std::collections::HashMap<String, String>> {
        use base64::Engine;

        self.optel_propagator.as_ref().and_then(|propagator| {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(propagator)
                .ok()?;
            let decoded = String::from_utf8_lossy(&decoded);
            serde_json::from_str(&decoded).ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{build_oti, FdtInstance};
    use crate::common::{
        alccodec::AlcCodec,
        lct,
        oti::{FECEncodingID, Oti, RaptorQSchemeSpecific, RaptorSchemeSpecific, SchemeSpecific},
        partition,
    };

    fn raptor_info(z: u16, n: u8, al: u8) -> String {
        RaptorSchemeSpecific {
            source_blocks_length: z,
            sub_blocks_length: n,
            symbol_alignment: al,
        }
        .scheme_specific()
    }

    fn raptorq_info(z: u8, n: u16, al: u8) -> String {
        RaptorQSchemeSpecific {
            source_blocks_length: z,
            sub_blocks_length: n,
            symbol_alignment: al,
        }
        .scheme_specific()
    }

    fn parse_fdt(instance_attributes: &str, file_attributes: &str) -> FdtInstance {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<FDT-Instance xmlns="urn:IETF:metadata:2005:FLUTE:FDT" Expires="3000000000" {instance_attributes}>
<File Content-Location="file:///object" TOI="1" {file_attributes}/>
</FDT-Instance>"#
        );
        FdtInstance::parse(xml.as_bytes()).unwrap()
    }

    fn oti_for_file(fdt: &FdtInstance) -> Option<Oti> {
        fdt.get_oti_for_file(fdt.get_file(&1).unwrap())
    }

    #[test]
    fn raptor_oti_from_fdt_without_maximum_source_block_length() {
        crate::tests::init();

        // F=10000, T=1024 -> Kt=10, Z=3 -> KL=4
        let fdt = parse_fdt(
            "",
            &format!(
                r#"Transfer-Length="10000" FEC-OTI-FEC-Encoding-ID="1" FEC-OTI-Encoding-Symbol-Length="1024" FEC-OTI-Scheme-Specific-Info="{}""#,
                raptor_info(3, 2, 4)
            ),
        );

        let oti = oti_for_file(&fdt).unwrap();
        assert_eq!(oti.fec_encoding_id, FECEncodingID::Raptor);
        assert_eq!(oti.encoding_symbol_length, 1024);
        assert_eq!(oti.maximum_source_block_length, 4);
        assert_eq!(oti.max_number_of_parity_symbols, 0);
        match oti.scheme_specific.as_ref() {
            Some(SchemeSpecific::Raptor(scheme)) => {
                assert_eq!(scheme.source_blocks_length, 3);
                assert_eq!(scheme.sub_blocks_length, 2);
                assert_eq!(scheme.symbol_alignment, 4);
            }
            _ => panic!("Raptor scheme specific expected"),
        }

        let (_, _, _, nb_blocks) =
            partition::block_partitioning(oti.maximum_source_block_length as u64, 10000, 1024);
        assert_eq!(nb_blocks, 3);
    }

    #[test]
    fn raptorq_oti_from_fdt_instance_without_maximum_source_block_length() {
        crate::tests::init();

        // OTI at FDT-Instance level, F of the File
        // F=100000, T=1400 -> Kt=72, Z=2 -> KL=36
        let fdt = parse_fdt(
            &format!(
                r#"FEC-OTI-FEC-Encoding-ID="6" FEC-OTI-Encoding-Symbol-Length="1400" FEC-OTI-Scheme-Specific-Info="{}""#,
                raptorq_info(2, 1, 4)
            ),
            r#"Transfer-Length="100000""#,
        );

        let oti = oti_for_file(&fdt).unwrap();
        assert_eq!(oti.fec_encoding_id, FECEncodingID::RaptorQ);
        assert_eq!(oti.encoding_symbol_length, 1400);
        assert_eq!(oti.maximum_source_block_length, 36);
        assert_eq!(oti.max_number_of_parity_symbols, 0);
        match oti.scheme_specific.as_ref() {
            Some(SchemeSpecific::RaptorQ(scheme)) => {
                assert_eq!(scheme.source_blocks_length, 2);
                assert_eq!(scheme.sub_blocks_length, 1);
                assert_eq!(scheme.symbol_alignment, 4);
            }
            _ => panic!("RaptorQ scheme specific expected"),
        }
    }

    #[test]
    fn oti_attributes_inherited_from_fdt_instance() {
        crate::tests::init();

        // Encoding ID and T at FDT-Instance level, Scheme-Specific-Info at File level
        // F=10000, T=1024 -> Kt=10, Z=3 -> KL=4
        let fdt = parse_fdt(
            r#"FEC-OTI-FEC-Encoding-ID="1" FEC-OTI-Encoding-Symbol-Length="1024""#,
            &format!(
                r#"Transfer-Length="10000" FEC-OTI-Scheme-Specific-Info="{}""#,
                raptor_info(3, 2, 4)
            ),
        );
        let oti = oti_for_file(&fdt).unwrap();
        assert_eq!(oti.fec_encoding_id, FECEncodingID::Raptor);
        assert_eq!(oti.encoding_symbol_length, 1024);
        assert_eq!(oti.maximum_source_block_length, 4);
        match oti.scheme_specific.as_ref() {
            Some(SchemeSpecific::Raptor(scheme)) => assert_eq!(scheme.source_blocks_length, 3),
            _ => panic!("Raptor scheme specific expected"),
        }

        // Encoding ID at File level, other attributes at FDT-Instance level
        let fdt = parse_fdt(
            r#"FEC-OTI-Maximum-Source-Block-Length="60" FEC-OTI-Encoding-Symbol-Length="1400" FEC-OTI-Max-Number-of-Encoding-Symbols="64""#,
            r#"Transfer-Length="10000" FEC-OTI-FEC-Encoding-ID="5""#,
        );
        let oti = oti_for_file(&fdt).unwrap();
        assert_eq!(oti.fec_encoding_id, FECEncodingID::ReedSolomonGF28);
        assert_eq!(oti.encoding_symbol_length, 1400);
        assert_eq!(oti.maximum_source_block_length, 60);
        assert_eq!(oti.max_number_of_parity_symbols, 4);

        // Without the FDT-Instance, the OTI of the File is incomplete
        assert!(fdt.get_file(&1).unwrap().get_oti(None).is_none());
    }

    #[test]
    fn oti_attributes_of_file_take_precedence() {
        crate::tests::init();

        let fdt = parse_fdt(
            r#"FEC-OTI-FEC-Encoding-ID="0" FEC-OTI-Maximum-Source-Block-Length="64" FEC-OTI-Encoding-Symbol-Length="1400" FEC-OTI-Max-Number-of-Encoding-Symbols="64""#,
            r#"Transfer-Length="10000" FEC-OTI-FEC-Encoding-ID="5" FEC-OTI-Encoding-Symbol-Length="1024" FEC-OTI-Max-Number-of-Encoding-Symbols="70""#,
        );
        let oti = oti_for_file(&fdt).unwrap();
        assert_eq!(oti.fec_encoding_id, FECEncodingID::ReedSolomonGF28);
        assert_eq!(oti.encoding_symbol_length, 1024);
        assert_eq!(oti.maximum_source_block_length, 64);
        assert_eq!(oti.max_number_of_parity_symbols, 6);
    }

    #[test]
    fn raptor_oti_from_fdt_uses_z() {
        crate::tests::init();

        // Attributes not defined by RFC 5053 are ignored, source blocks are derived from Z
        let fdt = parse_fdt(
            "",
            &format!(
                r#"Content-Length="10000" FEC-OTI-FEC-Encoding-ID="1" FEC-OTI-Maximum-Source-Block-Length="64" FEC-OTI-Max-Number-of-Encoding-Symbols="84" FEC-OTI-Encoding-Symbol-Length="1024" FEC-OTI-Scheme-Specific-Info="{}""#,
                raptor_info(3, 1, 4)
            ),
        );

        let oti = oti_for_file(&fdt).unwrap();
        assert_eq!(oti.maximum_source_block_length, 4);
        assert_eq!(oti.max_number_of_parity_symbols, 0);
    }

    #[test]
    fn raptor_oti_from_fdt_invalid() {
        crate::tests::init();

        let cases = [
            // Missing Scheme-Specific-Info
            r#"Transfer-Length="10000" FEC-OTI-FEC-Encoding-ID="1" FEC-OTI-Maximum-Source-Block-Length="64" FEC-OTI-Encoding-Symbol-Length="1024""#.to_string(),
            // Missing Transfer Length
            format!(
                r#"FEC-OTI-FEC-Encoding-ID="1" FEC-OTI-Encoding-Symbol-Length="1024" FEC-OTI-Scheme-Specific-Info="{}""#,
                raptor_info(3, 1, 4)
            ),
            // Z = 0
            format!(
                r#"Transfer-Length="10000" FEC-OTI-FEC-Encoding-ID="1" FEC-OTI-Encoding-Symbol-Length="1024" FEC-OTI-Scheme-Specific-Info="{}""#,
                raptor_info(0, 1, 4)
            ),
            // T = 0
            format!(
                r#"Transfer-Length="10000" FEC-OTI-FEC-Encoding-ID="6" FEC-OTI-Encoding-Symbol-Length="0" FEC-OTI-Scheme-Specific-Info="{}""#,
                raptorq_info(1, 1, 4)
            ),
            // Scheme-Specific-Info of a wrong size
            r#"Transfer-Length="10000" FEC-OTI-FEC-Encoding-ID="6" FEC-OTI-Encoding-Symbol-Length="1024" FEC-OTI-Scheme-Specific-Info="AQE=""#.to_string(),
        ];

        for file_attributes in cases {
            let fdt = parse_fdt("", &file_attributes);
            assert!(oti_for_file(&fdt).is_none(), "{}", file_attributes);
        }
    }

    #[test]
    fn raptor_oti_from_fdt_matches_ext_fti() {
        crate::tests::init();

        for fec in [FECEncodingID::Raptor, FECEncodingID::RaptorQ] {
            // (F, T, Z)
            for (transfer_length, t, z) in [
                (0u64, 1024u16, 1u8),
                (1, 4, 1),
                (10000, 1024, 3),
                (100000, 1400, 2),
                (1000000, 1400, 7),
                (65536, 16, 255),
            ] {
                let mut oti = match fec {
                    FECEncodingID::Raptor => Oti::new_raptor(t, 64, 0, 1, 4),
                    _ => Oti::new_raptorq(t, 64, 0, 1, 4),
                }
                .unwrap();
                match oti.scheme_specific.as_mut() {
                    Some(SchemeSpecific::Raptor(scheme)) => scheme.source_blocks_length = z as u16,
                    Some(SchemeSpecific::RaptorQ(scheme)) => scheme.source_blocks_length = z,
                    _ => panic!("Scheme specific expected"),
                }

                let codec = <dyn AlcCodec>::instance(fec);
                let mut data = Vec::new();
                lct::push_lct_header(&mut data, 0, &0, 1, &2, 1, false, false);
                codec.add_fti(&mut data, &oti, transfer_length);
                let lct_header = lct::parse_lct_header(&data).unwrap();
                let (fti_oti, _) = codec.get_fti(&data, &lct_header).unwrap().unwrap();

                let fdt = parse_fdt(
                    "",
                    &format!(
                        r#"Transfer-Length="{}" FEC-OTI-FEC-Encoding-ID="{}" FEC-OTI-Encoding-Symbol-Length="{}" FEC-OTI-Scheme-Specific-Info="{}""#,
                        transfer_length,
                        fec as u8,
                        t,
                        oti.get_attributes().fec_oti_scheme_specific_info.unwrap()
                    ),
                );
                let fdt_oti = oti_for_file(&fdt).unwrap();

                assert_eq!(
                    fdt_oti.maximum_source_block_length, fti_oti.maximum_source_block_length,
                    "FEC={:?} F={} T={} Z={}",
                    fec, transfer_length, t, z
                );
                assert_eq!(
                    fdt_oti.encoding_symbol_length,
                    fti_oti.encoding_symbol_length
                );
                assert_eq!(
                    fdt_oti.max_number_of_parity_symbols,
                    fti_oti.max_number_of_parity_symbols
                );
            }
        }
    }

    #[test]
    fn build_oti_checks_numeric_ranges() {
        let oti = build_oti(
            FECEncodingID::NoCode as u8,
            Some(u16::MAX as u64),
            u32::MAX as u64,
            u16::MAX as u64,
            Some(u32::MAX as u64 + 10),
            None,
        )
        .unwrap();
        assert_eq!(oti.max_number_of_parity_symbols, 10);

        assert!(build_oti(FECEncodingID::NoCode as u8, None, 10, 1, Some(9), None,).is_none());
        assert!(build_oti(
            FECEncodingID::NoCode as u8,
            Some(u16::MAX as u64 + 1),
            1,
            1,
            None,
            None,
        )
        .is_none());
        assert!(build_oti(
            FECEncodingID::NoCode as u8,
            None,
            u32::MAX as u64 + 1,
            1,
            None,
            None,
        )
        .is_none());
        assert!(build_oti(
            FECEncodingID::NoCode as u8,
            None,
            1,
            u16::MAX as u64 + 1,
            None,
            None,
        )
        .is_none());
        assert!(build_oti(
            FECEncodingID::NoCode as u8,
            None,
            0,
            1,
            Some(u32::MAX as u64 + 1),
            None,
        )
        .is_none());
    }
}
