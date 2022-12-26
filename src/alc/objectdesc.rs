use super::compress;
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
        cenc: lct::CENC,
        inband_cenc: bool,
        oti: Option<oti::Oti>,
        md5: bool,
    ) -> Result<Box<ObjectDesc>> {
        let content = std::fs::read(path)?;
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

        ObjectDesc::create_with_content(
            content,
            Some(path.to_path_buf()),
            content_type.to_string(),
            content_location,
            max_transfer_count,
            carousel_delay,
            cenc,
            inband_cenc,
            oti,
            md5,
        )
    }

    /// Return an `ObjectDesc` from a buffer
    pub fn create_from_buffer(
        content: &Vec<u8>,
        content_type: &str,
        content_location: &url::Url,
        max_transfer_count: u32,
        carousel_delay: Option<std::time::Duration>,
        cenc: lct::CENC,
        inband_cenc: bool,
        oti: Option<oti::Oti>,
        md5: bool,
    ) -> Result<Box<ObjectDesc>> {
        ObjectDesc::create_with_content(
            content.clone(),
            None,
            content_type.to_string(),
            content_location.clone(),
            max_transfer_count,
            carousel_delay,
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
        cenc: lct::CENC,
        inband_cenc: bool,
        oti: Option<oti::Oti>,
        md5: bool,
    ) -> Result<Box<ObjectDesc>> {
        let content_length = content.len();

        let md5 = match md5 {
            // https://www.rfc-editor.org/rfc/rfc2616#section-14.15
            true => Some(base64::encode(md5::compute(&content).0)),
            false => None,
        };

        if cenc != lct::CENC::Null {
            content = compress::compress(&content, cenc)?;
            log::info!(
                "compress content from {} to {}",
                content_length,
                content.len()
            );
        }

        let transfer_length = content.len();

        Ok(Box::new(ObjectDesc {
            content_location: content_location,
            path: path,
            content: Some(content),
            content_type: content_type,
            content_length: content_length as u64,
            transfer_length: transfer_length as u64,
            cenc: cenc,
            inband_cenc: inband_cenc,
            md5: md5,
            attributes: None,
            oti: oti,
            max_transfer_count,
            carousel_delay,
        }))
    }
}
