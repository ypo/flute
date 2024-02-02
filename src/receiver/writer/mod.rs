//!
//! Write FLUTE objects to their final destination
//!
//! # Example
//!
//! ```
//! use flute::receiver::writer;
//!
//! let writer = writer::ObjectWriterFSBuilder::new(&std::path::Path::new("./destination_dir")).ok();
//! ```
//!

use std::time::Duration;

use crate::common::udpendpoint::UDPEndpoint;
use crate::tools::error::Result;

///
/// Struct representing metadata for an object.
///
#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    /// URI that can be used as an identifier for this object
    pub content_location: url::Url,
    /// Anticipated size of this object
    pub content_length: Option<usize>,
    /// The content type of this object.
    /// This field describes the format of the object's content,
    /// and can be used to determine how to handle or process the object.
    pub content_type: Option<String>,
    /// Object cache duration hint
    pub cache_duration: Option<Duration>,
    /// List of groups
    pub groups: Option<Vec<String>>,
    /// Object MD5
    pub md5: Option<String>,
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
        meta: Option<&ObjectMetadata>,
        now: std::time::SystemTime,
    ) -> Box<dyn ObjectWriter>;
    /// Update cache duration of an object
    fn set_cache_duration(
        &self,
        endpoint: &UDPEndpoint,
        tsi: &u64,
        toi: &u128,
        content_location: &url::Url,
        duration: &Duration,
    );
    /// Called when an FDT is received
    fn fdt_received(
        &self,
        endpoint: &UDPEndpoint,
        tsi: &u64,
        fdt_xml: &str,
        expires: std::time::SystemTime,
        now: std::time::SystemTime,
    );
}

///
/// A trait for writing an object to its final destination.
///
pub trait ObjectWriter {
    /// Open the destination
    fn open(&self) -> Result<()>;
    /// Write data
    fn write(&self, data: &[u8]);
    /// Called when all the data has been written
    fn complete(&self);
    /// Called when an error occurred during the reception of this object
    fn error(&self);
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
