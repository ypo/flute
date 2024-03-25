use base64::Engine;

use super::compress;
use super::toiallocator::Toi;
use crate::common::{fdtinstance, lct, oti};
use crate::error::FluteError;
use crate::tools;
use crate::tools::error::Result;
use std::ffi::OsStr;
use std::io::{BufReader, Read};
use std::sync::Arc;
use std::time::SystemTime;

/// Cache Control
///
/// The `CacheControl` enum represents different directives used for controlling caching behavior.
/// It is commonly used in web development to indicate caching preferences for specific files or resources.
#[derive(Debug, Clone, Copy)]
pub enum CacheControl {
    /// Specifies that the receiver should not cache the specific file or resource.
    NoCache,

    /// Indicates that a specific file (or set of files) should be cached for an indefinite period of time,
    /// allowing stale versions of the resource to be served even after they have expired.
    MaxStale,

    /// Specifies the expected expiry time for the file or resource, allowing the server
    /// to indicate when the cached version should no longer be considered valid.
    Expires(std::time::Duration),
}

/// Concert CacheControl to fdtinstance::CacheControl
pub fn create_fdt_cache_control(cc: &CacheControl, now: SystemTime) -> fdtinstance::CacheControl {
    match cc {
        CacheControl::NoCache => fdtinstance::CacheControl {
            value: fdtinstance::CacheControlChoice::NoCache(Some(true)),
        },
        CacheControl::MaxStale => fdtinstance::CacheControl {
            value: fdtinstance::CacheControlChoice::MaxStale(Some(true)),
        },
        CacheControl::Expires(duration) => {
            let expires = now + *duration;
            let ntp = tools::system_time_to_ntp(expires).unwrap_or_default();
            fdtinstance::CacheControl {
                value: fdtinstance::CacheControlChoice::Expires((ntp >> 32) as u32),
            }
        }
    }
}

///
/// Object (file) that can be send over FLUTE
///
#[derive(Debug)]
pub struct ObjectDesc {
    /// supply the resource location for this object
    /// as defined in [rfc2616 14.14](https://www.rfc-editor.org/rfc/rfc2616#section-14.14)
    pub content_location: url::Url,
    /// Optional path to the file
    pub path: Option<std::path::PathBuf>,
    /// Optional buffer contening the content of this object
    pub content: Option<Vec<u8>>,
    /// Media type of the object
    /// as defined in [rfc2616 14.17](https://www.rfc-editor.org/rfc/rfc2616#section-14.17)
    pub content_type: String,
    /// Size of the object (uncompressed)
    /// as defined in [rfc2616 14.13](https://www.rfc-editor.org/rfc/rfc2616#section-14.13)
    pub content_length: u64,
    /// Size of the object after transfer-coding (`Cenc`) has been applied
    /// as defined in [rfc2616 4.4](https://www.rfc-editor.org/rfc/rfc2616#section-4.4)
    pub transfer_length: u64,
    /// Content Encoding (compression)
    pub cenc: lct::Cenc,
    /// If `true`, Cenc extension are added to ALC/LCT packet
    /// Else Cenc is defined only inside the FDT
    pub inband_cenc: bool,
    /// the MD5 sum of this object. Can be used by the FLUTE `receiver`to validate the integrity of the reception
    pub md5: Option<String>,
    /// Optional list of attributes that will be added to the FDT
    pub attributes: Option<std::collections::HashMap<String, String>>,
    /// If defined, FEC Object Transmission Information (OTI) overload the default OTI defined in the FDT
    pub oti: Option<oti::Oti>,
    /// Repeat the transfer the same object multiple times
    pub max_transfer_count: u32,
    /// If defined, object is transmitted in a carousel every `carousel_delay_ns`
    pub carousel_delay: Option<std::time::Duration>,
    /// Define object cache control
    pub cache_control: Option<CacheControl>,
    /// Add file to a list of groups
    pub groups: Option<Vec<String>>,
    /// Assign an optional TOI to this object
    pub toi: Option<Arc<Toi>>,
}

