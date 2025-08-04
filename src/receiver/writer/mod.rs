//!
//! Write FLUTE objects to their final destination
//!
//! # Example
//!
//! ```
//! use flute::receiver::writer;
//!
//! let enable_md5_check = true;
//! let writer = writer::ObjectWriterFSBuilder::new(&std::path::Path::new("./destination_dir"), enable_md5_check).ok();
//! ```
//!

use std::collections::HashMap;
use std::time::Duration;
use std::time::SystemTime;

use crate::common::udpendpoint::UDPEndpoint;
use crate::core::lct::Cenc;
use crate::core::Oti;
use crate::tools::error::Result;

///
/// Cache-Duration for an object.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectCacheControl {
    /// No cache duration, the object should not be cached
    NoCache,
    /// Should be cached permanently
    MaxStale,
    /// Cache duration with a specific time
    ExpiresAt(SystemTime),
    /// When hint when there is not cache-control directive for this object
    ExpiresAtHint(SystemTime),
}

impl ObjectCacheControl {
    /// Hint if the cache control should be updated
    pub fn should_update(&self, cache_control: ObjectCacheControl) -> bool {
        match self {
            ObjectCacheControl::NoCache => cache_control != ObjectCacheControl::NoCache,
            ObjectCacheControl::MaxStale => cache_control != ObjectCacheControl::MaxStale,
            ObjectCacheControl::ExpiresAt(expires_at) => {
                if let ObjectCacheControl::ExpiresAt(d) = cache_control {
                    let diff = match d < *expires_at {
                        true => expires_at.duration_since(d).unwrap_or_default(),
                        false => d.duration_since(*expires_at).unwrap_or_default(),
                    };
                    return diff > std::time::Duration::from_secs(1);
                }

                true
            }
            ObjectCacheControl::ExpiresAtHint(expires_at) => {
                if let ObjectCacheControl::ExpiresAtHint(d) = cache_control {
                    let diff = match d < *expires_at {
                        true => expires_at.duration_since(d).unwrap_or_default(),
                        false => d.duration_since(*expires_at).unwrap_or_default(),
                    };
                    return diff > std::time::Duration::from_secs(1);
                }

                true
            }
        }
    }
}

///
/// Struct representing metadata for an object.
///
#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    /// URI that can be used as an identifier for this object
    pub content_location: String,
    /// Final size of this object
    pub content_length: Option<usize>,
    /// Transfer length (compressed) of this object
    pub transfer_length: Option<usize>,
    /// The content type of this object.
    /// This field describes the format of the object's content,
    /// and can be used to determine how to handle or process the object.
    pub content_type: Option<String>,
    /// Object Cache Control
    pub cache_control: ObjectCacheControl,
    /// List of groups
    pub groups: Option<Vec<String>>,
    /// Object MD5
    pub md5: Option<String>,
    /// Opentelemetry propagation context
    pub optel_propagator: Option<HashMap<String, String>>,
    /// Object Transmission Information (OTI) of the received object
    pub oti: Option<Oti>,
    /// CENC information
    pub cenc: Option<Cenc>,
    /// ETag
    pub e_tag: Option<String>,
}

///
/// Represents the result when the creation of an `ObjectWriter` is requested.
///
#[derive(Debug)]
pub enum ObjectWriterBuilderResult {
    /// Indicates that the object must be stored using the provided writer.
    StoreObject(Box<dyn ObjectWriter>),
    /// Indicates that the object has already been received and does not need to be stored again.
    ObjectAlreadyReceived,
    /// Indicates that an error occurred and the object cannot be stored.
    Abort,
}

///
/// A trait for building an `ObjectWriter`
///
pub trait ObjectWriterBuilder {
    /// Return a new object writer that will be used to store the received object to its final destination
    fn new_object_writer(
        &self,
        endpoint: &UDPEndpoint,
        tsi: &u64,
        toi: &u128,
        meta: &ObjectMetadata,
        now: std::time::SystemTime,
    ) -> ObjectWriterBuilderResult;
    /// Triggered when the cache control of an object is updated
    fn update_cache_control(
        &self,
        endpoint: &UDPEndpoint,
        tsi: &u64,
        toi: &u128,
        meta: &ObjectMetadata,
        now: std::time::SystemTime,
    );
    /// Called when an FDT is received
    fn fdt_received(
        &self,
        endpoint: &UDPEndpoint,
        tsi: &u64,
        fdt_xml: &str,
        expires: std::time::SystemTime,
        meta: &ObjectMetadata,
        transfer_duration: Duration,
        now: std::time::SystemTime,
        ext_time: Option<std::time::SystemTime>,
    );
}

///
/// A trait for writing an object to its final destination.
///
pub trait ObjectWriter {
    /// Open the destination
    ///
    /// Returns `Ok(())` if the destination is opened successfully, or an error if it fails.
    fn open(&self, now: SystemTime) -> Result<()>;
    /// Writes a data block associated with a specific Source Block Number (SBN).
    ///
    /// # Arguments
    ///
    /// * `sbn` - The Source Block Number identifying the data block's origin or sequence.
    /// * `data` - A byte slice representing the content to be written.
    /// * `now` - The current system time, typically used for timestamping or aging logic.
    ///
    /// Returns `Ok(())` if the destination is opened successfully, or an error if it fails.
    /// In case of an error, the object will move to error state
    fn write(&self, sbn: u32, data: &[u8], now: SystemTime) -> Result<()>;
    /// Called when all the data has been written
    fn complete(&self, now: SystemTime);
    /// Called when an error occurred during the reception of this object
    fn error(&self, now: SystemTime);
    /// Called when the sender has interrupted the transmission of this object
    fn interrupted(&self, now: SystemTime);
    /// Indicates whether MD5 checksum verification is enabled for this object.
    ///
    /// - `true`: The MD5 checksum will be verified. If the checksum is invalid,
    ///   the object will transition to an error state.
    /// - `false`: The MD5 checksum will be skipped. Even if the checksum is invalid
    ///   or missing, the object will proceed to a complete state without error.
    fn enable_md5_check(&self) -> bool;
}

impl std::fmt::Debug for dyn ObjectWriterBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ObjectWriterBuilder {{  }}")
    }
}

impl std::fmt::Debug for dyn ObjectWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ObjectWriter {{  }}")
    }
}

mod objectwriterbuffer;
mod objectwriterfs;

pub use objectwriterbuffer::ObjectWriterBuffer;
pub use objectwriterbuffer::ObjectWriterBufferBuilder;

pub use objectwriterfs::ObjectWriterFS;
pub use objectwriterfs::ObjectWriterFSBuilder;
