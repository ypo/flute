use base64::Engine;

use super::compress;
use super::toiallocator::Toi;
use crate::common::{fdtinstance, lct, oti};
use crate::error::FluteError;
use crate::tools;
use crate::tools::error::Result;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::BufReader;
use std::io::{Read, Seek};
use std::sync::Mutex;
use std::time::SystemTime;

/// Cache Control
///
/// The `CacheControl` enum represents different directives used for controlling caching behavior.
/// It is commonly used in web development to indicate caching preferences for specific files or resources.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum CacheControl {
    /// Specifies that the receiver should not cache the specific file or resource.
    NoCache,

    /// Indicates that a specific file (or set of files) should be cached for an indefinite period of time,
    /// allowing stale versions of the resource to be served even after they have expired.
    MaxStale,

    /// Specifies the expected expiry time for the file or resource, allowing the server
    /// to indicate when the cached version should no longer be considered valid.
    Expires(std::time::Duration),

    /// Specifies the expected expiry time for the file or resource, using a specific timestamp.
    ExpiresAt(SystemTime),
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
        CacheControl::ExpiresAt(timestamp) => {
            let ntp = tools::system_time_to_ntp(*timestamp).unwrap_or_default();
            fdtinstance::CacheControl {
                value: fdtinstance::CacheControlChoice::Expires((ntp >> 32) as u32),
            }
        }
    }
}

///
/// Target Acquisition for Object
///
#[derive(Debug, Clone)]
pub enum TargetAcquisition {
    /// Transfer the object as fast as possible
    AsFastAsPossible,
    /// Transfer the object within the specified duration
    WithinDuration(std::time::Duration),
    /// Transfer the object within the specified timestamp
    WithinTime(std::time::SystemTime),
}

///
/// Object Data Stream Trait
///
pub trait ObjectDataStreamTrait:
    std::io::Read + std::io::Seek + Send + Sync + std::fmt::Debug
{
}
impl<T: std::io::Read + std::io::Seek + Send + Sync + std::fmt::Debug> ObjectDataStreamTrait for T {}

impl dyn ObjectDataStreamTrait + '_ {
    /// Calculate the MD5 hash of the stream
    pub fn md5_base64(&mut self) -> Result<String> {
        let md5 = self.md5()?;
        // https://www.rfc-editor.org/rfc/rfc2616#section-14.15
        Ok(base64::engine::general_purpose::STANDARD.encode(md5.0))
    }

    fn md5(&mut self) -> Result<md5::Digest> {
        self.seek(std::io::SeekFrom::Start(0))?;
        let mut reader = BufReader::new(self);
        let mut context = md5::Context::new();
        let mut buffer = vec![0; 102400];

        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            context.consume(&buffer[0..count]);
        }

        reader.seek(std::io::SeekFrom::Start(0))?;
        Ok(context.compute())
    }
}

/// Boxed Object Data Stream
pub type ObjectDataStream = Box<dyn ObjectDataStreamTrait>;

/// Object Data Source
#[derive(Debug)]
pub enum ObjectDataSource {
    /// Source from a stream
    Stream(Mutex<ObjectDataStream>),
    /// Source from a buffer
    Buffer(Vec<u8>),
}

impl ObjectDataSource {
    /// Create an Object Data Source from a buffer
    pub fn from_buffer(buffer: &[u8], cenc: lct::Cenc) -> Result<Self> {
        let data = match cenc {
            lct::Cenc::Null => Ok(buffer.to_vec()),
            _ => compress::compress_buffer(buffer, cenc),
        }?;

        Ok(ObjectDataSource::Buffer(data))
    }

    /// Create an Object Data Source from a vector
    pub fn from_vec(buffer: Vec<u8>, cenc: lct::Cenc) -> Result<Self> {
        let data = match cenc {
            lct::Cenc::Null => Ok(buffer.to_vec()),
            _ => compress::compress_buffer(&buffer, cenc),
        }?;

        Ok(ObjectDataSource::Buffer(data))
    }

    /// Create an Object Data Source from a stream
    pub fn from_stream(stream: ObjectDataStream) -> Self {
        ObjectDataSource::Stream(Mutex::new(stream))
    }

