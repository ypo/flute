use serde::Serialize;

use crate::tools::error::{FluteError, Result};

use super::fdtinstance::{CacheControlChoice, FdtInstance, File};

const XMLNS: &str = "urn:3GPP:metadata:2022:FLUTE:FDT";

#[derive(Serialize)]
pub(crate) struct FdtInstanceL6<'a> {
    #[serde(rename = "@xmlns")]
    xmlns: &'static str,
    #[serde(rename = "@Expires")]
    expires: &'a str,
    #[serde(rename = "@Complete", skip_serializing_if = "Option::is_none")]
    complete: Option<bool>,
    #[serde(rename = "@Content-Type", skip_serializing_if = "Option::is_none")]
    content_type: Option<&'a str>,
    #[serde(rename = "@Content-Encoding", skip_serializing_if = "Option::is_none")]
    content_encoding: Option<&'a str>,
    #[serde(
        rename = "@FEC-OTI-FEC-Encoding-ID",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_fec_encoding_id: Option<u8>,
    #[serde(
        rename = "@FEC-OTI-FEC-Instance-ID",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_fec_instance_id: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Maximum-Source-Block-Length",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_maximum_source_block_length: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Encoding-Symbol-Length",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_encoding_symbol_length: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Max-Number-of-Encoding-Symbols",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_max_number_of_encoding_symbols: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Scheme-Specific-Info",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_scheme_specific_info: Option<&'a str>,
    #[serde(rename = "File")]
    file: Vec<FileL6<'a>>,
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
}

#[derive(Serialize)]
struct FileL6<'a> {
    #[serde(rename = "@Content-Location")]
    content_location: &'a str,
    #[serde(rename = "@TOI")]
    toi: &'a str,
    #[serde(rename = "@Content-Length", skip_serializing_if = "Option::is_none")]
    content_length: Option<u64>,
    #[serde(rename = "@Transfer-Length", skip_serializing_if = "Option::is_none")]
    transfer_length: Option<u64>,
    #[serde(rename = "@Content-Type", skip_serializing_if = "Option::is_none")]
    content_type: Option<&'a str>,
    #[serde(rename = "@Content-Encoding", skip_serializing_if = "Option::is_none")]
    content_encoding: Option<&'a str>,
    #[serde(rename = "@Content-MD5", skip_serializing_if = "Option::is_none")]
    content_md5: Option<&'a str>,
    #[serde(
        rename = "@FEC-OTI-FEC-Encoding-ID",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_fec_encoding_id: Option<u8>,
    #[serde(
        rename = "@FEC-OTI-FEC-Instance-ID",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_fec_instance_id: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Maximum-Source-Block-Length",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_maximum_source_block_length: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Encoding-Symbol-Length",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_encoding_symbol_length: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Max-Number-of-Encoding-Symbols",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_max_number_of_encoding_symbols: Option<u64>,
    #[serde(
        rename = "@FEC-OTI-Scheme-Specific-Info",
        skip_serializing_if = "Option::is_none"
    )]
    fec_oti_scheme_specific_info: Option<&'a str>,
    #[serde(
        rename = "@FEC-Redundancy-Level",
        skip_serializing_if = "Option::is_none"
    )]
    fec_redundancy_level: Option<u32>,
    #[serde(rename = "@File-ETag", skip_serializing_if = "Option::is_none")]
    file_etag: Option<&'a str>,
    #[serde(
        rename = "@X-Optel-Propagator",
        skip_serializing_if = "Option::is_none"
    )]
    optel_propagator: Option<&'a str>,
    #[serde(rename = "Cache-Control", skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControlL6>,
}

#[derive(Serialize)]
struct CacheControlL6 {
    #[serde(rename = "$value")]
    value: CacheControlChoiceL6,
}

#[derive(Serialize)]
enum CacheControlChoiceL6 {
    #[serde(rename = "no-cache")]
    NoCache(bool),
    #[serde(rename = "max-stale")]
    MaxStale(bool),
    #[serde(rename = "Expires")]
    Expires(u32),
}

