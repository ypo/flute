use super::lct;
use super::oti;
use crate::tools::error::{FluteError, Result};
use std::ffi::OsStr;

pub struct ObjectDesc {
    pub content_location: url::Url,
    pub path: Option<std::path::PathBuf>,
    pub content: Option<Vec<u8>>,
    pub content_type: String,
    pub content_length: u64,
    pub transfer_length: u64,
    pub cenc: lct::CENC,
    pub inband_cenc: bool,
    pub md5: Option<String>,
    pub attributes: Option<std::collections::HashMap<String, String>>,
    pub oti: Option<oti::Oti>,
    pub max_transfer_count: u32,
    pub carousel_delay_ns: Option<std::time::Duration>,
}

impl ObjectDesc {
    pub fn create_from_file(
        path: &std::path::Path,
        content_location: Option<&url::Url>,
        content_type: &str,
        max_transfer_count: u32,
        carousel_delay_ns: Option<std::time::Duration>,
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
            carousel_delay_ns,
        }))
    }

    pub fn create_from_buffer(
        content: &Vec<u8>,
        content_type: &str,
        content_location: &url::Url,
        max_transfer_count: u32,
        carousel_delay_ns: Option<std::time::Duration>,
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
            carousel_delay_ns,
        }))
    }

    pub fn content_encoding(&self) -> &str {
        match self.cenc {
            lct::CENC::Null => "null",
            lct::CENC::Zlib => "zlib",
            lct::CENC::Deflate => "deflate",
            lct::CENC::Gzip => "gzip",
        }
    }
}