impl ObjectDesc {
    /// Return an `ObjectDesc` from a file
    pub fn create_from_file(
        path: &std::path::Path,
        content_location: Option<&url::Url>,
        content_type: &str,
        cache_in_ram: bool,
        max_transfer_count: u32,
        carousel_delay: Option<std::time::Duration>,
        cache_control: Option<CacheControl>,
        groups: Option<Vec<String>>,
        cenc: lct::Cenc,
        inband_cenc: bool,
        oti: Option<oti::Oti>,
        md5: bool,
    ) -> Result<Box<ObjectDesc>> {
        let content_location = match content_location {
            Some(cl) => cl.clone(),
            None => url::Url::parse(&format!(
                "file:///{}",
                path.file_name()
                    .unwrap_or(OsStr::new(""))
                    .to_str()
                    .unwrap_or("")
            ))
            .unwrap_or(url::Url::parse("file:///").unwrap()),
        };

        if cache_in_ram {
            let content = std::fs::read(path)?;
            Self::create_with_content(
                content,
                Some(path.to_path_buf()),
                content_type.to_string(),
                content_location,
                max_transfer_count,
                carousel_delay,
                cache_control,
                groups,
                cenc,
                inband_cenc,
                oti,
                md5,
            )
        } else {
            Self::create_with_path(
                path.to_path_buf(),
                content_type.to_string(),
                content_location,
                max_transfer_count,
                carousel_delay,
                cache_control,
                groups,
                cenc,
                inband_cenc,
                oti,
                md5,
            )
        }
    }

    /// Return an `ObjectDesc` from a buffer
    pub fn create_from_buffer(
        content: &[u8],
        content_type: &str,
        content_location: &url::Url,
        max_transfer_count: u32,
        carousel_delay: Option<std::time::Duration>,
        cache_control: Option<CacheControl>,
        groups: Option<Vec<String>>,
        cenc: lct::Cenc,
        inband_cenc: bool,
        oti: Option<oti::Oti>,
        md5: bool,
    ) -> Result<Box<ObjectDesc>> {
        ObjectDesc::create_with_content(
            content.to_vec(),
            None,
            content_type.to_string(),
            content_location.clone(),
            max_transfer_count,
            carousel_delay,
            cache_control,
            groups,
            cenc,
            inband_cenc,
            oti,
            md5,
        )
    }

    fn create_with_content(
        mut content: Vec<u8>,
        path: Option<std::path::PathBuf>,
        content_type: String,
        content_location: url::Url,
        max_transfer_count: u32,
        carousel_delay: Option<std::time::Duration>,
        cache_control: Option<CacheControl>,
        groups: Option<Vec<String>>,
        cenc: lct::Cenc,
        inband_cenc: bool,
        oti: Option<oti::Oti>,
        md5: bool,
    ) -> Result<Box<ObjectDesc>> {
        let content_length = content.len();

        let md5 = match md5 {
            // https://www.rfc-editor.org/rfc/rfc2616#section-14.15
            true => {
                Some(base64::engine::general_purpose::STANDARD.encode(md5::compute(&content).0))
            }
            false => None,
        };

        if cenc != lct::Cenc::Null {
            content = compress::compress(&content, cenc)?;
            log::info!(
                "compress content from {} to {}",
                content_length,
                content.len()
            );
        }

        let transfer_length = content.len();

        Ok(Box::new(ObjectDesc {
            content_location,
            path,
            content: Some(content),
            content_type,
            content_length: content_length as u64,
            transfer_length: transfer_length as u64,
            cenc,
            inband_cenc,
            md5,
            attributes: None,
            oti,
            max_transfer_count,
            carousel_delay,
            cache_control,
            groups,
            toi: None,
        }))
    }

    fn create_with_path(
        path: std::path::PathBuf,
        content_type: String,
        content_location: url::Url,
        max_transfer_count: u32,
        carousel_delay: Option<std::time::Duration>,
        cache_control: Option<CacheControl>,
        groups: Option<Vec<String>>,
        cenc: lct::Cenc,
        inband_cenc: bool,
        oti: Option<oti::Oti>,
        md5: bool,
    ) -> Result<Box<ObjectDesc>> {
        if cenc != lct::Cenc::Null {
            return Err(FluteError::new(
                "Compressed object is not compatible with file path",
            ));
        }
        let file = std::fs::File::open(path.clone())?;
        let transfer_length = file.metadata()?.len();

        let md5 = match md5 {
            // https://www.rfc-editor.org/rfc/rfc2616#section-14.15
            true => Some(
                base64::engine::general_purpose::STANDARD.encode(Self::compute_file_md5(&file).0),
            ),
            false => None,
        };

        Ok(Box::new(ObjectDesc {
            content_location,
            path: Some(path.to_path_buf()),
            content: None,
            content_type,
            content_length: transfer_length,
            transfer_length,
            cenc,
            inband_cenc,
            md5,
            attributes: None,
            oti,
            max_transfer_count,
            carousel_delay,
            cache_control,
            groups,
            toi: None,
        }))
    }

    fn compute_file_md5(file: &std::fs::File) -> md5::Digest {
        let mut reader = BufReader::new(file);
        let mut context = md5::Context::new();
        let mut buffer = vec![0; 102400];

        loop {
            let count = reader.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            context.consume(&buffer[0..count]);
        }

        context.compute()
    }
}