impl<'a> FdtInstanceL6<'a> {
    pub(crate) fn try_from_fdt(instance: &'a FdtInstance) -> Result<Self> {
        let schema_version = instance
            .schema_version
            .ok_or_else(|| FluteError::new("L.6 FDT requires schemaVersion"))?;
        let files = instance
            .file
            .as_ref()
            .filter(|files| !files.is_empty())
            .ok_or_else(|| FluteError::new("L.6 FDT requires at least one File"))?;

        let file = files
            .iter()
            .map(FileL6::try_from_file)
            .collect::<Result<Vec<_>>>()?;

        log_omitted("FullFDT", instance.full_fdt.is_some());
        log_omitted("Base-URL-1", instance.base_url_1.is_some());
        log_omitted("Base-URL-2", instance.base_url_2.is_some());
        log_omitted("delimiter", instance.delimiter.is_some());
        log_omitted("Group", instance.group.is_some());
        log_omitted(
            "MBMS-Session-Identity-Expiry",
            instance.mbms_session_identity_expiry.is_some(),
        );

        Ok(Self {
            xmlns: XMLNS,
            expires: &instance.expires,
            complete: instance.complete,
            content_type: instance.content_type.as_deref(),
            content_encoding: instance.content_encoding.as_deref(),
            fec_oti_fec_encoding_id: instance.fec_oti_fec_encoding_id,
            fec_oti_fec_instance_id: instance.fec_oti_fec_instance_id,
            fec_oti_maximum_source_block_length: instance.fec_oti_maximum_source_block_length,
            fec_oti_encoding_symbol_length: instance.fec_oti_encoding_symbol_length,
            fec_oti_max_number_of_encoding_symbols: instance.fec_oti_max_number_of_encoding_symbols,
            fec_oti_scheme_specific_info: instance.fec_oti_scheme_specific_info.as_deref(),
            file,
            schema_version,
        })
    }
}

impl<'a> FileL6<'a> {
    fn try_from_file(file: &'a File) -> Result<Self> {
        if !matches!(file.toi.parse::<u128>(), Ok(toi) if toi > 0) {
            return Err(FluteError::new("L.6 File TOI must be a positive integer"));
        }

        let fec_redundancy_level = file
            .fec_redundancy_level
            .as_deref()
            .map(str::parse::<u32>)
            .transpose()
            .map_err(|_| FluteError::new("L.6 FEC-Redundancy-Level must be an unsigned integer"))?;

        let cache_control = file
            .cache_control
            .as_ref()
            .map(|cache_control| {
                let value = match cache_control.value {
                    CacheControlChoice::NoCache(Some(false))
                    | CacheControlChoice::MaxStale(Some(false)) => {
                        return Err(FluteError::new(
                            "L.6 no-cache and max-stale values must be true",
                        ))
                    }
                    CacheControlChoice::NoCache(_) => CacheControlChoiceL6::NoCache(true),
                    CacheControlChoice::MaxStale(_) => CacheControlChoiceL6::MaxStale(true),
                    CacheControlChoice::Expires(value) => CacheControlChoiceL6::Expires(value),
                };
                Ok(CacheControlL6 { value })
            })
            .transpose()?;

        log_omitted("Decryption-KEY-URI", file.decryption_key_uri.is_some());
        log_omitted(
            "IndependentUnitPositions",
            file.independent_unit_positions.is_some(),
        );
        log_omitted("delimiter", file.delimiter.is_some());
        log_omitted(
            "Alternate-Content-Location-1",
            file.alternate_content_location_1.is_some(),
        );
        log_omitted(
            "Alternate-Content-Location-2",
            file.alternate_content_location_2.is_some(),
        );
        log_omitted("delimiter", file.delimiter2.is_some());
        log_omitted("Group", file.group.is_some());
        log_omitted(
            "MBMS-Session-Identity",
            file.mbms_session_identity.is_some(),
        );

        Ok(Self {
            content_location: &file.content_location,
            toi: &file.toi,
            content_length: file.content_length,
            transfer_length: file.transfer_length,
            content_type: file.content_type.as_deref(),
            content_encoding: file.content_encoding.as_deref(),
            content_md5: file.content_md5.as_deref(),
            fec_oti_fec_encoding_id: file.fec_oti_fec_encoding_id,
            fec_oti_fec_instance_id: file.fec_oti_fec_instance_id,
            fec_oti_maximum_source_block_length: file.fec_oti_maximum_source_block_length,
            fec_oti_encoding_symbol_length: file.fec_oti_encoding_symbol_length,
            fec_oti_max_number_of_encoding_symbols: file.fec_oti_max_number_of_encoding_symbols,
            fec_oti_scheme_specific_info: file.fec_oti_scheme_specific_info.as_deref(),
            fec_redundancy_level,
            file_etag: file.file_etag.as_deref(),
            optel_propagator: file.optel_propagator.as_deref(),
            cache_control,
        })
    }
}

fn log_omitted(field: &str, present: bool) {
    if present {
        log::error!("Omitting {} because it is not part of the L.6 FDT schema", field);
    }
}
