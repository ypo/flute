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
    fec_oti_scheme_specific_info: &Option<String>,
) -> Result<Option<SchemeSpecific>> {
    if fec_oti_scheme_specific_info.is_none() {
        return Ok(None);
    }

    let scheme =
        ReedSolomonGF2MSchemeSpecific::decode(fec_oti_scheme_specific_info.as_ref().unwrap())?;
    Ok(Some(SchemeSpecific::ReedSolomon(scheme)))
}

fn raptorq_scheme_specific(
    fec_oti_scheme_specific_info: &Option<String>,
) -> Result<Option<SchemeSpecific>> {
    if fec_oti_scheme_specific_info.is_none() {
        return Ok(None);
    }

    let scheme = RaptorQSchemeSpecific::decode(fec_oti_scheme_specific_info.as_ref().unwrap())?;
    Ok(Some(SchemeSpecific::RaptorQ(scheme)))
}

fn raptor_scheme_specific(
    fec_oti_scheme_specific_info: &Option<String>,
) -> Result<Option<SchemeSpecific>> {
    if fec_oti_scheme_specific_info.is_none() {
        return Ok(None);
    }

    let scheme = RaptorSchemeSpecific::decode(fec_oti_scheme_specific_info.as_ref().unwrap())?;
    Ok(Some(SchemeSpecific::Raptor(scheme)))
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
        let oti = file.get_oti();
        if oti.is_some() {
            return oti;
        }

        self.get_oti()
    }

    pub fn get_oti(&self) -> Option<oti::Oti> {
        if self.fec_oti_fec_encoding_id.is_none()
            || self.fec_oti_maximum_source_block_length.is_none()
            || self.fec_oti_encoding_symbol_length.is_none()
        {
            return None;
        }

        let fec_encoding_id: oti::FECEncodingID =
            self.fec_oti_fec_encoding_id.unwrap().try_into().ok()?;

        let scheme_specific = match fec_encoding_id {
            oti::FECEncodingID::ReedSolomonGF2M => {
                reed_solomon_scheme_specific(&self.fec_oti_scheme_specific_info).unwrap_or(None)
            }
            oti::FECEncodingID::RaptorQ => {
                raptorq_scheme_specific(&self.fec_oti_scheme_specific_info).unwrap_or(None)
            }
            oti::FECEncodingID::Raptor => {
                raptor_scheme_specific(&self.fec_oti_scheme_specific_info).unwrap_or(None)
            }
            _ => None,
        };

        let fec_oti_max_number_of_encoding_symbols = self
            .fec_oti_max_number_of_encoding_symbols
            .unwrap_or(self.fec_oti_maximum_source_block_length.unwrap());

        Some(oti::Oti {
            fec_encoding_id,
            fec_instance_id: self.fec_oti_fec_instance_id.unwrap_or(0) as u16,
            maximum_source_block_length: self.fec_oti_maximum_source_block_length.unwrap() as u32,
            encoding_symbol_length: self.fec_oti_encoding_symbol_length.unwrap() as u16,
            max_number_of_parity_symbols: (fec_oti_max_number_of_encoding_symbols
                - self.fec_oti_maximum_source_block_length.unwrap())
                as u32,
            scheme_specific,
            inband_fti: false,
        })
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

    pub fn get_oti(&self) -> Option<oti::Oti> {
        if self.fec_oti_fec_encoding_id.is_none()
            || self.fec_oti_maximum_source_block_length.is_none()
            || self.fec_oti_encoding_symbol_length.is_none()
        {
            log::debug!("Cannot find OTI {:?}", self);
            return None;
        }
        let fec_encoding_id: oti::FECEncodingID =
            self.fec_oti_fec_encoding_id.unwrap().try_into().ok()?;

        let scheme_specific = match fec_encoding_id {
            oti::FECEncodingID::ReedSolomonGF2M => {
                reed_solomon_scheme_specific(&self.fec_oti_scheme_specific_info).unwrap_or(None)
            }
            oti::FECEncodingID::RaptorQ => {
                raptorq_scheme_specific(&self.fec_oti_scheme_specific_info).unwrap_or(None)
            }
            oti::FECEncodingID::Raptor => {
                raptor_scheme_specific(&self.fec_oti_scheme_specific_info).unwrap_or(None)
            }
            _ => None,
        };

        let fec_oti_max_number_of_encoding_symbols = self
            .fec_oti_max_number_of_encoding_symbols
            .unwrap_or(self.fec_oti_maximum_source_block_length.unwrap());

        Some(oti::Oti {
            fec_encoding_id,
            fec_instance_id: self.fec_oti_fec_instance_id.unwrap_or(0) as u16,
            maximum_source_block_length: self.fec_oti_maximum_source_block_length.unwrap() as u32,
            encoding_symbol_length: self.fec_oti_encoding_symbol_length.unwrap() as u16,
            max_number_of_parity_symbols: (fec_oti_max_number_of_encoding_symbols
                - self.fec_oti_maximum_source_block_length.unwrap())
                as u32,
            scheme_specific,
            inband_fti: false,
        })
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