    fn len(&mut self) -> Result<u64> {
        match self {
            ObjectDataSource::Buffer(buffer) => Ok(buffer.len() as u64),
            ObjectDataSource::Stream(stream) => {
                let mut stream = stream.lock().unwrap();
                let current_pos = stream.stream_position()?;
                let end_pos = stream.seek(std::io::SeekFrom::End(0))?;
                stream.seek(std::io::SeekFrom::Start(current_pos))?;
                Ok(end_pos)
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
/// Carousel Repeat Mode
pub enum CarouselRepeatMode {
    /// Waits for a specified duration at the end of transfer before starting the next one.
    DelayBetweenTransfers(std::time::Duration),

    /// Ensures each transfer starts at a fixed interval. interval = (transfer + delay).
    IntervalBetweenStartTimes(std::time::Duration),
}

///
/// Object (file) that can be send over FLUTE
///
#[derive(Debug)]
pub struct ObjectDesc {
    /// supply the resource location for this object
    /// as defined in [rfc2616 14.14](https://www.rfc-editor.org/rfc/rfc2616#section-14.14)
    pub content_location: url::Url,
    /// Data Source of the object
    pub source: ObjectDataSource,
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
    /// If defined, FEC Object Transmission Information (OTI) overload the default OTI defined in the FDT
    pub oti: Option<oti::Oti>,
    /// Repeat the transfer the same object multiple times
    pub max_transfer_count: u32,
    /// Specifies the desired duration for transferring the object to the receiver.
    /// When enabled, the transfer bitrate of this object is adapted to reach the transfer duration.
    pub target_acquisition: Option<TargetAcquisition>,
    /// Controls how an object is repeatedly transferred in a carousel loop.
    /// When set, the object remains in the carousel and is retransmitted at regular intervals
    /// until it is explicitly removed.
    pub carousel_mode: Option<CarouselRepeatMode>,
    /// Optional: specifies an absolute system time at which the first transfer of the object should start.
    /// If not set, the transfer starts immediately.
    pub transfer_start_time: Option<SystemTime>,
    /// Define object cache control
    pub cache_control: Option<CacheControl>,
    /// Add file to a list of groups
    pub groups: Option<Vec<String>>,
    /// Assign an optional TOI to this object
    pub toi: Option<Box<Toi>>,
    /// Optional Opentelemetry propagator (only available with the `opentelemetry` feature)
    pub optel_propagator: Option<HashMap<String, String>>,
    /// Optional ETag or entity-tag as defined in RFC 2616
    pub e_tag: Option<String>,
    /// If `true`, the object can be stopped immediately before the first transfer
    /// if `false` (default) then transfer is stopped only after being transferred at least once
    pub allow_immediate_stop_before_first_transfer: Option<bool>,
}

impl ObjectDesc {
    /// Assigns a Transport Object Identification (TOI) to this object.
    ///
    /// If no TOI is assigned, a new TOI will be created during the push of this object into the FLUTE session.
    ///
    /// # Arguments
    ///
    /// * `toi` - A boxed `Toi` object representing the Time of Interest to be assigned.
    pub fn set_toi(&mut self, toi: Box<Toi>) {
        self.toi = Some(toi);
    }

    /// Return an `ObjectDesc` from a file
    pub fn create_from_file(
        path: &std::path::Path,
        content_location: Option<&url::Url>,
        content_type: &str,
        cache_in_ram: bool,
        max_transfer_count: u32,
        carousel_mode: Option<CarouselRepeatMode>,
        target_acquisition: Option<TargetAcquisition>,
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
                content_type.to_string(),
                content_location,
                max_transfer_count,
                carousel_mode,
                target_acquisition,
                cache_control,
                groups,
                cenc,
                inband_cenc,
                oti,
                md5,
            )
        } else {
            if cenc != lct::Cenc::Null {
                return Err(FluteError::new(
                    "Compressed object is not compatible with file path",
                ));
            }
            let file = std::fs::File::open(path)?;
            Self::create_from_stream(
                Box::new(file),
                content_type,
                &content_location,
                max_transfer_count,
                carousel_mode,
                target_acquisition,
                cache_control,
                groups,
                inband_cenc,
                oti,
                md5,
            )
        }
    }

    /// Create an Object Description from a stream
    pub fn create_from_stream(
        mut stream: ObjectDataStream,
        content_type: &str,
        content_location: &url::Url,
        max_transfer_count: u32,
        carousel_mode: Option<CarouselRepeatMode>,
        target_acquisition: Option<TargetAcquisition>,
        cache_control: Option<CacheControl>,
        groups: Option<Vec<String>>,
        inband_cenc: bool,
        oti: Option<oti::Oti>,
        md5: bool,
    ) -> Result<Box<ObjectDesc>> {
        let md5 = match md5 {
            true => Some(stream.md5_base64()?),
            false => None,
        };

        let mut source = ObjectDataSource::from_stream(stream);
        let transfer_length = source.len()?;

        Ok(Box::new(ObjectDesc {
            content_location: content_location.clone(),
            source,
            content_type: content_type.to_string(),
            content_length: transfer_length,
            transfer_length,
            cenc: lct::Cenc::Null,
            inband_cenc,
            md5,
            oti,
            max_transfer_count,
            carousel_mode,
            target_acquisition,
            transfer_start_time: None,
            cache_control,
            groups,
            toi: None,
            optel_propagator: None,
            e_tag: None,
            allow_immediate_stop_before_first_transfer: None,
        }))
    }

    /// Return an `ObjectDesc` from a buffer
    pub fn create_from_buffer(
        content: Vec<u8>,
        content_type: &str,
        content_location: &url::Url,
        max_transfer_count: u32,
        carousel_mode: Option<CarouselRepeatMode>,
        target_acquisition: Option<TargetAcquisition>,
        cache_control: Option<CacheControl>,
        groups: Option<Vec<String>>,
        cenc: lct::Cenc,
        inband_cenc: bool,
        oti: Option<oti::Oti>,
        md5: bool,
    ) -> Result<Box<ObjectDesc>> {
        ObjectDesc::create_with_content(
            content,
            content_type.to_string(),
            content_location.clone(),
            max_transfer_count,
            carousel_mode,
            target_acquisition,
            cache_control,
            groups,
            cenc,
            inband_cenc,
            oti,
            md5,
        )
    }

    fn create_with_content(
        content: Vec<u8>,
        content_type: String,
        content_location: url::Url,
        max_transfer_count: u32,
        carousel_mode: Option<CarouselRepeatMode>,
        target_acquisition: Option<TargetAcquisition>,
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

        let mut source = ObjectDataSource::from_vec(content, cenc)?;
        let transfer_length = source.len()?;

        Ok(Box::new(ObjectDesc {
            content_location,
            source,
            content_type,
            content_length: content_length as u64,
            transfer_length,
            cenc,
            inband_cenc,
            md5,
            oti,
            max_transfer_count,
            carousel_mode,
            target_acquisition,
            transfer_start_time: None,
            cache_control,
            groups,
            toi: None,
            optel_propagator: None,
            e_tag: None,
            allow_immediate_stop_before_first_transfer: None,
        }))
    }
}
