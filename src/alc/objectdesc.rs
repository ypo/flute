use super::lct;
use super::oti;
use crate::tools::error::Result;
use std::ffi::OsStr;

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
    /// Size of the object after transfer-coding (`CENC`) has been applied
    /// as defined in [rfc2616 4.4](https://www.rfc-editor.org/rfc/rfc2616#section-4.4)
    pub transfer_length: u64,
    /// Content Encoding (compression)
    pub cenc: lct::CENC,
    /// If `true`, CENC extension are added to ALC/LCT packet
    /// Else CENC is defined only inside the FDT
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
}

impl ObjectDesc {
    /// Return an `ObjectDesc` from a file
    pub fn create_from_file(
        path: &std::path::Path,
        content_location: Option<&url::Url>,
        content_type: &str,
        max_transfer_count: u32,
        carousel_delay: Option<std::time::Duration>,
    ) -> Result<Box<ObjectDesc>> {
        let content = std::fs::read(path)?;
        let content_length = content.len();
        let transfer_length = content_length;

        // TODO CENC

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

        Ok(Box::new(ObjectDesc {
            content_location: content_location.clone(),
            path: Some(path.to_path_buf()),
            content: Some(content),
            content_type: content_type.to_string(),
            content_length: content_length as u64,
            transfer_length: transfer_length as u64,
            cenc: lct::CENC::Null,
            inband_cenc: true,
            md5: None,
            attributes: None,
            oti: None,
            max_transfer_count,
            carousel_delay,
        }))
    }

    /// Return an `ObjectDesc` from a buffer
    pub fn create_from_buffer(
        content: &Vec<u8>,
        content_type: &str,
        content_location: &url::Url,
        max_transfer_count: u32,
        carousel_delay: Option<std::time::Duration>,
    ) -> Result<Box<ObjectDesc>> {
        let content_length = content.len();
        let transfer_length = content_length;

        Ok(Box::new(ObjectDesc {
            content_location: content_location.clone(),
            path: None,
            content: Some(content.clone()),
            content_type: content_type.to_string(),
            content_length: content_length as u64,
            transfer_length: transfer_length as u64,
            cenc: lct::CENC::Null,
            inband_cenc: true,
            md5: None,
            attributes: None,
            oti: None,
            max_transfer_count,
            carousel_delay,
        }))
    }
}
